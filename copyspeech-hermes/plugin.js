// CopySpeech auto-read for Hermes Desktop.
// Reads each new completed assistant reply through the running CopySpeech app.
// Manual control: a play button on every reply (hover its action row), a
// "Read aloud" pill on text selected inside a reply, and a playback bar above
// the chat input (pause, resume, back and forward 5 seconds, stop, show in chat) for any
// reading this plugin started, automatic ones included.
// Optional: highlights the word CopySpeech speaks inside the reply (CSS Custom Highlight API,
// or a gliding pill when "Glide between words" is on). Manual reads paint too.
// Optional: follows the spoken word, scrolling the reply to keep it in view.
import {
  PALETTE_AREA,
  Button,
  Input,
  SegmentedControl,
  ToggleRow,
  atom,
  host,
  useQuery,
  useValue
} from "@hermes/plugin-sdk";
import { useEffect, useState } from "react";
import { jsx, jsxs } from "react/jsx-runtime";

const ID = "copyspeech-hermes";
// ctx.storage keys live under hermes.plugin.<id>. (localStorage), so this is the key for "settings".
const SETTINGS_KEY = "hermes.plugin." + ID + ".settings";
const CONTROL_URL = "http://127.0.0.1:43117";
const SPEAK_URL = CONTROL_URL + "/speak";
const HEALTH_URL = CONTROL_URL + "/health";
const STATUS_URL = CONTROL_URL + "/status";
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
  captions: true,
  glide: false,
  follow: false,
  messageButtons: true,
  selectionRead: true,
  skipMarkdownSections: false
};
const CAPTION_NAME = "copyspeech-word";
const CAPTION_STYLE_ID = "copyspeech-hermes-captions";
const CAPTION_POLL_MS = 120;
const CAPTION_SILENCE_MS = 60000;
// Follow keeps the spoken word between these fractions of the reply's scrollport.
// Outside the band, its line moves to the anchor. Manual input pauses follow.
const FOLLOW_BAND_TOP = 0.15;
const FOLLOW_BAND_BOTTOM = 0.82;
const FOLLOW_ANCHOR = 0.34;
const FOLLOW_INTERVAL_MS = 400;
const FOLLOW_PAUSE_MS = 5000;
const FOLLOW_KEYS = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"]);
const ALIGN_WINDOW = 24;
const RUN_NEEDED = 3;
const REASONING_SELECTOR =
  '[data-slot="aui_reasoning-text"], [data-slot="aui_thinking-disclosure"], [data-slot="aui_thinking-body"]';
// Glide pill: the glide duration, the per-frame cap, the padding around a word, and the look.
const GLIDE_MS = 120;
const GLIDE_FRAME_CAP_MS = 40;
const PILL_PAD_X = 4;
const PILL_PAD_Y = 2;
const PILL_RADIUS_PX = 4;
const PILL_Z_INDEX = 2147483646;
// Any overflow other than `visible` clips a box's content.
const CLIP_OVERFLOW = ["auto", "scroll", "hidden", "clip"];

// ---- Playback controls: one shared reading, three ways to start it ----
// The message play button, the selection read, and auto-read all register the
// same request here. The bar and the buttons observe the record and repaint.
const SEEK_STEP_MS = 5000;
// An optimistic pause survives this long before a poll may override it, so a
// poll that left before the app applied the command cannot flicker the glyph.
const PAUSE_RECONCILE_MS = 350;
const MESSAGE_ROOT_SELECTOR = '[data-slot="aui_assistant-message-root"][data-role="assistant"]';
const MESSAGE_BODY_SELECTOR = ".aui-md";
const MESSAGE_ACTIONS_SELECTOR = ".aui-message-actions-desktop";
const PLAY_BUTTON_FLAG = "data-copyspeech-play";
const UI_STYLE_ID = "copyspeech-hermes-ui";

let playbackRequestId = null;
// "auto" | "message" | "selection" — which surface owns the live reading.
let playbackSource = null;
let playbackRoot = null;
let playbackRange = null;
let playbackPaused = false;
let playbackPauseChangedAt = 0;
let playbackStopEpoch = 0;
let playbackSpeakAbort = null;
const playbackListeners = new Set();

function playbackEmit() {
  for (const listener of Array.from(playbackListeners)) listener();
  updateBar();
  syncMessageButtons();
}

function playbackRemember(source, requestId, root, range) {
  playbackRequestId = requestId;
  playbackSource = source;
  playbackRoot = root ?? null;
  playbackRange = range ?? null;
  playbackPaused = false;
  playbackPauseChangedAt = 0;
  playbackEmit();
}

function playbackForget() {
  playbackRequestId = null;
  playbackSource = null;
  playbackRoot = null;
  playbackRange = null;
  playbackPaused = false;
  playbackPauseChangedAt = 0;
  playbackSpeakAbort = null;
  stopCaptions();
  playbackEmit();
}

function playbackSetPaused(paused) {
  if (playbackPaused === paused) return;
  playbackPaused = paused;
  playbackEmit();
}

// Same transport as before: no-cors and text/plain avoid a preflight.
function controlPost(path, body, signal) {
  return fetch(CONTROL_URL + path, {
    method: "POST",
    mode: "no-cors",
    cache: "no-store",
    headers: { "content-type": "text/plain" },
    body: JSON.stringify(body),
    signal: signal
  }).then(
    () => true,
    () => false
  );
}

function stopReading(requestId) {
  return controlPost("/stop", { request_id: requestId });
}

// Pause and resume address the live reading by request id. The glyph flips at
// once when a caption poll is running to confirm it; without a poll there is
// nothing to reconcile against, so the request goes out alone.
function pauseReading() {
  const requestId = playbackRequestId;
  if (requestId === null) return Promise.resolve();
  const polled = captionRun !== null && captionRun.requestId === requestId;
  if (polled) {
    playbackSetPaused(true);
    playbackPauseChangedAt = Date.now();
  }
  return controlPost("/pause", { request_id: requestId });
}

function resumeReading() {
  const requestId = playbackRequestId;
  if (requestId === null) return Promise.resolve();
  const polled = captionRun !== null && captionRun.requestId === requestId;
  if (polled) {
    playbackSetPaused(false);
    playbackPauseChangedAt = Date.now();
  }
  return controlPost("/resume", { request_id: requestId });
}

// Rewind or skip one reading. Seek stays available while paused: the app moves
// the position, and a later resume plays from it.
function seekReading(deltaMs) {
  const requestId = playbackRequestId;
  if (requestId === null) return Promise.resolve();
  return controlPost("/seek", { request_id: requestId, delta_ms: deltaMs });
}

// Stop the live reading and clear every surface at once. The epoch bump also
// cancels a reading still queued behind an earlier start.
function stopReadingNow() {
  const requestId = playbackRequestId;
  playbackStopEpoch += 1;
  if (playbackSpeakAbort !== null) {
    playbackSpeakAbort.abort();
    playbackSpeakAbort = null;
  }
  if (requestId === null) {
    playbackForget();
    return Promise.resolve();
  }
  return stopReading(requestId).then(() => playbackForget());
}

let requestCounter = 0;
let notifiedDown = false;
let captionRun = null;
let pill = null;

// One offline toast until a request succeeds again.
function notifyOffline() {
  if (notifiedDown) return;
  notifiedDown = true;
  host.notify({ kind: "error", message: "CopySpeech not reachable on 127.0.0.1:43117" });
}

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
    captions: boolOr(source.captions, DEFAULTS.captions),
    glide: boolOr(source.glide, DEFAULTS.glide),
    follow: boolOr(source.follow, DEFAULTS.follow),
    messageButtons: boolOr(source.messageButtons, DEFAULTS.messageButtons),
    selectionRead: boolOr(source.selectionRead, DEFAULTS.selectionRead),
    skipMarkdownSections: boolOr(source.skipMarkdownSections, DEFAULTS.skipMarkdownSections)
  };
}

// Drop whole Markdown lines that speech reads badly: headings, bullet and
// numbered items, horizontal rules, and table rows. The text under a heading
// stays, so an answer under "## Answer" still reads. Runs before toSpeech.
export function dropSkippedMarkup(markdown) {
  const skipped = /^\s{0,3}(?:#{1,6}\s|[-*+]\s|\d+[.)]\s|\|| ?([-*_]) ?(?:\1 ?){2,}$)/;
  return String(markdown ?? "")
    .split("\n")
    .filter((line) => !skipped.test(line))
    .join("\n");
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

function prepareSpeech(text, settings) {
  const speech = toSpeech(settings.skipMarkdownSections ? dropSkippedMarkup(text) : text);
  if (!settings.excerpt) return speech;
  return excerptSpeech(speech, settings.excerptMode, settings.excerptTop, settings.excerptBottom);
}

// Message reads and auto-read follow the excerpt settings. A selection is the user's own pick, so it reads in full.
function speechForRead(source, text, settings) {
  if (source === "selection") return text;
  return prepareSpeech(text, settings);
}

function claimKey(event) {
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

function builtInAutoSpeakOn() {
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

// The norms of a speech text, for the code-split merge on selection reads.
function wordNorms(text) {
  return new Set(wordTokens(text).map((word) => word.norm));
}

// Lowercased letters and digits only, so punctuation and wrapping do not
// split a match between the spoken text and the rendered text.
function normalizeWord(raw) {
  return String(raw)
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "");
}

const SELECTION_WORD = /\S+/gu;
const SELECTION_WORD_CHAR = /[\p{L}\p{N}]/u;

// The first and last letter or digit of a whitespace-free run.
function wordSpan(raw) {
  let start = 0;
  let end = raw.length;
  while (start < end && !SELECTION_WORD_CHAR.test(raw[start])) start += 1;
  while (end > start && !SELECTION_WORD_CHAR.test(raw[end - 1])) end -= 1;
  return { start: start, end: end };
}

// The words inside one cloned selection range, in document order. Code tokens
// carry a flag, because syntax highlighters split one word across text nodes.
function tokensFromRange(range, speechNorms) {
  const tokens = [];
  if (range === null) return tokens;
  const root = range.commonAncestorContainer;
  if (root === null || !root.isConnected) return tokens;
  const area = root.nodeType === 3 ? root.parentElement : root;
  if (area === null) return tokens;
  const walker = document.createTreeWalker(area, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    if (!range.intersectsNode(node)) continue;
    const text = node.nodeValue ?? "";
    const first = node === range.startContainer ? range.startOffset : 0;
    const last = node === range.endContainer ? range.endOffset : text.length;
    if (last <= first) continue;
    const inCode = node.parentElement !== null && node.parentElement.closest("code, pre") !== null;
    const slice = text.slice(first, last);
    for (const match of slice.matchAll(SELECTION_WORD)) {
      const norm = normalizeWord(match[0]);
      if (norm === "") continue;
      const span = wordSpan(match[0]);
      const base = first + match.index;
      tokens.push({
        node: node,
        start: base + span.start,
        end: base + span.end,
        norm: norm,
        code: inCode
      });
    }
  }
  return mergeCodeSplits(tokens, speechNorms);
}

// Syntax highlighters split one code word across several text nodes, so the
// per-node tokens never match the spoken word ("keep" + "_mark" vs "keepmark").
// Merge adjacent code tokens while their concatenated norm exists in the
// spoken stream: the merge can only create a spoken match, never break one.
function mergeCodeSplits(tokens, speechNorms) {
  if (speechNorms === undefined || speechNorms === null || speechNorms.size === 0) {
    return tokens;
  }
  const out = [];
  let index = 0;
  while (index < tokens.length) {
    const first = tokens[index];
    if (first.code !== true) {
      out.push(first);
      index += 1;
      continue;
    }
    let merged = first;
    let cursor = index + 1;
    while (cursor < tokens.length) {
      const next = tokens[cursor];
      if (next.code !== true) break;
      const joined = merged.norm + next.norm;
      if (!speechNorms.has(joined)) break;
      merged = {
        node: first.node,
        start: first.start,
        end: next.end,
        norm: joined,
        code: true
      };
      cursor += 1;
    }
    out.push(merged);
    index = cursor;
  }
  return out;
}

// Group CopySpeech caption entries under each spoken word. ElevenLabs sends one
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

// Map each caption word to a rendered token, in order. Reports the counts even on failure.
// The first word of each fragment uses resync, so an excerpt gap cannot pull the anchor onto a common word.
function alignDetail(words, tokens, from) {
  const map = words.map(() => -1);
  let cursor = from;
  let mapped = 0;
  for (let i = 0; i < words.length; i += 1) {
    let at = mapped === 0 ? -1 : findNear(tokens, words[i].norm, cursor);
    // A near match anchors only when the following words follow it. A common
    // word inside a skipped excerpt gap fails this check, so resync finds the
    // real run. With no later run, keep the near match (TTS may respell a word).
    if (at >= 0 && !runAt(words, tokens, i, at, Math.min(RUN_NEEDED, words.length - i))) {
      const far = resync(words, tokens, i, cursor);
      if (far >= 0) at = far;
    }
    if (at < 0) at = resync(words, tokens, i, cursor);
    if (at < 0) continue;
    map[i] = at;
    cursor = at + 1;
    mapped += 1;
  }
  const ok = mapped > 0 && mapped * 2 >= words.length;
  return { ok: ok, map: map, cursor: cursor, mapped: mapped };
}

function nextRequestId() {
  requestCounter += 1;
  return "hermes." + Date.now() + "." + requestCounter;
}

// Same transport as copyspeech-dsh: no-cors and text/plain avoid a preflight.
// A failure also clears the playback record, so the bar never hangs on a dead
// request.
function postSpeech(text, requestId) {
  return fetch(SPEAK_URL, {
    method: "POST",
    mode: "no-cors",
    cache: "no-store",
    headers: { "content-type": "text/plain" },
    body: JSON.stringify({ text, request_id: requestId, wait: false })
  }).then(
    () => {
      notifiedDown = false;
    },
    (error) => {
      console.warn("[copyspeech-hermes] speak failed", error);
      notifyOffline();
      if (playbackRequestId === requestId) playbackForget();
    }
  );
}

async function probeCopySpeech() {
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

// The session the chat shows. Hosts without focusedSessionId fall back to activeSessionId.
function focusedSession() {
  const atom =
    host.state.focusedSessionId !== undefined
      ? host.state.focusedSessionId
      : host.state.activeSessionId;
  return atom.get();
}

// The chat pane the user works in. Hermes marks each chat pane with data-session-anchor:
// `session-tile:<storedId>` for a tile, `workspace` for the primary. A focused tile has
// focusedStoredSessionId = its storedId; otherwise that id is the primary's selection.
// With no anchors at all (an older Hermes), the whole document is the scope.
// Null when anchors exist but none matches, so no caption paints in the wrong chat.
function focusedPane() {
  if (document.querySelectorAll("[data-session-anchor]").length === 0) return document;
  const stored = host.state.focusedStoredSessionId?.get?.() ?? null;
  const tile = stored ? document.querySelector(anchorSelector("session-tile:" + stored)) : null;
  return tile ?? document.querySelector(anchorSelector("workspace")) ?? null;
}

function anchorSelector(anchor) {
  return '[data-session-anchor="' + String(anchor).replace(/["\\]/g, "\\$&") + '"]';
}

// The newest assistant reply in the focused pane.
function assistantRoot() {
  const scope = focusedPane();
  if (scope === null) return undefined;
  const roots = scope.querySelectorAll(
    '[data-slot="aui_assistant-message-root"][data-role="assistant"]'
  );
  return roots[roots.length - 1];
}

// Text nodes of a reply root, minus reasoning and code blocks. Words keep
// their node and range. Code blocks are skipped for the same reason the
// spoken text skips them: CopySpeech never reads them, so a code word must
// not pull the highlight.
function rootTokens(root, skipCode) {
  const tokens = [];
  if (root === undefined || root === null) return tokens;
  for (const body of root.querySelectorAll(MESSAGE_BODY_SELECTOR)) {
    if (body.closest(REASONING_SELECTOR) !== null) continue;
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      if (skipCode && node.parentElement !== null && node.parentElement.closest("pre") !== null) {
        continue;
      }
      for (const token of wordTokens(node.nodeValue ?? "")) tokens.push({ ...token, node: node });
    }
  }
  return tokens;
}

// Text nodes of the newest assistant reply, minus reasoning.
function replyTokens() {
  return rootTokens(assistantRoot(), true);
}

// The rendered tokens a caption run paints against: the newest reply for
// auto-read, one specific reply for a message read, or the cloned selection
// range for a selection read.
function targetTokens(run) {
  const target = run.target;
  if (target === undefined || target === null) return replyTokens();
  if (target.kind === "selection") return tokensFromRange(target.range, target.speechNorms);
  if (target.kind === "message") return rootTokens(target.root, true);
  return replyTokens();
}

function clearCaptionPaint() {
  if (captionsSupported()) window.CSS.highlights.delete(CAPTION_NAME);
}

function tokenRange(token) {
  const range = document.createRange();
  range.setStart(token.node, token.start);
  range.setEnd(token.node, token.end);
  return range;
}

function liveStyle(el) {
  return window.getComputedStyle(el);
}

// The visible area, in viewport coordinates.
function liveViewport() {
  const root = document.documentElement;
  return { left: 0, top: 0, right: root.clientWidth, bottom: root.clientHeight };
}

// Browser hooks for the glide pill. The pill itself touches no globals.
function livePillEnv() {
  return {
    make: function () {
      return document.createElement("div");
    },
    attach: function (el) {
      document.documentElement.appendChild(el);
    },
    measure: function (range) {
      return measureWord(range, liveStyle, liveViewport());
    },
    clip: function (range) {
      return clipBox(range.startContainer, liveStyle, liveViewport());
    },
    reduced: function () {
      return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    },
    raf: function (fn) {
      return window.requestAnimationFrame(fn);
    },
    cancelRaf: function (id) {
      window.cancelAnimationFrame(id);
    },
    listen: function (type, fn) {
      window.addEventListener(type, fn, { capture: true, passive: true });
      return function () {
        window.removeEventListener(type, fn, true);
      };
    }
  };
}

function ensurePill() {
  if (pill === null) pill = createGlidePill(livePillEnv());
  return pill;
}

function destroyPill() {
  if (pill === null) return;
  pill.destroy();
  pill = null;
}

function stopCaptions() {
  if (captionRun !== null) {
    captionRun.dispose();
    captionRun = null;
  }
  destroyPill();
  clearCaptionPaint();
}

// Scroll behaviour for the reply: smooth, or an instant jump when the OS asks for reduced motion.
// "instant" overrides the page's CSS scroll-behavior; "auto" would keep a page's smooth default.
function motionBehavior() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth";
}

// The nearest ancestor that scrolls its content vertically, or null.
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

// Listen for manual input on the run's scroller. A wheel, a scroll key, any touch inside it
// (a tap on a word too, since a touch can start a scroll), or a press on the scroller's own
// box (its scrollbar or background that no child covers) pauses follow. A mouse press on a
// word does not.
function watchScroller(run, scroller) {
  if (run.watched === scroller) return;
  unwatchScroller(run);
  run.watched = scroller;
  scroller.addEventListener("wheel", run.pause, { passive: true });
  scroller.addEventListener("touchstart", run.pause, { passive: true });
  scroller.addEventListener("keydown", run.pauseKey, true);
  scroller.addEventListener("pointerdown", run.pauseScrollbar, { passive: true });
}

function unwatchScroller(run) {
  const scroller = run.watched;
  if (scroller === null) return;
  scroller.removeEventListener("wheel", run.pause);
  scroller.removeEventListener("touchstart", run.pause);
  scroller.removeEventListener("keydown", run.pauseKey, true);
  scroller.removeEventListener("pointerdown", run.pauseScrollbar);
  run.watched = null;
}

// The scroller that holds the word. Keeps the current one while the word stays inside it.
function ensureScroller(run, token) {
  const current = run.scroller;
  if (current !== null && current.isConnected && current.contains(token.node)) {
    // Re-attach on reuse too. watchScroller is a no-op when the listeners are already attached.
    watchScroller(run, current);
    return current;
  }
  const element = token.node.parentElement;
  run.scroller = element === null ? null : findScroller(element);
  if (run.scroller === null) unwatchScroller(run);
  else watchScroller(run, run.scroller);
  return run.scroller;
}

// True while a text selection in the reply starts or ends inside the scroller. Follow then
// leaves the view alone, so a reader's drag-select is not fought.
function selectingIn(scroller) {
  const selection = window.getSelection?.();
  if (!selection || selection.isCollapsed) return false;
  return [selection.anchorNode, selection.focusNode].some(
    (node) => node !== null && node !== undefined && scroller.contains(node)
  );
}

// Scroll only when the spoken word leaves the comfort band, so most words move nothing.
function followToken(run, token) {
  if (!run.follow) return;
  const now = Date.now();
  if (now < run.followPausedUntil || now - run.lastScrollAt < FOLLOW_INTERVAL_MS) return;
  const scroller = ensureScroller(run, token);
  if (scroller === null || selectingIn(scroller)) return;
  const rect = firstPaintedRect(tokenRange(token));
  if (rect === undefined) return;
  const box = scroller.getBoundingClientRect();
  const top = rect.top - box.top;
  const bottom = top + rect.height;
  if (top >= box.height * FOLLOW_BAND_TOP && bottom <= box.height * FOLLOW_BAND_BOTTOM) return;
  run.lastScrollAt = now;
  const offset = top - box.height * FOLLOW_ANCHOR;
  scroller.scrollTo({ top: scroller.scrollTop + offset, behavior: motionBehavior() });
}

// One warning per failed alignment: shows whether the spoken words or the rendered tokens were short.
function warnNoAlignment(spoken, rendered, mapped, reason) {
  console.warn(
    "[copyspeech-hermes] captions: no alignment (" +
      spoken +
      " spoken, " +
      rendered +
      " rendered, " +
      mapped +
      " mapped, reason=" +
      reason +
      ")"
  );
}

// Linear blend of two rects. `p` runs from 0 (at `a`) to 1 (at `b`).
function lerpRect(a, b, p) {
  return {
    left: a.left + (b.left - a.left) * p,
    top: a.top + (b.top - a.top) * p,
    width: a.width + (b.width - a.width) * p,
    height: a.height + (b.height - a.height) * p
  };
}

// CSS cubic-bezier(x1, y1, x2, y2) as an easing function over [0, 1].
function cubicBezier(x1, y1, x2, y2) {
  const axis = function (a1, a2, t) {
    return ((1 - 3 * a2 + 3 * a1) * t + (3 * a2 - 6 * a1)) * t * t + 3 * a1 * t;
  };
  return function (x) {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let low = 0;
    let high = 1;
    let t = x;
    for (let i = 0; i < 20; i += 1) {
      t = (low + high) / 2;
      if (axis(x1, x2, t) < x) low = t;
      else high = t;
    }
    return axis(y1, y2, t);
  };
}

// Swift start, then settle. The same curve as the DSH pill.
const easeGlide = cubicBezier(0, 0.5, 0, 1);

// Overflow clips a box's content, but an inline box does not clip, so it is skipped.
function clipsContent(style) {
  if (style.display === "inline") return false;
  return CLIP_OVERFLOW.includes(style.overflowX) || CLIP_OVERFLOW.includes(style.overflowY);
}

// The area an element's overflow clips to: its padding box, without scrollbars.
function paddingBox(el) {
  const box = el.getBoundingClientRect();
  const left = box.left + el.clientLeft;
  const top = box.top + el.clientTop;
  return { left: left, top: top, right: left + el.clientWidth, bottom: top + el.clientHeight };
}

// The overlap of two boxes. Null when they do not overlap.
function overlapBox(a, b) {
  const box = {
    left: Math.max(a.left, b.left),
    top: Math.max(a.top, b.top),
    right: Math.min(a.right, b.right),
    bottom: Math.min(a.bottom, b.bottom)
  };
  if (box.right <= box.left || box.bottom <= box.top) return null;
  return box;
}

// The viewport cut by every ancestor whose overflow clips `node`. Null when nothing is left.
function clipBox(node, styleOf, viewport) {
  let box = viewport;
  for (let el = node.parentElement; el !== null && box !== null; el = el.parentElement) {
    if (clipsContent(styleOf(el))) box = overlapBox(box, paddingBox(el));
  }
  return box;
}

// Cut a {left, top, width, height} rect to a clip box. Null when nothing is left.
function cutRect(rect, clip) {
  if (clip === null) return null;
  const left = Math.max(rect.left, clip.left);
  const top = Math.max(rect.top, clip.top);
  const right = Math.min(rect.left + rect.width, clip.right);
  const bottom = Math.min(rect.top + rect.height, clip.bottom);
  if (right <= left || bottom <= top) return null;
  return { left: left, top: top, width: right - left, height: bottom - top };
}

// The first client rect of a range that has a size. Undefined when none does.
function firstPaintedRect(range) {
  let rects = [];
  try {
    rects = Array.from(range.getClientRects());
  } catch {
    rects = [];
  }
  return rects.find(function (r) {
    return r.width > 0 && r.height > 0;
  });
}

// The padded box of a word's first painted client rect, cut to every clipping ancestor
// and the viewport. Returns null when no part of the word is visible.
export function measureWord(range, styleOf, viewport) {
  const rect = firstPaintedRect(range);
  if (rect === undefined) return null;
  const padded = {
    left: rect.left - PILL_PAD_X,
    top: rect.top - PILL_PAD_Y,
    width: rect.width + 2 * PILL_PAD_X,
    height: rect.height + 2 * PILL_PAD_Y
  };
  return cutRect(padded, clipBox(range.startContainer, styleOf, viewport));
}

// The glide pill: one fixed div over the spoken word, moved by one rAF loop.
// The loop runs only while the pill moves. It re-reads the word's rect on every frame,
// so scroll and layout changes during a glide land on the word.
// `env` supplies the browser hooks: make, attach, measure, reduced, raf, cancelRaf, listen.
export function createGlidePill(env) {
  let el = null;
  let range = null;
  let drawn = null;
  let from = null;
  let frame = 0;
  let elapsed = 0;
  let last = null;
  let offs = [];

  function target() {
    return range === null ? null : env.measure(range);
  }

  function draw(rect) {
    drawn = rect;
    if (el === null) return;
    if (rect === null) {
      el.style.display = "none";
      return;
    }
    el.style.display = "block";
    el.style.left = rect.left + "px";
    el.style.top = rect.top + "px";
    el.style.width = rect.width + "px";
    el.style.height = rect.height + "px";
  }

  function stopLoop() {
    if (frame !== 0) env.cancelRaf(frame);
    frame = 0;
  }

  function tick(ts) {
    frame = 0;
    if (el === null || range === null || !range.startContainer.isConnected) {
      destroy();
      return;
    }
    const next = target();
    if (next === null) {
      from = null;
      draw(null);
      return;
    }
    // Cap each frame's step, so a late frame does not skip ahead.
    const step = last === null ? 0 : Math.min(ts - last, GLIDE_FRAME_CAP_MS);
    last = ts;
    elapsed = Math.min(elapsed + step, GLIDE_MS);
    // Bound each intermediate frame to the target's clip. Start and target can sit in different clips.
    draw(cutRect(lerpRect(from, next, easeGlide(elapsed / GLIDE_MS)), env.clip(range)));
    if (elapsed < GLIDE_MS) {
      frame = env.raf(tick);
    } else {
      from = null;
      last = null;
    }
  }

  // Move the pill to a word. A glide starts from the drawn place, so a new word mid-glide
  // continues from where the pill is, not from where the last glide began.
  function move(next) {
    range = next;
    if (el === null) create();
    stopLoop();
    last = null;
    elapsed = 0;
    const rect = target();
    if (rect === null) {
      from = null;
      draw(null);
      return;
    }
    if (drawn === null || env.reduced()) {
      from = null;
      draw(rect);
      return;
    }
    from = drawn;
    frame = env.raf(tick);
  }

  // Re-place the pill on the word. A running glide already reads the live rect each frame.
  function reposition() {
    if (el === null) return;
    if (range !== null && !range.startContainer.isConnected) {
      destroy();
      return;
    }
    if (frame !== 0) return;
    draw(target());
  }

  function create() {
    el = env.make();
    el.setAttribute("aria-hidden", "true");
    el.style.position = "fixed";
    el.style.zIndex = String(PILL_Z_INDEX);
    el.style.pointerEvents = "none";
    el.style.borderRadius = PILL_RADIUS_PX + "px";
    el.style.background = "#fff";
    el.style.mixBlendMode = "difference";
    env.attach(el);
    offs = [env.listen("scroll", reposition), env.listen("resize", reposition)];
  }

  function destroy() {
    stopLoop();
    for (const off of offs) off();
    offs = [];
    if (el !== null) el.remove();
    el = null;
    range = null;
    drawn = null;
    from = null;
    last = null;
    elapsed = 0;
  }

  return {
    move: move,
    reposition: reposition,
    destroy: destroy,
    moving: function () {
      return frame !== 0;
    },
    element: function () {
      return el;
    }
  };
}

// Paint one word. Plain mode uses the CSS highlight. Glide mode moves the pill,
// and only when the word changes, so repeated status ticks do not restart the glide.
function paintToken(run, token) {
  if (run.mode === "none") return;
  if (run.mode !== "glide") {
    destroyPill();
    if (captionsSupported()) {
      window.CSS.highlights.set(CAPTION_NAME, new window.Highlight(tokenRange(token)));
    }
    return;
  }
  clearCaptionPaint();
  if (token === run.pillToken) return;
  run.pillToken = token;
  ensurePill().move(tokenRange(token));
}

// Change the paint mode between two words. The last word repaints in the new mode.
function switchMode(run, mode) {
  run.mode = mode;
  run.pillToken = null;
  destroyPill();
  clearCaptionPaint();
  if (run.lastToken !== null && run.lastToken.node.isConnected) paintToken(run, run.lastToken);
}

// Paint the word at `position`. Returns true while the fragment is aligned, even on a held gap.
function paintCaption(run, caption, position, fragmentIndex) {
  if (String(caption.text) !== caption.text || !Array.isArray(caption.words)) return false;
  const key = String(fragmentIndex) + "|" + caption.text;
  if (key !== run.key) {
    const tokens = targetTokens(run);
    if (tokens.length === 0) {
      if (!run.warnedEmpty) {
        run.warnedEmpty = true;
        warnNoAlignment(captionWords(caption).length, 0, 0, targetMissReason(run));
      }
      return false;
    }
    const words = captionWords(caption);
    const detail = alignDetail(words, tokens, run.cursor);
    // A miss stores no key, so the next poll aligns again. The reply may still be rendering.
    run.key = detail.ok ? key : null;
    run.words = words;
    run.tokens = tokens;
    run.map = detail.ok ? detail.map : null;
    if (detail.ok) {
      run.cursor = detail.cursor;
    } else if (run.warnedKey !== key) {
      run.warnedKey = key;
      const reason = detail.mapped === 0 ? "resync" : "floor";
      warnNoAlignment(words.length, tokens.length, detail.mapped, reason);
    }
  }
  if (run.map === null) return false;
  const active = activeWordIndex(run.words, position);
  // A gap between words holds the last painted word: no paint call, no clear.
  if (active < 0) return true;
  const token = run.tokens[run.map[active]];
  if (token === undefined || token.node === undefined) return true;
  if (!token.node.isConnected) {
    run.key = null;
    destroyPill();
    run.pillToken = null;
    return false;
  }
  run.lastToken = token;
  followToken(run, token);
  paintToken(run, token);
  return true;
}

// Stop the run only while it is still the live one, so a stale run cannot stop its replacement.
function endRun(run) {
  if (captionRun === run) stopCaptions();
}

// Why a run found no rendered words: one label per target kind.
function targetMissReason(run) {
  const target = run.target;
  if (target === null || target === undefined) {
    return assistantRoot() === undefined ? "no-root" : "empty-root";
  }
  if (target.kind === "selection") return "no-selection";
  return target.root === null ? "no-message" : "empty-message";
}

// Re-check focus and settings for a run. Applies a paint-mode change and the follow flag.
// False when the run must stop.
function syncRun(run) {
  const current = run.settings();
  const plan = pollPlan(run, current, focusedSession());
  if (plan === "stop") return false;
  run.follow = current.follow;
  // Drop the cached scroller with its listeners, so a re-enable looks it up and watches it again.
  if (!run.follow) {
    unwatchScroller(run);
    run.scroller = null;
  }
  if (plan === "switch") switchMode(run, paintMode(run, current));
  return true;
}

function onStatus(run, body) {
  if (captionRun !== run) return;
  // Re-check before painting: focus or settings may have changed while the request was pending.
  if (!syncRun(run)) {
    endRun(run);
    return;
  }
  // Mirror the app's pause state into the playback record, so the bar's glyph
  // follows a pause made in the CopySpeech window too. An optimistic flip
  // keeps the field for a moment, so an in-flight poll cannot flicker it.
  if (body !== null && run.requestId !== null && run.requestId === playbackRequestId) {
    if (Date.now() - playbackPauseChangedAt >= PAUSE_RECONCILE_MS) {
      playbackSetPaused(body.paused === true);
    }
  }
  const now = Date.now();
  const caption = body !== null && body.active === true ? body.captions : null;
  if (caption && paintCaption(run, caption, body.position_ms, body.fragment_index)) {
    run.lastMatch = now;
    run.matched = true;
    run.endedPolls = 0;
    return;
  }
  // Inactive with captions can be a synthesis stall; the end report has no captions.
  const ended = body !== null && body.active !== true && !body.captions;
  // One inactive poll can be a throttled or briefly stale webview report. End
  // only after two in a row, so a single missed report never hides the captions.
  run.endedPolls = ended ? (run.endedPolls || 0) + 1 : 0;
  const silent = now - (run.matched ? run.lastMatch : run.startedAt) >= CAPTION_SILENCE_MS;
  if (silent || (run.matched && run.endedPolls >= 2)) endPlaybackRun(run);
}

// A natural end also clears the playback record when the ending run owns it,
// so the bar and the buttons retire with the reading. A run that lost focus
// only stops painting; its reading keeps playing.
function endPlaybackRun(run) {
  if (run.requestId !== null && playbackRequestId === run.requestId) playbackForget();
  else stopCaptions();
}

// Paint mode of a run: none with captions off, glide with the pill, else the
// CSS highlight. A selection read paints even with the captions switch off.
function paintMode(run, settings) {
  if (!settings.captions && run.forceCaptions !== true) return "none";
  return settings.glide ? "glide" : "plain";
}

// What one status poll does: stop the run, switch paint mode, or keep going.
// A run with follow on keeps polling while captions are off. A controlled run
// (message or selection read) also keeps polling with every paint switch off,
// because the same poll drives the bar's pause state.
export function pollPlan(run, settings, focusedId) {
  if (run.sessionId !== focusedId) return "stop";
  if (
    !settings.captions &&
    !settings.follow &&
    run.forceCaptions !== true &&
    run.controlled !== true
  ) {
    return "stop";
  }
  return paintMode(run, settings) === run.mode ? "keep" : "switch";
}

function pollCaptions(run) {
  if (captionRun !== run) return;
  // Check focus and settings before the busy return, so a pending request cannot hide a change.
  if (!syncRun(run)) {
    endRun(run);
    return;
  }
  if (run.busy) return;
  run.busy = true;
  void fetch(STATUS_URL, { mode: "cors", cache: "no-store" })
    .then((response) => (response.ok ? response.json() : null))
    .catch(() => null)
    .then((body) => {
      run.busy = false;
      onStatus(run, body);
    });
}

// Start one caption run. Captions and follow share the status poll; a new
// reading replaces the run. A target names what the run paints against:
// `latest` (auto-read), `message` (a play-button read), or `selection` (a
// cloned range). A controlled run keeps polling even with paint off, because
// the same poll drives the bar's pause state.
function startRun(ctx, settingsNow, sessionId, target, requestId) {
  stopCaptions();
  const current = settingsNow();
  const kind = target === undefined || target === null ? "latest" : target.kind;
  const controlled = kind !== "latest";
  const force = controlled && kind === "selection";
  // Glide paints with the pill, which needs no highlight API. Plain captions need it.
  const paints = current.captions && (current.glide || captionsSupported());
  // Follow needs no paint, so it runs where captions cannot paint.
  if (!controlled && !current.follow && !paints) return;
  const now = Date.now();
  const run = {
    key: null,
    words: [],
    tokens: [],
    map: null,
    cursor: 0,
    warnedEmpty: false,
    warnedKey: null,
    busy: false,
    settings: settingsNow,
    sessionId: sessionId,
    requestId: requestId === undefined ? null : requestId,
    target: controlled
      ? {
          kind: kind,
          root: target.root ?? null,
          range: target.range ?? null,
          speechNorms: target.speechNorms ?? null
        }
      : null,
    controlled: controlled,
    forceCaptions: force,
    mode: paintMode({ forceCaptions: force }, current),
    follow: current.follow,
    scroller: null,
    watched: null,
    followPausedUntil: 0,
    lastScrollAt: 0,
    lastToken: null,
    pillToken: null,
    startedAt: now,
    lastMatch: now,
    matched: false,
    dispose: null
  };
  run.pause = function () {
    run.followPausedUntil = Date.now() + FOLLOW_PAUSE_MS;
  };
  run.pauseKey = function (event) {
    if (FOLLOW_KEYS.has(event.key)) run.pause();
  };
  run.pauseScrollbar = function (event) {
    if (event.target === run.watched) run.pause();
  };
  const stopPoll = ctx.setInterval(() => pollCaptions(run), CAPTION_POLL_MS);
  run.dispose = function () {
    stopPoll();
    unwatchScroller(run);
  };
  captionRun = run;
}

// The auto-read entry: paint the newest assistant reply of the session.
function startCaptions(ctx, settingsNow, sessionId) {
  startRun(ctx, settingsNow, sessionId, null, null);
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

// ---- Injected UI: icons, the playback bar, message buttons, selection pill ----

// The desktop app provides MutationObserver and SVG; the node test world does
// not, and the caption tests run without the injected UI.
function uiAvailable() {
  return window.MutationObserver !== undefined && window.document !== undefined;
}

// Register fills these two: they need ctx.storage and the begin chain.
let pluginBeginRead = null;
let pluginSettings = null;

const SVG_NS = "http://www.w3.org/2000/svg";

// Icons draw in currentColor, so every surface follows the app theme. The
// speaker is the outlined app-logo mark; the controls are filled, one family.
const ICON_SPEAKER = {
  viewBox: "14.86 14.717 48.611 48.611",
  stroked: true,
  paths: [
    "m37.35 20.74c-0.74-0.04-1.46 0.21-2.02 0.71l-7.68 7.13c-0.24 0.23-0.56 0.37-0.9 0.39l-5.58 0.21c-2.55 0.1-4.51 2.23-4.51 5v7.99c0 2.81 2.1 4.95 4.74 5.09l5.3 0.28c0.28 0.02 0.54 0.15 0.74 0.35l7.94 8.12c0.71 0.71 1.67 1.04 2.55 1.02 1.52-0.04 2.65-1.2 2.65-2.76v-30.18c0-1.84-1.65-3.27-3.23-3.35z",
    "M48.17 21.93A19.44 19.44 0 0 1 48.17 56.13",
    "M45.06 31.13A9.43 9.43 0 0 1 45.06 46.87"
  ]
};
const ICON_PLAY = { viewBox: "0 0 16 16", paths: ["M5.8 3.5 12.2 8 5.8 12.5Z"] };
const ICON_PAUSE = { viewBox: "0 0 16 16", paths: ["M4.6 3h2.5v10H4.6zM8.9 3h2.5v10H8.9z"] };
const ICON_STOP = { viewBox: "0 0 16 16", paths: ["M3.8 3.8h8.4v8.4H3.8z"] };
// The message button's stop glyph: an outlined square in the speaker's own
// viewBox and stroke, so the two glyphs share weight and size.
const ICON_STOP_MESSAGE = {
  viewBox: "14.86 14.717 48.611 48.611",
  stroked: true,
  paths: ["M23 29h20v20H23z"]
};
const ICON_BACK = {
  viewBox: "0 0 24 24",
  paths: ["M11 18V6l-8.5 6 8.5 6zm.5-6l8.5 6V6l-8.5 6z"]
};
const ICON_FORWARD = {
  viewBox: "0 0 24 24",
  paths: ["M4 18l8.5-6L4 6v12zm9-12v12l8.5-6L13 6z"]
};
const ICON_CHAT = {
  viewBox: "0 0 16 16",
  paths: [
    "M3.6 3.1H12.4A1.9 1.9 0 0 1 14.3 5V9A1.9 1.9 0 0 1 12.4 10.9H7.6L4.9 13.3V10.9H3.6A1.9 1.9 0 0 1 1.7 9V5A1.9 1.9 0 0 1 3.6 3.1Z"
  ]
};

const UI_STYLE = [
  /* The message play button sits in the host's hover action row, beside Copy
     and the built-in Read aloud. The row is transparent until the message is
     hovered; the button inherits that, so it appears with its neighbors. */
  ".csh-msg-play{width:28px;height:28px;border:none;border-radius:6px;background:transparent;padding:6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:var(--ui-text-tertiary)}",
  "@media (hover:hover){.csh-msg-play:hover{background:color-mix(in oklab, var(--ui-text-tertiary) 12%, transparent);color:var(--ui-text-secondary)}}",
  ".csh-msg-play:focus-visible{outline:2px solid var(--ui-accent);outline-offset:2px}",
  ".csh-msg-play svg{width:15px;height:15px;display:block}",
  ".csh-tooltip{position:fixed;z-index:2147483002;pointer-events:none;max-width:min(24rem, calc(100vw - 24px));border-radius:4px;background:var(--foreground);color:var(--background);padding:4px 8px;font-family:inherit;font-size:0.6875rem;font-weight:500;line-height:1.4;overflow-wrap:anywhere;white-space:normal}",
  /* The playback bar sits in the composer's top slot, left-aligned above the
     input. The composer ring draws its outline, so the bar has no border. */
  ".csh-bar{display:inline-flex;align-items:center;align-self:flex-start;gap:2px;padding:0 2px}",
  ".csh-bar-btn{width:26px;height:26px;border:none;border-radius:6px;background:transparent;padding:5px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:var(--ui-text-secondary)}",
  "@media (hover:hover){.csh-bar-btn:hover{background:color-mix(in oklab, var(--ui-text-secondary) 12%, transparent)}}",
  ".csh-bar-btn:focus-visible{outline:2px solid var(--ui-accent);outline-offset:1px}",
  ".csh-bar-btn svg{width:15px;height:15px;display:block}",
  ".csh-bar-stop{color:var(--ui-text-primary, var(--ui-text-secondary))}",
  /* The selection pill: a small floating Read aloud above the selection. */
  ".csh-pill{position:fixed;z-index:2147483001}",
  ".csh-pill-btn{border:none;border-radius:999px;padding:5px 12px;background:var(--ui-accent);color:var(--ui-accent-foreground, #fff);font:inherit;font-size:12px;line-height:16px;cursor:pointer;box-shadow:0 4px 14px rgb(0 0 0 / 22%)}",
  "@media (hover:hover){.csh-pill-btn:hover{filter:brightness(1.08)}}",
  ".csh-pill-btn:focus-visible{outline:2px solid var(--ui-accent);outline-offset:2px}"
].join("\n");

function installUiStyle() {
  const old = document.getElementById(UI_STYLE_ID);
  if (old !== null) old.remove();
  const style = document.createElement("style");
  style.id = UI_STYLE_ID;
  style.textContent = UI_STYLE;
  document.head.appendChild(style);
  return style;
}

function svgGlyph(icon, size) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", icon.viewBox);
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  const stroke = icon.stroked === true ? String(3.3 * (17 / size)) : null;
  for (const d of icon.paths) {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    if (stroke !== null) {
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", "currentColor");
      path.setAttribute("stroke-width", stroke);
      path.setAttribute("stroke-linecap", "round");
      path.setAttribute("stroke-linejoin", "round");
    } else {
      path.setAttribute("fill", "currentColor");
    }
    svg.appendChild(path);
  }
  return svg;
}

// ---- The playback bar ----

// The bar renders in the composer's top slot, above the chat input. The composer
// ring draws its outline. The slot name is app/chat/composer/contrib.ts's
// composer.top; the SDK barrel does not export it, so the string lives here.
const COMPOSER_TOP_AREA = "composer.top";

// Pure: what the bar shows for the current reading.
export function barStateFor(requestId, paused) {
  const active = requestId !== null;
  return { active: active, paused: active && paused };
}

// updateBar() writes this atom. PlaybackBar subscribes to it. register() creates it, like the settings atom.
let barState = null;

function barGlyph(icon, size) {
  const stroke = icon.stroked === true ? String(3.3 * (17 / size)) : null;
  return jsx("svg", {
    viewBox: icon.viewBox,
    width: size,
    height: size,
    "aria-hidden": "true",
    focusable: "false",
    children: icon.paths.map((d, i) =>
      jsx("path", {
        key: i,
        d: d,
        ...(stroke !== null
          ? {
              fill: "none",
              stroke: "currentColor",
              strokeWidth: stroke,
              strokeLinecap: "round",
              strokeLinejoin: "round"
            }
          : { fill: "currentColor" })
      })
    )
  });
}

function barButton(label, icon, onClick, extraClass) {
  return jsx("button", {
    type: "button",
    className: extraClass ? "csh-bar-btn " + extraClass : "csh-bar-btn",
    "aria-label": label,
    title: label,
    onClick: onClick,
    children: barGlyph(icon, 15)
  });
}

function PlaybackBar({ barAtom }) {
  const state = useValue(barAtom);
  if (!state.active) return null;
  const label = state.paused ? "Resume reading" : "Pause reading";
  return jsxs("div", {
    className: "csh-bar",
    role: "region",
    "aria-label": "CopySpeech playback",
    children: [
      barButton(label, state.paused ? ICON_PLAY : ICON_PAUSE, () => {
        void (playbackPaused ? resumeReading() : pauseReading());
      }),
      barButton("Back 5 seconds", ICON_BACK, () => {
        void seekReading(-SEEK_STEP_MS);
      }),
      barButton("Forward 5 seconds", ICON_FORWARD, () => {
        void seekReading(SEEK_STEP_MS);
      }),
      barButton(
        "Stop reading",
        ICON_STOP,
        () => {
          void stopReadingNow();
        },
        "csh-bar-stop"
      ),
      barButton("Show the spoken word", ICON_CHAT, () => {
        showPlaybackInChat();
      })
    ]
  });
}

function updateBar() {
  if (barState === null) return;
  barState.set(barStateFor(playbackRequestId, playbackPaused));
}

// The bar's Show-in-chat control: bring the spoken word's line to the anchor.
// With no aligned word yet, it brings the reading's target into view instead.
function showPlaybackInChat() {
  const run = captionRun;
  let element = null;
  let range = null;
  if (run !== null && run.lastToken !== null && run.lastToken.node.isConnected) {
    element = run.lastToken.node.parentElement;
    range = tokenRange(run.lastToken);
  } else if (playbackRoot !== null && playbackRoot.isConnected) {
    element = playbackRoot;
  } else if (playbackRange !== null && playbackRange.startContainer.isConnected) {
    element = playbackRange.startContainer.parentElement;
    range = playbackRange;
  }
  if (element === null) return;
  const scroller = findScroller(element);
  if (scroller === null) {
    element.scrollIntoView({ block: "center", behavior: motionBehavior() });
    return;
  }
  const rect =
    range !== null
      ? (firstPaintedRect(range) ?? range.getBoundingClientRect())
      : element.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return;
  const box = scroller.getBoundingClientRect();
  const offset = rect.top - box.top - box.height * FOLLOW_ANCHOR;
  scroller.scrollTo({ top: scroller.scrollTop + offset, behavior: motionBehavior() });
}

// ---- Message play buttons ----

const messageButtonSyncs = new Set();
let messageTooltip = null;
let messageTooltipTimer = null;
let messageTooltipPending = null;

function hideMessageTooltip() {
  if (messageTooltipTimer !== null) {
    window.clearTimeout(messageTooltipTimer);
    messageTooltipTimer = null;
  }
  messageTooltipPending = null;
  if (messageTooltip !== null) {
    messageTooltip.el.remove();
    messageTooltip = null;
  }
}

function showMessageTooltip(button) {
  hideMessageTooltip();
  messageTooltipPending = button;
  messageTooltipTimer = window.setTimeout(() => {
    messageTooltipTimer = null;
    messageTooltipPending = null;
    if (!button.isConnected) return;
    const el = document.createElement("div");
    el.className = "csh-tooltip";
    el.setAttribute("role", "tooltip");
    el.textContent = button.getAttribute("aria-label");
    document.documentElement.appendChild(el);
    const target = button.getBoundingClientRect();
    const tip = el.getBoundingClientRect();
    const top = target.top - tip.height - 5;
    el.style.top = (top >= 8 ? top : target.bottom + 5) + "px";
    el.style.left =
      Math.max(
        8,
        Math.min(target.left + (target.width - tip.width) / 2, window.innerWidth - tip.width - 8)
      ) + "px";
    messageTooltip = { el: el, button: button };
  }, 200);
}

let buttonsObserver = null;
let scanQueued = false;

// The reply's rendered text for speech: the Markdown bodies' words, minus
// reasoning and code blocks, joined as paragraphs.
function spokenTextFromRoot(root) {
  const parts = [];
  for (const body of root.querySelectorAll(MESSAGE_BODY_SELECTOR)) {
    if (body.closest(REASONING_SELECTOR) !== null) continue;
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      if (node.parentElement !== null && node.parentElement.closest("pre") !== null) continue;
      parts.push(node.nodeValue ?? "");
    }
    parts.push("\n\n");
  }
  return toSpeech(parts.join(""));
}

function makeMessageButton(root) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "csh-msg-play";
  button.setAttribute(PLAY_BUTTON_FLAG, "");
  const sync = () => {
    const active = playbackSource === "message" && playbackRoot === root;
    const label = active ? "Stop CopySpeech reading" : "Read aloud with CopySpeech";
    button.setAttribute("aria-label", label);
    button.setAttribute("aria-pressed", active ? "true" : "false");
    if (messageTooltip !== null && messageTooltip.button === button) {
      messageTooltip.el.textContent = label;
    }
    button.dataset.active = active ? "true" : "false";
    button.replaceChildren(svgGlyph(active ? ICON_STOP_MESSAGE : ICON_SPEAKER, 15));
  };
  button.addEventListener("pointerenter", () => showMessageTooltip(button));
  button.addEventListener("pointerleave", () => {
    if (!button.matches(":focus-visible")) hideMessageTooltip();
  });
  button.addEventListener("focus", () => {
    if (button.matches(":focus-visible")) showMessageTooltip(button);
  });
  button.addEventListener("blur", hideMessageTooltip);
  button.addEventListener("keydown", (event) => {
    if (event.key === "Escape") hideMessageTooltip();
  });
  button.addEventListener("click", () => {
    if (playbackSource === "message" && playbackRoot === root) {
      void stopReadingNow();
      return;
    }
    if (pluginBeginRead === null) return;
    const speech = spokenTextFromRoot(root);
    if (speech === "") {
      host.notify({ kind: "warning", message: "Nothing to read aloud in this reply" });
      return;
    }
    pluginBeginRead({ source: "message", speech: speech, root: root });
  });
  messageButtonSyncs.add({ button: button, sync: sync });
  sync();
  return button;
}

// One injected button per hover action row. The row belongs to a settled
// reply, so buttons appear when replies finish and ride virtualization.
function scanMessageRows() {
  if (messageTooltip !== null && !messageTooltip.button.isConnected) hideMessageTooltip();
  if (messageTooltipPending !== null && !messageTooltipPending.isConnected) hideMessageTooltip();
  for (const entry of messageButtonSyncs) {
    if (!entry.button.isConnected) messageButtonSyncs.delete(entry);
  }
  for (const row of document.querySelectorAll(MESSAGE_ACTIONS_SELECTOR)) {
    if (row.querySelector("button[" + PLAY_BUTTON_FLAG + "]") !== null) continue;
    const root = row.closest(MESSAGE_ROOT_SELECTOR);
    if (root === null) continue;
    row.appendChild(makeMessageButton(root));
  }
}

function scheduleScan() {
  if (scanQueued || buttonsObserver === null) return;
  scanQueued = true;
  window.requestAnimationFrame(() => {
    scanQueued = false;
    scanMessageRows();
  });
}

function installMessageButtons() {
  if (buttonsObserver !== null || window.MutationObserver === undefined) return;
  buttonsObserver = new window.MutationObserver(scheduleScan);
  buttonsObserver.observe(document.documentElement, { childList: true, subtree: true });
  scanMessageRows();
}

function removeMessageButtons() {
  hideMessageTooltip();
  if (buttonsObserver !== null) {
    buttonsObserver.disconnect();
    buttonsObserver = null;
  }
  for (const button of document.querySelectorAll("button[" + PLAY_BUTTON_FLAG + "]")) {
    button.remove();
  }
  messageButtonSyncs.clear();
}

function syncMessageButtons() {
  for (const entry of messageButtonSyncs) {
    if (entry.button.isConnected) entry.sync();
    else messageButtonSyncs.delete(entry);
  }
}

// ---- The selection read pill ----

let selectionPill = null;
let selectionTimer = null;

function dismissSelectionPill() {
  if (selectionPill === null) return;
  selectionPill.el.remove();
  selectionPill = null;
}

function positionSelectionPill(range) {
  if (selectionPill === null) return;
  const rects = Array.from(range.getClientRects()).filter((r) => r.width > 0 && r.height > 0);
  if (rects.length === 0) {
    dismissSelectionPill();
    return;
  }
  const last = rects[rects.length - 1];
  const above = last.top > 52;
  const top = above ? last.top - 44 : last.bottom + 8;
  const left = Math.max(8, Math.min(last.right - 60, window.innerWidth - 150));
  selectionPill.el.style.top = Math.max(8, top) + "px";
  selectionPill.el.style.left = left + "px";
}

function showSelectionPill(range) {
  if (selectionPill === null) {
    const el = document.createElement("div");
    el.className = "csh-pill";
    const button = document.createElement("button");
    button.type = "button";
    button.className = "csh-pill-btn";
    button.textContent = "Read aloud";
    // A pointer-down on the pill must not collapse the selection before the
    // click can clone its range.
    el.addEventListener("pointerdown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      readSelectedRange();
    });
    el.appendChild(button);
    document.documentElement.appendChild(el);
    selectionPill = { el: el, button: button, range: range };
  } else {
    selectionPill.range = range;
  }
  positionSelectionPill(range);
}

function selectionInsideReply(range) {
  const node = range.startContainer;
  const anchor = node.nodeType === 1 ? node : node.parentElement;
  if (anchor === null) return false;
  return (
    anchor.closest(MESSAGE_BODY_SELECTOR) !== null && anchor.closest(MESSAGE_ROOT_SELECTOR) !== null
  );
}

function updateSelectionPill() {
  const settings = pluginSettings === null ? DEFAULTS : pluginSettings();
  if (!settings.selectionRead) {
    dismissSelectionPill();
    return;
  }
  const selection = window.getSelection();
  if (selection === null || selection.isCollapsed || selection.rangeCount !== 1) {
    dismissSelectionPill();
    return;
  }
  const range = selection.getRangeAt(0);
  if (!selectionInsideReply(range)) {
    dismissSelectionPill();
    return;
  }
  showSelectionPill(range);
}

// selectionchange fires per drag movement; one trailing run per 120 ms.
function onSelectionChange() {
  if (selectionTimer !== null) return;
  selectionTimer = window.setTimeout(() => {
    selectionTimer = null;
    updateSelectionPill();
  }, 120);
}

function readSelectedRange() {
  if (pluginBeginRead === null || selectionPill === null) return;
  const range = selectionPill.range;
  dismissSelectionPill();
  if (range === null || !range.startContainer.isConnected) return;
  const speech = toSpeech(range.toString());
  const selection = window.getSelection();
  if (selection !== null) selection.removeAllRanges();
  if (speech === "") {
    host.notify({ kind: "warning", message: "Nothing to read aloud in this selection" });
    return;
  }
  pluginBeginRead({ source: "selection", speech: speech, range: range });
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

// Draft edits whose value differs from the saved value. Only these are written on Save.
export function pendingEdits(saved, draft) {
  const pending = {};
  for (const key of Object.keys(draft)) {
    if (draft[key] !== saved[key]) pending[key] = draft[key];
  }
  return pending;
}

// Form values: unsaved edits win, every other field follows the saved settings.
export function draftView(saved, draft) {
  return { ...saved, ...draft };
}

// One edit on top of the draft. A value equal to the saved value is not an edit, so it is dropped.
export function applyEdit(saved, draft, patch) {
  return pendingEdits(saved, { ...draft, ...patch });
}

// Edits stay in local state until Save. Cancel clears them.
function SettingsPage({ storage, settings }) {
  const saved = useValue(settings);
  const [changes, setChanges] = useState({});
  useEffect(() => {
    // A saved change (palette toggle, storage event) can make an edit equal. Drop it so it cannot return.
    setChanges((draft) => {
      const kept = pendingEdits(saved, draft);
      return Object.keys(kept).length === Object.keys(draft).length ? draft : kept;
    });
  }, [saved]);
  const s = draftView(saved, changes);
  const dirty = Object.keys(pendingEdits(saved, changes)).length > 0;
  const health = useQuery({
    queryKey: [ID, "health"],
    queryFn: probeCopySpeech,
    refetchInterval: 10000
  });
  const update = (patch) => setChanges((draft) => applyEdit(settings.get(), draft, patch));
  const save = () => {
    // Compare with stored settings, not this window's copy: another window may have saved since.
    saveSettings(storage, settings, pendingEdits(readSettings(storage), changes));
    setChanges({});
  };
  const cancel = () => setChanges({});
  const builtIn = builtInAutoSpeakOn();
  const status =
    health.data === undefined
      ? "Checking CopySpeech…"
      : health.data
        ? "CopySpeech is running"
        : "CopySpeech not reachable on 127.0.0.1:43117";
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
      s.captions
        ? jsx(ToggleRow, {
            label: "Glide between words",
            checked: s.glide,
            onChange: (on) => update({ glide: on })
          })
        : null,
      jsx(ToggleRow, {
        label: "Follow playback",
        checked: s.follow,
        onChange: (on) => update({ follow: on })
      }),
      jsx(ToggleRow, {
        label: "Show play buttons on replies",
        checked: s.messageButtons,
        onChange: (on) => update({ messageButtons: on })
      }),
      jsx(ToggleRow, {
        label: "Read aloud on selection",
        checked: s.selectionRead,
        onChange: (on) => update({ selectionRead: on })
      }),
      jsx(ToggleRow, {
        label: "Skip bullet points and sections",
        checked: s.skipMarkdownSections,
        onChange: (on) => update({ skipMarkdownSections: on })
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
        : null,
      dirty
        ? jsxs("div", {
            className: "flex items-center justify-end gap-2",
            children: [
              jsx("span", {
                className: "me-auto text-(--ui-text-tertiary)",
                children: "Unsaved changes"
              }),
              jsx(Button, { variant: "outline", size: "sm", onClick: cancel, children: "Cancel" }),
              jsx(Button, { size: "sm", onClick: save, children: "Save" })
            ]
          })
        : null
    ]
  });
}

export default {
  id: ID,
  name: "CopySpeech",
  description: "Reads new completed Hermes replies aloud through the running CopySpeech app.",
  register(ctx) {
    const storage = ctx.storage;
    const settings = atom(readSettings(storage));
    const armed = new Set();
    const style = installCaptionStyle();
    const ui = uiAvailable();
    pluginSettings = () => readSettings(storage);
    let uiStyle = null;
    if (ui) uiStyle = installUiStyle();

    // Start one controlled reading: a message play-button read or a selection
    // read. Calls queue through the chain, so overlapping starts cannot post
    // two /speak calls around one stop; a stop bumps the epoch and cancels a
    // start still queued behind it.
    let beginChain = Promise.resolve();
    const beginReading = (plan) => {
      const epoch = playbackStopEpoch;
      const run = () => {
        if (epoch !== playbackStopEpoch) return Promise.resolve(null);
        const previous = playbackRequestId;
        const stopping = previous === null ? Promise.resolve(true) : stopReading(previous);
        return stopping.then((stopped) => {
          if (!stopped) {
            notifyOffline();
            return "error.offline";
          }
          const requestId = nextRequestId();
          const selection = plan.source === "selection";
          const speech = speechForRead(plan.source, plan.speech, readSettings(storage));
          if (speech === "") {
            host.notify({
              kind: "warning",
              message: "Nothing to read aloud after skipping bullets and sections"
            });
            return null;
          }
          playbackRemember(
            plan.source,
            requestId,
            selection ? null : (plan.root ?? null),
            selection ? (plan.range ?? null) : null
          );
          const target = selection
            ? {
                kind: "selection",
                range: plan.range ?? null,
                speechNorms: wordNorms(speech)
              }
            : { kind: "message", root: plan.root ?? null };
          startRun(ctx, () => readSettings(storage), focusedSession(), target, requestId);
          const controller = new AbortController();
          playbackSpeakAbort = controller;
          void controlPost(
            "/speak",
            { text: speech, request_id: requestId, wait: true },
            controller.signal
          ).then((sent) => {
            if (playbackSpeakAbort === controller) playbackSpeakAbort = null;
            if (sent) notifiedDown = false;
            if (playbackRequestId !== requestId) return null;
            playbackForget();
            // An aborted request is the stop the user asked for, not a failure.
            if (controller.signal.aborted) return null;
            if (!sent) {
              notifyOffline();
              return "error.offline";
            }
            return null;
          });
        });
      };
      const next = beginChain.then(run, run);
      beginChain = next;
      return next;
    };
    pluginBeginRead = beginReading;

    ctx.onDispose(() => {
      pluginBeginRead = null;
      pluginSettings = null;
      // Unloading the plugin stops its reading, not just the paint.
      if (playbackSpeakAbort !== null) {
        playbackSpeakAbort.abort();
        playbackSpeakAbort = null;
      }
      const requestId = playbackRequestId;
      if (requestId !== null) void stopReading(requestId);
      playbackForget();
      if (ui) {
        removeMessageButtons();
        dismissSelectionPill();
        if (uiStyle !== null) {
          uiStyle.remove();
          uiStyle = null;
        }
      }
      stopCaptions();
      style.remove();
    });
    ctx.addEventListener(window, "storage", (event) => {
      if (event.key === null || event.key === SETTINGS_KEY) {
        settings.set(readSettings(storage));
        syncUi();
      }
    });

    // The injected UI follows the saved settings: buttons and the pill can be
    // turned off. The bar is a composer contribution and shows only while a reading plays.
    function syncUi() {
      if (!ui) return;
      const current = readSettings(storage);
      if (current.messageButtons) installMessageButtons();
      else removeMessageButtons();
      if (!current.selectionRead) dismissSelectionPill();
    }
    syncUi();
    if (ui) {
      ctx.addEventListener(document, "selectionchange", () => {
        onSelectionChange();
      });
      ctx.addEventListener(
        window,
        "scroll",
        () => {
          dismissSelectionPill();
          hideMessageTooltip();
        },
        { capture: true, passive: true }
      );
      ctx.addEventListener(
        window,
        "keydown",
        (event) => {
          if (event.key === "Escape") dismissSelectionPill();
        },
        { capture: true }
      );
    }

    ctx.onEvent("message.start", (event) => {
      if (event && event.session_id) armed.add(event.session_id);
    });

    ctx.onEvent("message.complete", (event) => {
      const current = readSettings(storage);
      settings.set(current);
      const verdict = speakVerdict(event, {
        settings: current,
        focusedSessionId: focusedSession(),
        armed,
        claims: readClaims(storage),
        builtInOn: builtInAutoSpeakOn()
      });
      if (!verdict.speak) {
        if (verdict.reason === BUILT_IN_REASON) {
          console.info("[copyspeech-hermes] skipped reply: Hermes built-in auto-read is on");
        }
        return;
      }
      const speech = prepareSpeech(String(event.payload.text || ""), current);
      if (!speech) return;
      storage.set("claims", withClaim(readClaims(storage), claimKey(event), Date.now()));
      // Speech starts first. Captions only follow it, so they never delay speech.
      const requestId = nextRequestId();
      playbackRemember("auto", requestId, null, null);
      void postSpeech(speech, requestId);
      if (current.captions || current.follow) {
        startRun(ctx, () => readSettings(storage), event.session_id, null, requestId);
      }
    });

    barState = atom(barStateFor(playbackRequestId, playbackPaused));
    ctx.register({
      id: "playback-bar",
      area: COMPOSER_TOP_AREA,
      render: () => jsx(PlaybackBar, { barAtom: barState })
    });

    ctx.register({
      id: "toggle-auto-read",
      area: PALETTE_AREA,
      data: {
        id: ID + ".toggle-auto-read",
        label: "CopySpeech: Toggle auto-read",
        keywords: ["copyspeech", "copyspeech", "speak", "read", "aloud", "tts"],
        run: () => {
          const next = !readSettings(storage).autoRead;
          saveSettings(storage, settings, { autoRead: next });
          host.notify({
            kind: "info",
            message: next ? "CopySpeech auto-read is on" : "CopySpeech auto-read is off"
          });
        }
      }
    });

    ctx.register({
      id: "stop-reading",
      area: PALETTE_AREA,
      data: {
        id: ID + ".stop-reading",
        label: "CopySpeech: Stop reading",
        keywords: ["copyspeech", "stop", "tts", "speak"],
        run: () => {
          void stopReadingNow();
        }
      }
    });

    ctx.registerSettingsPage({
      id: ID,
      title: "CopySpeech",
      render: () => jsx(SettingsPage, { storage, settings })
    });

    console.info("[copyspeech-hermes] registered");
  }
};
