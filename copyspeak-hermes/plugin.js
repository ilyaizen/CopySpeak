// CopySpeak auto-read for Hermes Desktop.
// Reads each new completed assistant reply through the running CopySpeak app.
// Optional: highlights the word CopySpeak speaks inside the reply (CSS Custom Highlight API).
import {
  PALETTE_AREA,
  Input,
  SegmentedControl,
  ToggleRow,
  atom,
  host,
  useQuery,
  useValue
} from "@hermes/plugin-sdk";
import { jsx, jsxs } from "react/jsx-runtime";

const ID = "copyspeak-hermes";
const SPEAK_URL = "http://127.0.0.1:43117/speak";
const HEALTH_URL = "http://127.0.0.1:43117/health";
const STATUS_URL = "http://127.0.0.1:43117/status";
const BUILT_IN_KEY = "hermes.desktop.autoSpeakReplies";
const MAX_CLAIMS = 50;
const EXCERPT_MIN = 1;
const EXCERPT_MAX = 20;
const MODES = ["top", "bottom", "both"];
const DEFAULTS = {
  autoRead: false,
  excerpt: true,
  excerptMode: "both",
  excerptTop: 2,
  excerptBottom: 2,
  captions: true
};
const CAPTION_NAME = "copyspeak-word";
const CAPTION_STYLE_ID = "copyspeak-hermes-captions";
const CAPTION_POLL_MS = 120;
const CAPTION_IDLE_MS = 2000;
const CAPTION_START_MS = 60000;
const ALIGN_WINDOW = 24;
const RUN_NEEDED = 3;
const REASONING_SELECTOR =
  '[data-slot="aui_reasoning-text"], [data-slot="aui_thinking-disclosure"], [data-slot="aui_thinking-body"]';

let requestCounter = 0;
let notifiedDown = false;
let captionRun = null;

export function clampCount(value) {
  const number = Math.round(Number(value));
  if (!Number.isFinite(number)) return EXCERPT_MIN;
  return Math.min(EXCERPT_MAX, Math.max(EXCERPT_MIN, number));
}

// Keep a boolean setting, or fall back to its default.
function boolOr(value, fallback) {
  return value === true || value === false ? value : fallback;
}

export function normalizeSettings(raw) {
  const source = raw ?? {};
  return {
    autoRead: boolOr(source.autoRead, DEFAULTS.autoRead),
    excerpt: boolOr(source.excerpt, DEFAULTS.excerpt),
    excerptMode: MODES.includes(source.excerptMode) ? source.excerptMode : DEFAULTS.excerptMode,
    excerptTop: clampCount(source.excerptTop ?? DEFAULTS.excerptTop),
    excerptBottom: clampCount(source.excerptBottom ?? DEFAULTS.excerptBottom),
    captions: boolOr(source.captions, DEFAULTS.captions)
  };
}

// Turn rendered Markdown into plain text for a speech engine.
export function toSpeech(markdown) {
  let text = markdown == null ? "" : String(markdown);
  text = text.replace(/```[\s\S]*?```/g, " ");
  text = text.replace(/`([^`]*)`/g, "$1");
  text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1");
  text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  text = text.replace(/^ {0,3}#{1,6}\s*/gm, "");
  text = text.replace(/^ {0,3}>\s?/gm, "");
  text = text.replace(/^ {0,3}(?:[-*+]|\d+[.)])\s+/gm, "");
  text = text.replace(/(\*\*|__)(.*?)\1/g, "$2");
  text = text.replace(/\*([^*\n]+)\*/g, "$1");
  text = text.replace(/<\/?[a-zA-Z][^>]*>/g, " ");
  text = text.replace(/\|/g, " ");
  text = text.replace(/[ \t]+\n/g, "\n");
  text = text.replace(/\n{3,}/g, "\n\n");
  return text.trim();
}

// Keep the first and last paragraphs of a reply. Paragraphs are blank-line blocks.
export function excerptSpeech(text, mode, topCount, bottomCount) {
  const blocks = text
    .split(/\n\s*\n/)
    .map((block) => block.trim())
    .filter((block) => block !== "");
  if (blocks.length < 2) return text;
  const top = clampCount(topCount);
  const bottom = clampCount(bottomCount);
  if (mode === "top") return blocks.slice(0, top).join("\n\n");
  if (mode === "bottom") return blocks.slice(-bottom).join("\n\n");
  if (top + bottom >= blocks.length) return text;
  return blocks.slice(0, top).concat(blocks.slice(-bottom)).join("\n\n");
}

export function prepareSpeech(text, settings) {
  const speech = toSpeech(text);
  if (!settings.excerpt) return speech;
  return excerptSpeech(speech, settings.excerptMode, settings.excerptTop, settings.excerptBottom);
}

export function claimKey(event) {
  return "claim." + event.session_id + "." + event.seq;
}

// Adds one claim and keeps only the newest MAX_CLAIMS entries.
export function withClaim(claims, key, now) {
  const next = { ...claims, [key]: now };
  const keys = Object.keys(next).sort((a, b) => next[a] - next[b]);
  for (const old of keys.slice(0, Math.max(0, keys.length - MAX_CLAIMS))) delete next[old];
  return next;
}

const BUILT_IN_REASON = "Hermes built-in auto-read is on";

// Runs the gates in order. Consumes the armed flag when the session passes gate 3.
export function speakVerdict(event, state) {
  const skip = (reason) => ({ speak: false, reason });
  if (!state.settings.autoRead) return skip("auto-read off");
  if (event.replayed === true) return skip("replayed event");
  if (!state.armed.has(event.session_id)) return skip("no message.start for this session");
  state.armed.delete(event.session_id);
  const payload = event.payload || {};
  if (payload.status !== "complete") return skip("status " + payload.status);
  if (event.session_id !== state.focusedSessionId) return skip("session not focused");
  if (!String(payload.text || "").trim()) return skip("empty text");
  if (Object.prototype.hasOwnProperty.call(state.claims, claimKey(event)))
    return skip("duplicate claim");
  if (state.builtInOn) return skip(BUILT_IN_REASON);
  return { speak: true, reason: "ok" };
}

export function shouldSpeak(event, state) {
  return speakVerdict(event, state).speak;
}

export function builtInAutoSpeakOn() {
  try {
    return localStorage.getItem(BUILT_IN_KEY) === "true";
  } catch {
    return false;
  }
}

// Words as letter or digit runs. Each word keeps its character range in the text.
export function wordTokens(text) {
  const tokens = [];
  for (const match of String(text).matchAll(/[\p{L}\p{N}]+/gu)) {
    tokens.push({
      norm: match[0].toLowerCase(),
      start: match.index,
      end: match.index + match[0].length
    });
  }
  return tokens;
}

// Group CopySpeak caption entries under each spoken word. ElevenLabs sends one
// entry per character; other engines send whole words. Both give one window per word.
export function captionWords(caption) {
  const entries = Array.isArray(caption.words) ? caption.words : [];
  const words = [];
  for (const token of wordTokens(caption.text)) {
    let startMs = Infinity;
    let endMs = -Infinity;
    for (const entry of entries) {
      if (!(entry.text_end > token.start && entry.text_start < token.end)) continue;
      if (entry.end_ms <= entry.start_ms) continue;
      startMs = Math.min(startMs, entry.start_ms);
      endMs = Math.max(endMs, entry.end_ms);
    }
    if (endMs > startMs) words.push({ norm: token.norm, start_ms: startMs, end_ms: endMs });
  }
  return words;
}

export function activeWordIndex(words, position) {
  return words.findIndex((word) => position >= word.start_ms && position < word.end_ms);
}

function findNear(tokens, norm, from) {
  const highest = Math.min(tokens.length, from + ALIGN_WINDOW);
  for (let scan = from; scan < highest; scan += 1) {
    if (tokens[scan].norm === norm) return scan;
  }
  return -1;
}

// True when the next `need` words follow word i in order from token `at`.
function runAt(words, tokens, i, at, need) {
  let cursor = at + 1;
  for (let k = i + 1; k < i + need; k += 1) {
    const found = findNear(tokens, words[k].norm, cursor);
    if (found < 0) return false;
    cursor = found + 1;
  }
  return true;
}

// Find word i far ahead of `from`. A run of words must follow it, so one common word cannot match alone.
// A fragment with fewer words left than RUN_NEEDED needs all of them; a single word never anchors.
function resync(words, tokens, i, from) {
  const need = Math.min(RUN_NEEDED, words.length - i);
  if (need < 2) return -1;
  for (let scan = from; scan < tokens.length; scan += 1) {
    if (tokens[scan].norm === words[i].norm && runAt(words, tokens, i, scan, need)) return scan;
  }
  return -1;
}

// Map each caption word to a rendered token, in order. Returns null when under half map.
// The first word of each fragment uses resync, so an excerpt gap cannot pull the anchor onto a common word.
export function alignWords(words, tokens, from) {
  const map = words.map(() => -1);
  let cursor = from;
  let mapped = 0;
  for (let i = 0; i < words.length; i += 1) {
    let at = mapped === 0 ? -1 : findNear(tokens, words[i].norm, cursor);
    if (at < 0) at = resync(words, tokens, i, cursor);
    if (at < 0) continue;
    map[i] = at;
    cursor = at + 1;
    mapped += 1;
  }
  if (mapped === 0 || mapped * 2 < words.length) return null;
  return { map: map, cursor: cursor };
}

function nextRequestId() {
  requestCounter += 1;
  return "hermes." + Date.now() + "." + requestCounter;
}

// Same transport as copyspeak-dsh: no-cors and text/plain avoid a preflight.
function postSpeech(text) {
  return fetch(SPEAK_URL, {
    method: "POST",
    mode: "no-cors",
    cache: "no-store",
    headers: { "content-type": "text/plain" },
    body: JSON.stringify({ text, request_id: nextRequestId(), wait: false })
  }).then(
    () => {
      notifiedDown = false;
    },
    (error) => {
      if (notifiedDown) return;
      notifiedDown = true;
      console.warn("[copyspeak-hermes] speak failed", error);
      host.notify({ kind: "error", message: "CopySpeak not reachable on 127.0.0.1:43117" });
    }
  );
}

async function probeCopySpeak() {
  try {
    await fetch(HEALTH_URL, { mode: "no-cors", cache: "no-store" });
    return true;
  } catch {
    return false;
  }
}

function captionsSupported() {
  return (
    window.CSS !== undefined &&
    window.CSS.highlights !== undefined &&
    window.Highlight !== undefined
  );
}

// Text nodes of the newest assistant reply, minus reasoning. Words keep their node and range.
function replyTokens() {
  const roots = document.querySelectorAll(
    '[data-slot="aui_assistant-message-root"][data-role="assistant"]'
  );
  const root = roots[roots.length - 1];
  const tokens = [];
  if (root === undefined) return tokens;
  for (const body of root.querySelectorAll(".aui-md")) {
    if (body.closest(REASONING_SELECTOR) !== null) continue;
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      for (const token of wordTokens(node.nodeValue ?? "")) tokens.push({ ...token, node: node });
    }
  }
  return tokens;
}

function clearCaptionPaint() {
  if (captionsSupported()) window.CSS.highlights.delete(CAPTION_NAME);
}

function stopCaptions() {
  if (captionRun === null) return;
  captionRun.dispose();
  captionRun = null;
  clearCaptionPaint();
}

// Paint the word at `position`. Returns true while the fragment is aligned, even on a held gap.
function paintCaption(run, caption, position, fragmentIndex) {
  if (caption.text == null || !Array.isArray(caption.words)) return false;
  const key = String(fragmentIndex) + "|" + caption.text;
  if (key !== run.key) {
    const tokens = replyTokens();
    if (tokens.length === 0) return false;
    run.key = key;
    run.words = captionWords(caption);
    run.tokens = tokens;
    const aligned = alignWords(run.words, tokens, run.cursor);
    run.map = aligned === null ? null : aligned.map;
    if (aligned !== null) run.cursor = aligned.cursor;
  }
  if (run.map === null) return false;
  const active = activeWordIndex(run.words, position);
  if (active < 0) return true;
  const token = run.tokens[run.map[active]];
  if (token === undefined || token.node === undefined) return true;
  if (!token.node.isConnected) {
    run.key = null;
    return false;
  }
  const range = document.createRange();
  range.setStart(token.node, token.start);
  range.setEnd(token.node, token.end);
  window.CSS.highlights.set(CAPTION_NAME, new window.Highlight(range));
  return true;
}

function onStatus(run, body) {
  if (captionRun !== run) return;
  const now = Date.now();
  const caption = body !== null && body.active === true ? body.captions : null;
  if (caption && paintCaption(run, caption, body.position_ms, body.fragment_index)) {
    run.lastMatch = now;
    run.matched = true;
    return;
  }
  const quiet = now - run.lastMatch;
  if (run.matched ? quiet >= CAPTION_IDLE_MS : now - run.startedAt >= CAPTION_START_MS)
    stopCaptions();
}

function pollCaptions(run) {
  if (run.busy) return;
  if (!run.enabled()) {
    stopCaptions();
    return;
  }
  run.busy = true;
  void fetch(STATUS_URL, { mode: "cors", cache: "no-store" })
    .then((response) => (response.ok ? response.json() : null))
    .catch(() => null)
    .then((body) => {
      run.busy = false;
      onStatus(run, body);
    });
}

// Start one poll loop for the reply just sent. A new reply replaces the old loop.
function startCaptions(ctx, enabled) {
  stopCaptions();
  if (!captionsSupported()) return;
  const now = Date.now();
  const run = {
    key: null,
    words: [],
    tokens: [],
    map: null,
    cursor: 0,
    busy: false,
    enabled: enabled,
    startedAt: now,
    lastMatch: now,
    matched: false,
    dispose: null
  };
  run.dispose = ctx.setInterval(() => pollCaptions(run), CAPTION_POLL_MS);
  captionRun = run;
}

function installCaptionStyle() {
  const old = document.getElementById(CAPTION_STYLE_ID);
  if (old !== null) old.remove();
  const style = document.createElement("style");
  style.id = CAPTION_STYLE_ID;
  style.textContent =
    "::highlight(" +
    CAPTION_NAME +
    ") { background-color: color-mix(in oklab, var(--ui-accent) 30%, transparent); }";
  document.head.appendChild(style);
  return style;
}

function readSettings(storage) {
  return normalizeSettings(storage.get("settings", DEFAULTS));
}

function readClaims(storage) {
  const raw = storage.get("claims", {});
  return raw ?? {};
}

function saveSettings(storage, settings, patch) {
  const next = normalizeSettings({ ...readSettings(storage), ...patch });
  storage.set("settings", next);
  settings.set(next);
  return next;
}

function SettingsPage({ storage, settings }) {
  const s = useValue(settings);
  const health = useQuery({
    queryKey: [ID, "health"],
    queryFn: probeCopySpeak,
    refetchInterval: 10000
  });
  const update = (patch) => saveSettings(storage, settings, patch);
  const builtIn = builtInAutoSpeakOn();
  const status =
    health.data === undefined
      ? "Checking CopySpeak…"
      : health.data
        ? "CopySpeak is running"
        : "CopySpeak not reachable on 127.0.0.1:43117";
  const showTop = s.excerptMode !== "bottom";
  const showBottom = s.excerptMode !== "top";

  return jsxs("div", {
    className: "flex flex-col gap-3 p-3 text-sm",
    children: [
      jsx("div", { className: "text-(--ui-text-secondary)", children: status }),
      jsx(ToggleRow, {
        label: "Read new replies aloud",
        checked: s.autoRead,
        onChange: (on) => update({ autoRead: on })
      }),
      builtIn
        ? jsx("div", {
            className: "text-(--ui-text-tertiary)",
            children:
              'Hermes\u2019s own "Read replies aloud" is on. Turn it off to avoid double playback.'
          })
        : null,
      jsx(ToggleRow, {
        label: "Highlight spoken words",
        checked: s.captions,
        onChange: (on) => update({ captions: on })
      }),
      jsx(ToggleRow, {
        label: "Read only part of the reply",
        checked: s.excerpt,
        onChange: (on) => update({ excerpt: on })
      }),
      s.excerpt
        ? jsx(SegmentedControl, {
            options: [
              { id: "top", label: "Top" },
              { id: "bottom", label: "Bottom" },
              { id: "both", label: "Top and bottom" }
            ],
            value: s.excerptMode,
            onChange: (id) => update({ excerptMode: id })
          })
        : null,
      s.excerpt && showTop
        ? jsxs("div", {
            className: "flex items-center justify-between gap-3",
            children: [
              jsx("span", { children: "Top paragraphs" }),
              jsx(Input, {
                type: "number",
                min: EXCERPT_MIN,
                max: EXCERPT_MAX,
                value: s.excerptTop,
                onChange: (e) => update({ excerptTop: Number(e.target.value) })
              })
            ]
          })
        : null,
      s.excerpt && showBottom
        ? jsxs("div", {
            className: "flex items-center justify-between gap-3",
            children: [
              jsx("span", { children: "Bottom paragraphs" }),
              jsx(Input, {
                type: "number",
                min: EXCERPT_MIN,
                max: EXCERPT_MAX,
                value: s.excerptBottom,
                onChange: (e) => update({ excerptBottom: Number(e.target.value) })
              })
            ]
          })
        : null
    ]
  });
}

export default {
  id: ID,
  name: "CopySpeak",
  description: "Reads new completed Hermes replies aloud through the running CopySpeak app.",
  register(ctx) {
    const storage = ctx.storage;
    const settings = atom(readSettings(storage));
    const armed = new Set();
    const style = installCaptionStyle();
    ctx.onDispose(() => {
      stopCaptions();
      style.remove();
    });

    ctx.onEvent("message.start", (event) => {
      if (event && event.session_id) armed.add(event.session_id);
    });

    ctx.onEvent("message.complete", (event) => {
      const current = readSettings(storage);
      settings.set(current);
      const focusAtom =
        host.state.focusedSessionId !== undefined
          ? host.state.focusedSessionId
          : host.state.activeSessionId;
      const verdict = speakVerdict(event, {
        settings: current,
        focusedSessionId: focusAtom.get(),
        armed,
        claims: readClaims(storage),
        builtInOn: builtInAutoSpeakOn()
      });
      if (!verdict.speak) {
        if (verdict.reason === BUILT_IN_REASON) {
          console.info("[copyspeak-hermes] skipped reply: Hermes built-in auto-read is on");
        }
        return;
      }
      const speech = prepareSpeech(String(event.payload.text || ""), current);
      if (!speech) return;
      storage.set("claims", withClaim(readClaims(storage), claimKey(event), Date.now()));
      void postSpeech(speech);
      if (current.captions) startCaptions(ctx, () => readSettings(storage).captions);
    });

    ctx.register({
      id: "toggle-auto-read",
      area: PALETTE_AREA,
      data: {
        id: ID + ".toggle-auto-read",
        label: "CopySpeak: Toggle auto-read",
        keywords: ["copyspeak", "speak", "read", "aloud", "tts"],
        run: () => {
          const next = !readSettings(storage).autoRead;
          saveSettings(storage, settings, { autoRead: next });
          host.notify({
            kind: "info",
            message: next ? "CopySpeak auto-read is on" : "CopySpeak auto-read is off"
          });
        }
      }
    });

    ctx.registerSettingsPage({
      id: ID,
      title: "CopySpeak",
      render: () => jsx(SettingsPage, { storage, settings })
    });

    console.info("[copyspeak-hermes] registered");
  }
};
