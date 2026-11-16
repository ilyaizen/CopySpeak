// CopySpeech auto-read for Hermes Desktop.
// Reads each new completed assistant reply through the running CopySpeech app.
// Optional: highlights the word CopySpeech speaks inside the reply (CSS Custom Highlight API,
// or a gliding pill when "Glide between words" is on).
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
  captions: true,
  glide: false,
  follow: false
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

let requestCounter = 0;
let notifiedDown = false;
let captionRun = null;
let pill = null;

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
    follow: boolOr(source.follow, DEFAULTS.follow)
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

function prepareSpeech(text, settings) {
  const speech = toSpeech(text);
  if (!settings.excerpt) return speech;
  return excerptSpeech(speech, settings.excerptMode, settings.excerptTop, settings.excerptBottom);
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
      console.warn("[copyspeech-hermes] speak failed", error);
      host.notify({ kind: "error", message: "CopySpeech not reachable on 127.0.0.1:43117" });
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

// Text nodes of the newest assistant reply, minus reasoning. Words keep their node and range.
function replyTokens() {
  const root = assistantRoot();
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
    const tokens = replyTokens();
    if (tokens.length === 0) {
      if (!run.warnedEmpty) {
        run.warnedEmpty = true;
        const reason = assistantRoot() === undefined ? "no-root" : "empty-root";
        warnNoAlignment(captionWords(caption).length, 0, 0, reason);
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
  if (plan === "switch") switchMode(run, paintMode(current));
  return true;
}

function onStatus(run, body) {
  if (captionRun !== run) return;
  // Re-check before painting: focus or settings may have changed while the request was pending.
  if (!syncRun(run)) {
    endRun(run);
    return;
  }
  const now = Date.now();
  const caption = body !== null && body.active === true ? body.captions : null;
  if (caption && paintCaption(run, caption, body.position_ms, body.fragment_index)) {
    run.lastMatch = now;
    run.matched = true;
    return;
  }
  // Inactive with captions can be a synthesis stall; the end report has no captions.
  const ended = body !== null && body.active !== true && !body.captions;
  const silent = now - (run.matched ? run.lastMatch : run.startedAt) >= CAPTION_SILENCE_MS;
  if (silent || (run.matched && ended)) stopCaptions();
}

// Paint mode of a run: none with captions off, glide with the pill, else the CSS highlight.
function paintMode(settings) {
  if (!settings.captions) return "none";
  return settings.glide ? "glide" : "plain";
}

// What one status poll does: stop the run, switch paint mode, or keep going.
// A run with follow on keeps polling while captions are off.
export function pollPlan(run, settings, focusedId) {
  if (run.sessionId !== focusedId) return "stop";
  if (!settings.captions && !settings.follow) return "stop";
  return paintMode(settings) === run.mode ? "keep" : "switch";
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

// Start one status poll for the reply just sent. Captions and follow share it; a new reply replaces it.
function startCaptions(ctx, settingsNow, sessionId) {
  stopCaptions();
  const current = settingsNow();
  // Glide paints with the pill, which needs no highlight API. Plain captions need it.
  const paints = current.captions && (current.glide || captionsSupported());
  // Follow needs no paint, so it runs where captions cannot paint.
  if (!current.follow && !paints) return;
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
    mode: paintMode(current),
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
    ctx.onDispose(() => {
      stopCaptions();
      style.remove();
    });
    ctx.addEventListener(window, "storage", (event) => {
      if (event.key === null || event.key === SETTINGS_KEY) settings.set(readSettings(storage));
    });

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
      void postSpeech(speech);
      if (current.captions || current.follow) {
        startCaptions(ctx, () => readSettings(storage), event.session_id);
      }
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

    ctx.registerSettingsPage({
      id: ID,
      title: "CopySpeech",
      render: () => jsx(SettingsPage, { storage, settings })
    });

    console.info("[copyspeech-hermes] registered");
  }
};
