/**
 * Browser half of the CopySpeak voice bundle.
 *
 * Adds three surfaces that share one transport to the CopySpeak control server:
 *
 *   POST http://127.0.0.1:43117/speak
 *   {"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
 *
 * - A CopySpeak panel in the sidebar, holding the two options and the app status.
 * - A speaker action in the action row of each finished assistant message.
 * - A per-session reader that speaks each settled reply when auto-read is on,
 *   and highlights the word CopySpeak plays inside the reply text itself.
 *
 * The request uses `mode: 'no-cors'` and a `text/plain` content type, so the
 * browser sends no CORS preflight; CopySpeak parses the JSON body without
 * reading the content type. The response is opaque, so a success only proves
 * the request reached the app. `wait: true` makes it resolve at playback end,
 * which restores the idle action state.
 *
 * Captions poll `GET http://127.0.0.1:43117/status`, which answers the caption
 * snapshot the CopySpeak webview already emits (text, word timings, position).
 * That read-only route is CORS-open. Polling runs only while this plugin's own
 * reading plays, and the highlight hides when the route is absent.
 */
window.__ModuleLoader__.load({
  id: "@local/copyspeak-voice",
  factory(require) {
    const React = require("react");
    const h = React.createElement;

    const NS = "copyspeak-voice";
    const PACKAGE = "@local/copyspeak-voice";
    const PANEL_ID = "copyspeak";
    const CONTROL_URL = "http://127.0.0.1:43117";
    const STYLE_KEY = PACKAGE + "/client.css";
    /* Custom-highlight name. Namespace it: other plugins may paint highlights. */
    const HIGHLIGHT_NAME = "copyspeak-read-word";
    const STORAGE = {
      autoRead: "dsh.copyspeak-voice.auto-read",
      captions: "dsh.copyspeak-voice.captions"
    };
    /* The webview reports its audible clock every ~25 ms and throttles the
     * control-server report to ~100 ms, so one poll per 100 ms stays current. */
    const CAPTION_POLL_MS = 100;
    /* Stop polling when the status route never answers (older CopySpeak). */
    const CAPTION_ATTEMPTS = 3;
    /* A smooth scroll per word would fight itself; one per quarter second. */
    const SCROLL_INTERVAL_MS = 250;
    /* Keep the spoken word inside the middle band of the chat scrollport. */
    const SCROLL_MARGIN = 0.25;
    /* Token look-ahead while aligning a caption fragment to rendered words. */
    const ALIGN_WINDOW = 24;
    const ALIGN_ATTEMPTS = 8;

    /*
     * CopySpeak app-logo mark: the speaker and its two sound waves, with the
     * gradient card and every layer color dropped. One currentColor fill, so
     * the glyph follows the host theme in light and dark modes.
     */
    const GLYPH_VIEW_BOX = "14.86 14.717 48.611 48.611";
    const GLYPH_PATHS = [
      "m37.35 20.74c-0.74-0.04-1.46 0.21-2.02 0.71l-7.68 7.13c-0.24 0.23-0.56 0.37-0.9 0.39l-5.58 0.21c-2.55 0.1-4.51 2.23-4.51 5v7.99c0 2.81 2.1 4.95 4.74 5.09l5.3 0.28c0.28 0.02 0.54 0.15 0.74 0.35l7.94 8.12c0.71 0.71 1.67 1.04 2.55 1.02 1.52-0.04 2.65-1.2 2.65-2.76v-30.18c0-1.84-1.65-3.27-3.23-3.35z",
      "m49.61 19.43c-1.5-0.74-3.32-0.37-4.11 1.04-0.8 1.45-0.21 3.13 1.27 3.97 5.25 2.81 8.39 8.04 8.39 14.44 0 6.47-3.45 12.05-8.57 14.61-1.47 0.81-2 2.47-1.33 3.85 0.72 1.4 2.78 2.03 4.44 1.41 7.13-3.14 11.97-10.79 11.97-19.57 0-8.67-5.02-16.48-12.06-19.75z",
      "m46.38 28.77c-1.27-0.75-2.96-0.42-3.67 0.89-0.75 1.28-0.3 2.86 0.86 3.78 1.73 1.33 2.77 3.26 2.77 5.48 0 2.27-1.14 4.28-2.83 5.45-1.2 0.9-1.48 2.53-0.7 3.78 0.84 1.23 2.55 1.74 3.95 1.16 3.49-1.78 5.54-5.56 5.54-9.89 0-4.62-2.49-8.79-5.92-10.65z"
    ];

    const en = {
      "panel.label": "CopySpeak",
      "panel.title": "CopySpeak",
      "panel.intro": "Reads assistant replies aloud through the CopySpeak desktop app.",
      "status.checking": "Checking for CopySpeak…",
      "status.online": "CopySpeak is running.",
      "status.offline": "CopySpeak is not running. Start it to read replies aloud.",
      "autoRead.title": "Read new replies aloud",
      "autoRead.description": "CopySpeak reads each reply in the open session when it settles.",
      "captions.title": "Show captions while reading",
      "captions.description": "Highlights each word in the reply as CopySpeak plays it.",
      "switch.on": "On",
      "switch.off": "Off",
      "action.read": "Read aloud with CopySpeak",
      "action.stop": "Stop reading",
      "error.empty": "Nothing to read aloud",
      "error.offline": "CopySpeak is not running",
      "error.failed": "CopySpeak request failed"
    };

    const PLUGIN_STYLE = [
      ".dsh-csva-glyph{display:block}",
      ".dsh-csva-root{display:inline-flex;align-items:center;gap:8px}",
      ".dsh-csva-button{width:28px;height:28px;border-radius:var(--dsw-radius-sm,6px);border:none;background:0 0;padding:6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:var(--dsw-alias-label-secondary);}",
      ".dsh-csva-button svg{width:15px;height:15px}",
      ".dsh-csva-button:hover:not(:disabled){background:var(--dsw-alias-bg-layer-2);color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-button[data-active]{color:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-button:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-button:disabled{opacity:.4;cursor:default}",
      ".dsh-csva-note{font-size:12px;line-height:16px;color:var(--dsw-alias-state-error-primary);white-space:nowrap}",
      ".dsh-csva-panel{box-sizing:border-box;width:100%;height:100%;padding:calc(var(--dsh-frame-top-clearance,0px) + 28px) clamp(24px,4vw,48px) 48px;color:var(--dsw-alias-label-primary);background:var(--dsw-alias-bg-base);display:flex;flex-direction:column;align-items:center;gap:20px;overflow:hidden auto}",
      ".dsh-csva-panel>*{flex:none;width:100%;max-width:720px}",
      ".dsh-csva-heading{display:flex;flex-direction:column;gap:6px}",
      ".dsh-csva-title{margin:0;font-size:20px;line-height:28px;font-weight:600}",
      ".dsh-csva-intro{margin:0;font-size:13px;line-height:20px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-status{display:inline-flex;align-items:center;gap:8px;margin:0;color:var(--dsw-alias-label-secondary);font-size:13px;line-height:20px}",
      ".dsh-csva-dot{flex:none;width:8px;height:8px;border-radius:50%;background:var(--dsw-alias-state-idle-primary)}",
      ".dsh-csva-status[data-state=online] .dsh-csva-dot{background:var(--dsw-alias-state-success-primary)}",
      ".dsh-csva-status[data-state=offline] .dsh-csva-dot{background:var(--dsw-alias-state-error-primary)}",
      ".dsh-csva-rows{display:flex;flex-direction:column;gap:10px;margin:0;padding:0;list-style:none}",
      ".dsh-csva-row{display:flex;align-items:center;gap:16px;padding:12px 14px;border:.5px solid var(--dsw-alias-border-l1);border-radius:var(--dsw-radius-xl,10px);background:var(--dsw-alias-bg-layer-1)}",
      ".dsh-csva-rowText{display:flex;flex-direction:column;gap:2px;min-width:0;flex:1}",
      ".dsh-csva-rowTitle{font-size:13px;line-height:20px;color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-rowDescription{font-size:12px;line-height:18px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-switch{position:relative;flex:none;width:36px;height:20px;padding:0;border:none;border-radius:999px;background:var(--dsw-alias-state-idle-primary);cursor:pointer;transition:background .15s}",
      ".dsh-csva-switch[data-on]{background:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-switch:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-switchThumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;background:var(--dsw-alias-bg-base);transition:transform .15s}",
      ".dsh-csva-switch[data-on] .dsh-csva-switchThumb{transform:translateX(16px)}",
      "::highlight(" +
        HIGHLIGHT_NAME +
        "){background-color:var(--dsw-alias-brand-primary);color:var(--dsw-alias-bg-base)}"
    ].join("\n");

    /* One reading at a time; every surface observes this record. */
    const playback = {
      messageId: null,
      requestId: null,
      listeners: new Set(),
      sequence: 1
    };

    function emit(listeners) {
      for (const listener of Array.from(listeners)) listener();
    }

    function subscribe(listeners, listener) {
      listeners.add(listener);
      return function () {
        listeners.delete(listener);
      };
    }

    function remember(messageId, requestId) {
      playback.messageId = messageId;
      playback.requestId = requestId;
      emit(playback.listeners);
    }

    function forget() {
      playback.messageId = null;
      playback.requestId = null;
      stopCaptionPolling();
      resetReading(null, null, "");
      emit(playback.listeners);
    }

    function nextRequestId() {
      const stamp = Date.now().toString(36);
      const sequence = playback.sequence.toString(36);
      playback.sequence += 1;
      return "dsh." + stamp + "." + sequence;
    }

    /* User preferences: page-local, so a switch keeps its state across
       reloads without a Host round trip. Captions start off. */
    const preferences = {
      autoRead: readStoredFlag(STORAGE.autoRead, false),
      captions: readStoredFlag(STORAGE.captions, false),
      listeners: new Set()
    };

    function readStoredFlag(key, fallback) {
      try {
        const stored = window.localStorage.getItem(key);
        if (stored === null) return fallback;
        return stored === "1";
      } catch {
        return fallback;
      }
    }

    function setPreference(name, enabled) {
      if (name === "autoRead") {
        preferences.autoRead = enabled;
      } else {
        preferences.captions = enabled;
      }
      try {
        window.localStorage.setItem(STORAGE[name], enabled ? "1" : "0");
      } catch {
        /* Private mode: the switch still works for this page. */
      }
      if (name === "captions") {
        if (!enabled) {
          stopCaptionPolling();
          clearHighlight();
        } else if (playback.requestId !== null) {
          startCaptionPolling(playback.requestId);
        }
      }
      emit(preferences.listeners);
    }

    /* ---------- Rendered-message word highlight ---------- */

    /*
     * The word highlight paints inside the assistant reply itself: the Chat
     * view marks each settled Markdown body with `data-text-interaction-source`,
     * and each Chat row carries `data-chat-node-key` (the durable node key the
     * Chat snapshot also exposes) plus `data-chat-flow-kind`.
     *
     * Caption word offsets index the fragment text CopySpeak spoke, not the
     * rendered Markdown. Sanitizers and pagination change the two strings, so
     * offsets cannot transfer directly. The reader instead aligns the fragment's
     * normalized words onto the rendered text nodes once per fragment, then
     * paints the active word with the CSS Custom Highlight API.
     */
    const WORD_SPAN = /\S+/gu;
    const WORD_CHAR = /[\p{L}\p{N}]/u;
    const ROW_SELECTOR = '[data-chat-flow-kind="assistant-step"]';
    const SOURCE_SELECTOR = "[data-text-interaction-source]";

    /* Current reading and its resolved DOM target. */
    const reading = {
      messageId: null,
      nodeKey: null,
      speechTokens: [],
      element: null,
      tokens: null,
      cursor: 0,
      fragmentText: null,
      fragmentWords: null,
      fragmentEntries: 0,
      fragmentMap: null,
      fragmentPosition: 0,
      scroller: null,
      activeToken: -1,
      lastScrollAt: 0
    };

    /* Lowercased letters and digits only, so punctuation and wrapping do not
     * split a match between the spoken text and the rendered text. */
    function normalizeWord(raw) {
      return raw.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
    }

    /* The first and last letter or digit of a whitespace-free run. */
    function wordSpan(raw) {
      let start = 0;
      let end = raw.length;
      while (start < end && !WORD_CHAR.test(raw[start])) start += 1;
      while (end > start && !WORD_CHAR.test(raw[end - 1])) end -= 1;
      return { start: start, end: end };
    }

    function tokensFromText(text) {
      const tokens = [];
      for (const match of text.matchAll(WORD_SPAN)) {
        const norm = normalizeWord(match[0]);
        if (norm !== "") tokens.push({ norm: norm });
      }
      return tokens;
    }

    function tokensFromElement(element) {
      const tokens = [];
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      let node = walker.nextNode();
      while (node !== null) {
        const text = node.nodeValue ?? "";
        for (const match of text.matchAll(WORD_SPAN)) {
          const norm = normalizeWord(match[0]);
          if (norm === "") continue;
          const span = wordSpan(match[0]);
          tokens.push({
            node: node,
            start: match.index + span.start,
            end: match.index + span.end,
            norm: norm
          });
        }
        node = walker.nextNode();
      }
      return tokens;
    }

    function tokenCounts(tokens) {
      const counts = new Map();
      for (const token of tokens) counts.set(token.norm, (counts.get(token.norm) ?? 0) + 1);
      return counts;
    }

    /* How many spoken words appear in the rendered text, as a bag of words. */
    function overlapScore(tokens, spokenTokens) {
      const remaining = tokenCounts(tokens);
      let score = 0;
      for (const token of spokenTokens) {
        const count = remaining.get(token.norm) ?? 0;
        if (count === 0) continue;
        remaining.set(token.norm, count - 1);
        score += 1;
      }
      return score;
    }

    function mappedCount(map) {
      let count = 0;
      for (const index of map) if (index >= 0) count += 1;
      return count;
    }

    function nearestMapped(map, index) {
      for (let distance = 0; distance <= 3; distance += 1) {
        const before = index - distance;
        if (before >= 0 && map[before] >= 0) return map[before];
        const after = index + distance;
        if (after < map.length && map[after] >= 0) return map[after];
      }
      return -1;
    }

    /* Match the fragment words that surround one confirmed anchor token. */
    function matchAround(fragmentWords, tokens, anchor, anchorToken) {
      const map = fragmentWords.map(function () {
        return -1;
      });
      map[anchor] = anchorToken;
      let token = anchorToken - 1;
      for (let index = anchor - 1; index >= 0; index -= 1) {
        const norm = fragmentWords[index].norm;
        if (norm === "") continue;
        const lowest = Math.max(0, token - ALIGN_WINDOW);
        for (let scan = token; scan >= lowest; scan -= 1) {
          if (tokens[scan].norm !== norm) continue;
          map[index] = scan;
          token = scan - 1;
          break;
        }
      }
      token = anchorToken + 1;
      for (let index = anchor + 1; index < fragmentWords.length; index += 1) {
        const norm = fragmentWords[index].norm;
        if (norm === "") continue;
        const highest = Math.min(tokens.length, token + ALIGN_WINDOW);
        for (let scan = token; scan < highest; scan += 1) {
          if (tokens[scan].norm !== norm) continue;
          map[index] = scan;
          token = scan + 1;
          break;
        }
      }
      return { map: map, score: mappedCount(map) };
    }

    /* Map one caption fragment onto the rendered tokens. Playback runs forward,
     * so the fragment starts at the last matched position; a fragment that
     * cannot be found stays unhighlighted. */
    function alignFragment(fragmentWords, tokens, from) {
      let anchor = -1;
      let expected = 0;
      for (let index = 0; index < fragmentWords.length; index += 1) {
        if (fragmentWords[index].norm === "") continue;
        expected += 1;
        if (anchor < 0) anchor = index;
      }
      if (anchor < 0) return null;
      const norm = fragmentWords[anchor].norm;
      let best = null;
      let scan = from;
      for (let attempt = 0; attempt < ALIGN_ATTEMPTS; attempt += 1) {
        let found = -1;
        for (let index = scan; index < tokens.length; index += 1) {
          if (tokens[index].norm !== norm) continue;
          found = index;
          break;
        }
        if (found < 0) break;
        const trial = matchAround(fragmentWords, tokens, anchor, found);
        if (best === null || trial.score > best.score) best = trial;
        if (best.score >= expected) break;
        if (best.score >= Math.ceil(expected * 0.8)) break;
        scan = found + 1;
      }
      if (best === null) return null;
      const required = expected <= 1 ? 1 : Math.max(2, Math.ceil(expected * 0.5));
      return best.score >= required ? best.map : null;
    }

    /* Provider caption entries are not words: ElevenLabs sends one entry per
     * character, other engines send whole words. Group the entries under the
     * words the reply renders, taking the first start and the last end each
     * word touches, so one window works for either granularity. */
    function captionWords(snapshot) {
      const words = [];
      for (const match of snapshot.text.matchAll(WORD_SPAN)) {
        const span = wordSpan(match[0]);
        const norm = normalizeWord(match[0].slice(span.start, span.end));
        if (norm === "") continue;
        /* Group by the whole whitespace run: a word-level provider reports a
           trailing punctuation mark with its word, so both granularities give
           the same window. */
        const start = match.index;
        const end = start + match[0].length;
        let startMs = Infinity;
        let endMs = -Infinity;
        for (const entry of snapshot.words) {
          if (entry.text_end <= start || entry.text_start >= end) continue;
          if (entry.text_end <= entry.text_start || entry.end_ms <= entry.start_ms) continue;
          if (entry.start_ms < startMs) startMs = entry.start_ms;
          if (entry.end_ms > endMs) endMs = entry.end_ms;
        }
        if (startMs === Infinity || endMs <= startMs) continue;
        words.push({ norm: norm, start_ms: startMs, end_ms: endMs });
      }
      return words;
    }

    function activeWordIndex(words, position) {
      for (let cursor = 0; cursor < words.length; cursor += 1) {
        if (position >= words[cursor].start_ms && position < words[cursor].end_ms) return cursor;
      }
      return -1;
    }

    function findScroller(element) {
      let node = element.parentElement;
      while (node !== null) {
        const style = window.getComputedStyle(node);
        if (
          (style.overflowY === "auto" || style.overflowY === "scroll") &&
          node.scrollHeight > node.clientHeight
        ) {
          return node;
        }
        node = node.parentElement;
      }
      return null;
    }

    /* One reply can render several Markdown bodies (text before and after a
     * tool call). Read them as one ordered word list, so a fragment that spans
     * two bodies still aligns. */
    function targetFrom(sources) {
      if (sources.length === 0) return null;
      const tokens = [];
      for (const source of sources) {
        for (const token of tokensFromElement(source)) tokens.push(token);
      }
      if (tokens.length === 0) return null;
      return {
        element: sources[0],
        tokens: tokens,
        score: overlapScore(tokens, reading.speechTokens)
      };
    }

    function rowSources(row) {
      const sources = [];
      for (const source of row.querySelectorAll(SOURCE_SELECTOR)) {
        /* Expanded reasoning carries its own source under the same row; the
           spoken reply must not align against reasoning words. */
        if (source.closest('[data-variant="think"]') !== null) continue;
        sources.push(source);
      }
      return sources;
    }

    /* Resolve the rendered reply. A node key is exact; without one, the DOM is
     * the only address and the closest assistant row wins. */
    function findMessageTarget() {
      const rows = document.querySelectorAll(ROW_SELECTOR);
      if (reading.nodeKey !== null) {
        /* The key is authoritative. A missing row means the reply is not on
           screen (another Session, or not rendered), so nothing is painted. */
        const sources = [];
        for (const row of rows) {
          if (row.getAttribute("data-chat-node-key") !== reading.nodeKey) continue;
          for (const source of rowSources(row)) sources.push(source);
        }
        const exact = targetFrom(sources);
        return exact !== null && exact.score > 0 ? exact : null;
      }
      /* Without a key the DOM is the only address, so score every assistant row. */
      let best = null;
      for (const row of rows) {
        const candidate = targetFrom(rowSources(row));
        if (candidate === null || candidate.score < 3) continue;
        if (best === null || candidate.score > best.score) best = candidate;
      }
      return best;
    }

    function ensureTarget() {
      if (reading.element !== null && reading.element.isConnected) return reading.element;
      const found = findMessageTarget();
      reading.element = found === null ? null : found.element;
      reading.tokens = found === null ? null : found.tokens;
      reading.scroller = null;
      reading.cursor = 0;
      reading.fragmentText = null;
      reading.fragmentWords = null;
      reading.fragmentMap = null;
      reading.activeToken = -1;
      return reading.element;
    }

    function ensureFragmentMap(snapshot) {
      if (reading.tokens === null) return null;
      /* Cache one alignment per fragment. An identical next fragment restarts
         the clock, so a position that jumps back re-anchors it. */
      const cached =
        reading.fragmentText === snapshot.text &&
        reading.fragmentEntries === snapshot.words.length &&
        reading.fragmentWords !== null &&
        reading.fragmentMap !== null &&
        snapshot.position >= reading.fragmentPosition - 300;
      if (cached) return reading.fragmentMap;
      const fragmentWords = captionWords(snapshot);
      const map = alignFragment(fragmentWords, reading.tokens, reading.cursor);
      reading.fragmentText = snapshot.text;
      reading.fragmentWords = fragmentWords;
      reading.fragmentEntries = snapshot.words.length;
      reading.fragmentMap = map;
      reading.fragmentPosition = snapshot.position;
      if (map === null) return null;
      let last = -1;
      for (const index of map) if (index > last) last = index;
      if (last >= 0) reading.cursor = last + 1;
      return map;
    }

    function clearHighlight() {
      if (window.CSS !== undefined && window.CSS.highlights !== undefined) {
        window.CSS.highlights.delete(HIGHLIGHT_NAME);
      }
      reading.activeToken = -1;
    }

    /* Resolve the scrollport of the painted word, and keep it while the word
     * stays inside it. */
    function ensureScroller(token) {
      const current = reading.scroller;
      if (current !== null && current.isConnected && current.contains(token.node)) return current;
      const element = token.node.parentElement;
      reading.scroller = element === null ? null : findScroller(element);
      return reading.scroller;
    }

    /* Bring the spoken word to the middle of the chat scrollport, but only
     * when it left the comfortable band, so reading does not nudge the view. */
    function scrollToRange(token, range) {
      const scroller = ensureScroller(token);
      if (scroller === null) return;
      const now = Date.now();
      if (now - reading.lastScrollAt < SCROLL_INTERVAL_MS) return;
      const rect = range.getBoundingClientRect();
      if (rect.width === 0 && rect.height === 0) return;
      const box = scroller.getBoundingClientRect();
      const top = rect.top - box.top;
      const bottom = rect.bottom - box.top;
      const margin = box.height * SCROLL_MARGIN;
      if (top >= margin && bottom <= box.height - margin) return;
      reading.lastScrollAt = now;
      const behavior = window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth";
      const offset = top + rect.height / 2 - box.height / 2;
      scroller.scrollTo({ top: scroller.scrollTop + offset, behavior: behavior });
    }

    function applyCaption(snapshot) {
      if (!preferences.captions || snapshot === null) {
        clearHighlight();
        return;
      }
      const element = ensureTarget();
      if (element === null || reading.tokens === null) {
        clearHighlight();
        return;
      }
      const map = ensureFragmentMap(snapshot);
      if (map === null) {
        clearHighlight();
        return;
      }
      const words = reading.fragmentWords === null ? [] : reading.fragmentWords;
      const wordIndex = activeWordIndex(words, snapshot.position);
      if (wordIndex < 0) {
        clearHighlight();
        return;
      }
      const tokenIndex = nearestMapped(map, wordIndex);
      if (tokenIndex < 0 || tokenIndex === reading.activeToken) return;
      const token = reading.tokens[tokenIndex];
      reading.activeToken = tokenIndex;
      const range = document.createRange();
      range.setStart(token.node, token.start);
      range.setEnd(token.node, token.end);
      if (window.CSS !== undefined && window.CSS.highlights !== undefined) {
        window.CSS.highlights.set(HIGHLIGHT_NAME, new window.Highlight(range));
      }
      scrollToRange(token, range);
    }

    function resetReading(messageId, nodeKey, speech) {
      reading.messageId = messageId;
      reading.nodeKey = nodeKey;
      reading.speechTokens = speech === "" ? [] : tokensFromText(speech);
      reading.element = null;
      reading.tokens = null;
      reading.cursor = 0;
      reading.fragmentText = null;
      reading.fragmentMap = null;
      reading.fragmentPosition = 0;
      reading.fragmentWords = null;
      reading.fragmentEntries = 0;
      reading.scroller = null;
      reading.activeToken = -1;
      reading.lastScrollAt = 0;
      clearHighlight();
    }

    const captionPoll = { timer: null, attempts: 0, sequence: 0 };

    function stopCaptionPolling() {
      if (captionPoll.timer !== null) clearInterval(captionPoll.timer);
      captionPoll.timer = null;
      captionPoll.attempts = 0;
      captionPoll.sequence = 0;
    }

    function startCaptionPolling(requestId) {
      stopCaptionPolling();
      if (!preferences.captions) return;
      function poll() {
        if (playback.requestId !== requestId) {
          stopCaptionPolling();
          return;
        }
        captionPoll.sequence += 1;
        const sequence = captionPoll.sequence;
        void readCaptionStatus().then(function (result) {
          /* Ignore a slower answer from an older poll. */
          if (playback.requestId !== requestId || sequence !== captionPoll.sequence) return;
          if (!result.reached) {
            clearHighlight();
            captionPoll.attempts += 1;
            if (captionPoll.attempts >= CAPTION_ATTEMPTS) stopCaptionPolling();
            return;
          }
          captionPoll.attempts = 0;
          applyCaption(result.caption);
        });
      }
      poll();
      captionPoll.timer = setInterval(poll, CAPTION_POLL_MS);
    }

    /* CopySpeak reports the caption it renders in its own webview. A missing
       route stops the polling; an inactive snapshot clears the highlight. */
    function readCaptionStatus() {
      return fetch(CONTROL_URL + "/status", { mode: "cors", cache: "no-store" })
        .then(function (response) {
          if (!response.ok) return { reached: false, caption: null };
          return response.json().then(function (body) {
            return { reached: true, caption: captionFromStatus(body) };
          });
        })
        .catch(function () {
          return { reached: false, caption: null };
        });
    }

    function captionFromStatus(body) {
      if (body === null || body === undefined || body.active !== true) return null;
      const source = body.captions;
      if (source === null || source === undefined) return null;
      if (!Array.isArray(source.words) || source.words.length === 0) return null;
      if (source.text === null || source.text === undefined || source.text === "") return null;
      if (body.position_ms === null || body.position_ms === undefined) return null;
      return {
        text: source.text,
        words: source.words,
        position: body.position_ms
      };
    }

    function post(path, body) {
      return fetch(CONTROL_URL + path, {
        method: "POST",
        mode: "no-cors",
        cache: "no-store",
        headers: { "content-type": "text/plain" },
        body: JSON.stringify(body)
      }).then(
        function () {
          return true;
        },
        function () {
          return false;
        }
      );
    }

    /* CopySpeak reachability: a no-cors GET rejects only when nothing listens. */
    function probeControlServer() {
      return fetch(CONTROL_URL + "/health", { mode: "no-cors", cache: "no-store" }).then(
        function () {
          return true;
        },
        function () {
          return false;
        }
      );
    }

    function stopReading(requestId) {
      return post("/stop", { request_id: requestId });
    }

    /* Start one reading and resolve with its failure code, or null. */
    function beginReading(messageId, nodeKey, speech) {
      const previous = playback.requestId;
      const stopping = previous === null ? Promise.resolve(true) : stopReading(previous);
      return stopping.then(function (stopped) {
        if (!stopped) {
          forget();
          return "error.offline";
        }
        const requestId = nextRequestId();
        resetReading(messageId, nodeKey, speech);
        remember(messageId, requestId);
        startCaptionPolling(requestId);
        return post("/speak", { text: speech, request_id: requestId, wait: true }).then(
          function (finished) {
            if (playback.requestId !== requestId) return null;
            forget();
            return finished ? null : "error.offline";
          }
        );
      });
    }

    function stopReadingNow() {
      const requestId = playback.requestId;
      if (requestId === null) {
        forget();
        return Promise.resolve();
      }
      return stopReading(requestId).then(function () {
        forget();
      });
    }

    /* Turn rendered Markdown into something a speech engine can read. */
    function toSpeech(markdown) {
      let text = markdown == null ? "" : String(markdown);
      text = text.replace(/```[\s\S]*?```/g, " ");
      text = text.replace(/`([^`]*)`/g, "$1");
      text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1");
      text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
      text = text.replace(/^ {0,3}#{1,6}\s*/gm, "");
      text = text.replace(/^ {0,3}>\s?/gm, "");
      text = text.replace(/^ {0,3}(?:[-*+]|\d+[.)])\s+/gm, "");
      text = text.replace(/^ {0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/gm, "");
      text = text.replace(/(\*\*|__)(.*?)\1/g, "$2");
      text = text.replace(/\*([^*\n]+)\*/g, "$1");
      text = text.replace(/~~(.*?)~~/g, "$1");
      text = text.replace(/<\/?[a-zA-Z][^>]*>/g, " ");
      text = text.replace(/\|/g, " ");
      text = text.replace(/[ \t]+\n/g, "\n");
      text = text.replace(/\n{3,}/g, "\n\n");
      return text.trim();
    }

    function assistantText(blocks) {
      if (!Array.isArray(blocks)) return "";
      const parts = [];
      for (const block of blocks) {
        if (block != null && block.kind === "text" && block.text != null) {
          parts.push(block.text);
        }
      }
      return parts.join("\n\n");
    }

    /* The Chat snapshot is the client's only source of settled message text:
       each settled assistant step carries its durable message id and blocks. */
    function assistantNode(snapshot, messageId) {
      if (snapshot == null || snapshot.nodes == null || snapshot.order == null) return null;
      const order = snapshot.order;
      for (let index = order.length - 1; index >= 0; index -= 1) {
        const node = snapshot.nodes.get(order[index]);
        if (node == null || node.kind !== "assistant-step" || node.data == null) continue;
        const finalNode = node.data.finalNode;
        if (finalNode == null || finalNode.messageId === undefined) continue;
        if (messageId !== undefined && finalNode.messageId !== messageId) continue;
        return node;
      }
      return null;
    }

    function CsGlyph(props) {
      const size = props.size ?? 18;
      return h(
        "svg",
        {
          viewBox: GLYPH_VIEW_BOX,
          width: size,
          height: size,
          className: "dsh-csva-glyph",
          "aria-hidden": true,
          focusable: "false"
        },
        GLYPH_PATHS.map(function (d, index) {
          return h("path", { key: index, d: d, fill: "currentColor" });
        })
      );
    }

    function CopySpeakPanelIcon(props) {
      return h(CsGlyph, { size: props.size });
    }

    function StopIcon() {
      return h(
        "svg",
        { viewBox: "0 0 24 24", "aria-hidden": true, focusable: "false" },
        h("rect", { x: 6, y: 6, width: 12, height: 12, rx: 2, fill: "currentColor" })
      );
    }

    function CopySpeakAction(props) {
      const messageId = props.messageId;
      const useChat = props.useChat;
      const t = props.t;
      /* The node key addresses the reply in the Chat flow, which the caption
         reader highlights; the node itself is kept out of the selector result
         so a streamed token does not allocate one. */
      const nodeRef = React.useRef(null);

      const text = useChat
        ? useChat(function (snapshot) {
            const node = assistantNode(snapshot, messageId);
            nodeRef.current = node;
            return node === null ? "" : assistantText(node.data.blocks);
          })
        : "";

      const [activeMessageId, setActiveMessageId] = React.useState(playback.messageId);
      const [busy, setBusy] = React.useState(false);
      const [failure, setFailure] = React.useState(null);
      const failureTimer = React.useRef(null);

      React.useEffect(function () {
        return subscribe(playback.listeners, function () {
          setActiveMessageId(playback.messageId);
        });
      }, []);

      React.useEffect(function () {
        return function () {
          if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        };
      }, []);

      function fail(key) {
        setFailure(key);
        if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        failureTimer.current = setTimeout(function () {
          setFailure(null);
        }, 4000);
      }

      const active = activeMessageId === messageId;
      const hasText = text.trim() !== "";
      const label = active ? t("action.stop") : t("action.read");
      /* A running reading must stay clickable even while its start is still
         waiting: the stop control is the same button. */
      const disabled = !active && (busy || !hasText);

      function onClick() {
        if (active) {
          setBusy(true);
          void stopReadingNow().then(function () {
            setBusy(false);
          });
          return;
        }
        if (busy) return;
        const speech = toSpeech(text);
        if (speech === "") {
          fail("error.empty");
          return;
        }
        const node = nodeRef.current;
        setBusy(true);
        void beginReading(messageId, node === null ? null : node.key, speech).then(
          function (error) {
            setBusy(false);
            if (error !== null) fail(error);
          }
        );
      }

      return h(
        "span",
        { className: "dsh-csva-root" },
        h(
          "button",
          {
            type: "button",
            className: "dsh-csva-button",
            "aria-label": label,
            "aria-pressed": active,
            title: label,
            "data-active": active || undefined,
            disabled: disabled,
            onClick: onClick
          },
          active ? h(StopIcon) : h(CsGlyph, { size: 15 })
        ),
        failure === null
          ? null
          : h("span", { className: "dsh-csva-note", role: "status" }, t(failure))
      );
    }

    /**
     * Per-session watcher under the composer: when a new reply settles and
     * auto-read is on, speak it. It renders nothing.
     */
    function AutoReader(props) {
      const useChat = props.useChat;
      const latest = React.useRef(null);
      const latestId = useChat
        ? useChat(function (snapshot) {
            const node = assistantNode(snapshot);
            latest.current =
              node === null
                ? null
                : {
                    messageId: node.data.finalNode.messageId,
                    key: node.key,
                    text: assistantText(node.data.blocks)
                  };
            return latest.current === null ? null : latest.current.messageId;
          })
        : null;

      const ready = React.useRef(false);
      const seen = React.useRef(null);

      React.useEffect(
        function () {
          if (!ready.current) {
            ready.current = true;
            seen.current = latestId;
            return;
          }
          if (latestId === null || latestId === seen.current) return;
          seen.current = latestId;
          if (!preferences.autoRead) return;
          const found = latest.current;
          if (found === null) return;
          /* The watcher reads the same rendered Markdown as the action, so it
             must strip the markup exactly the same way. */
          const speech = toSpeech(found.text);
          if (speech === "") return;
          void beginReading(found.messageId, found.key, speech).then(function (error) {
            if (error !== null) console.error("[copyspeak-voice] auto-read failed:", error);
          });
        },
        [latestId]
      );

      return null;
    }

    function Switch(props) {
      const label = props.label;
      return h(
        "button",
        {
          type: "button",
          role: "switch",
          className: "dsh-csva-switch",
          "aria-checked": props.checked,
          "aria-label": label,
          title: label,
          "data-on": props.checked || undefined,
          onClick: props.onChange
        },
        h("span", { className: "dsh-csva-switchThumb" })
      );
    }

    /** CopySpeak options and app status on the CopySpeak sidebar panel. */
    function CopySpeakScreen(props) {
      const t = props.t;
      const [autoRead, setAutoReadState] = React.useState(preferences.autoRead);
      const [captions, setCaptionsState] = React.useState(preferences.captions);
      const [online, setOnline] = React.useState(null);

      React.useEffect(function () {
        return subscribe(preferences.listeners, function () {
          setAutoReadState(preferences.autoRead);
          setCaptionsState(preferences.captions);
        });
      }, []);

      React.useEffect(function () {
        let cancelled = false;
        function probe() {
          void probeControlServer().then(function (reachable) {
            if (!cancelled) setOnline(reachable);
          });
        }
        probe();
        const timer = setInterval(probe, 10000);
        window.addEventListener("focus", probe);
        return function () {
          cancelled = true;
          clearInterval(timer);
          window.removeEventListener("focus", probe);
        };
      }, []);

      const statusKey =
        online === null ? "status.checking" : online ? "status.online" : "status.offline";
      const statusState = online === null ? "checking" : online ? "online" : "offline";

      return h(
        "div",
        { className: "dsh-csva-panel" },
        h(
          "header",
          { className: "dsh-csva-heading" },
          h("h1", { className: "dsh-csva-title" }, t("panel.title")),
          h("p", { className: "dsh-csva-intro" }, t("panel.intro"))
        ),
        h(
          "p",
          { className: "dsh-csva-status", "data-state": statusState, role: "status" },
          h("span", { className: "dsh-csva-dot" }),
          h("span", null, t(statusKey))
        ),
        h("ul", { className: "dsh-csva-rows" }, [
          h(
            "li",
            { key: "auto-read", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("autoRead.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("autoRead.description"))
            ),
            h(Switch, {
              checked: autoRead,
              label: t("autoRead.title") + ": " + t(autoRead ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("autoRead", !preferences.autoRead);
              }
            })
          ),
          h(
            "li",
            { key: "captions", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("captions.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("captions.description"))
            ),
            h(Switch, {
              checked: captions,
              label: t("captions.title") + ": " + t(captions ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("captions", !preferences.captions);
              }
            })
          )
        ])
      );
    }

    return {
      inject: ["slots", "locale"],
      apply(ctx) {
        ctx.effect(function () {
          const style = document.createElement("style");
          style.dataset.plugin = PACKAGE;
          style.dataset.pluginCss = STYLE_KEY;
          style.textContent = PLUGIN_STYLE;
          document.head.appendChild(style);
          return function () {
            style.remove();
          };
        }, PACKAGE + ": styles");

        ctx.effect(function () {
          return ctx.locale.register(NS, { en: en });
        }, PACKAGE + ": dictionaries");

        ctx.effect(function () {
          return function () {
            /* This plugin owns its reading; unloading it must stop the server
               playback, not just the timer. */
            const requestId = playback.requestId;
            stopCaptionPolling();
            if (requestId !== null) void stopReading(requestId);
            forget();
          };
        }, PACKAGE + ": captions");

        const t = ctx.locale.bind(NS);

        /* The sidebar entry owns the panel icon and resolves its label from the
           list metadata; the same id selects the `main` key. */
        ctx.slots.inject("sidebar.panellist", function () {
          return ctx.slots.register(
            {
              name: "sidebar.panellist",
              id: PANEL_ID,
              order: 20,
              label: function () {
                return t("panel.label");
              },
              locale: NS
            },
            CopySpeakPanelIcon
          );
        });

        ctx.slots.inject("main", function () {
          return ctx.slots.register(
            {
              name: "main",
              key: PANEL_ID,
              locale: NS
            },
            CopySpeakScreen
          );
        });

        ctx.slots.inject("conversation.chat.assistant-actions", function () {
          return ctx.slots.register(
            {
              name: "conversation.chat.assistant-actions",
              id: "copyspeak-voice",
              order: 20,
              locale: NS
            },
            CopySpeakAction
          );
        });

        /* The watcher lives in the session-scoped composer dock so it reads
           the session on screen; it renders nothing. */
        ctx.slots.inject("conversation.composer.dock", function () {
          return ctx.slots.register(
            {
              name: "conversation.composer.dock",
              id: "copyspeak-auto-reader",
              order: 90,
              locale: NS
            },
            AutoReader
          );
        });
      }
    };
  }
});
