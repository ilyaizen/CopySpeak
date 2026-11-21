import { atom, read, update } from "claude-code";
import type { EngineInterface, Register } from "claude-code";

import type { Reading } from "../types";
import { ICON_SVG } from "./icon";

// Port of copyspeech-hermes to Claude Code. CopySpeech's control server does the
// speaking; this mod draws the play button, the captions pane, and the Read aloud
// menu row. The engine draws each reply's own Markdown untouched.
const CONTROL = "http://127.0.0.1:43117";
const POLL_MS = 120;
const IDLE_STOP_MS = 2000;
const NO_MATCH_STOP_MS = 60000;
const ALIGN_WINDOW = 24;
const RUN_NEEDED = 3;
const EXCERPT_MIN = 1;
const EXCERPT_MAX = 20;
// Room at the start of the desktop button's label for the CopySpeech mark drawn
// over it: an em space and an en space, which a label never collapses.
const MARK_ROOM = "  ";
// The spoken word's chip: reverse video, the text color behind the word and the
// inverse text over it. A background moves no glyph, so words after the chip never move.
const CHIP = { backgroundColor: "text", color: "inverseText" } as const;
// The captions pane: the reading's replies as plain text, with the chip on the spoken word.
const PANE = "captions";
const EXCERPT_MODES = ["both", "top", "bottom"];

// Rows and the band read `reading`, which changes only at start, pause, resume and
// stop. Only the captions pane reads `spoken`, so a word redraws the pane alone.
const reading = atom({ plugin: "copyspeech", key: "reading" } as const, null);
const spoken = atom({ plugin: "copyspeech", key: "spoken" } as const, null);
// The rows the captions pane shows: the latest reading's replies. A new reading replaces them.
// The key keeps its old name: a pane hook from before a reload still reads it until it is replaced.
type ShownRow = { row: string; md: string };
const history = atom({ plugin: "copyspeech", key: "history" } as const, [] as ShownRow[]);
// Whether the captions pane's settings are expanded.
const settingsOpen = atom({ plugin: "copyspeech", key: "settingsOpen" } as const, false);

type Token = { norm: string; start: number; end: number };
type RowToken = Token & { row: string };
type CaptionWord = { norm: string; start_ms: number; end_ms: number };
type Caption = {
  text?: string;
  words?: { text_start: number; text_end: number; start_ms: number; end_ms: number }[];
};
type Status = {
  active?: boolean;
  paused?: boolean;
  captions?: Caption;
  position_ms?: number;
  fragment_index?: number;
};

// Markdown that is not spoken: fenced code blocks (``` or ~~~), link targets, HTML
// tags. Inline `code` is spoken; speakable() drops its backticks and reads a dot in it as "dot".
const UNSPOKEN = /(```|~~~)[\s\S]*?(?:\1|$)|\]\([^)\n]*\)|<[^>\n]+>/g;
// A line that opens or closes a fenced code block.
const FENCE_LINE = /^\s*(?:```|~~~)/;
// Lines a skip-sections read drops: headings, bullet and numbered items, horizontal rules, table rows.
const SKIPPED = /^\s{0,3}(?:#{1,6}\s|[-*+]\s|\d+[.)]\s|\|| ?([-*_]) ?(?:\1 ?){2,}$)/;

export function wordTokens(text: string): Token[] {
  return [...text.matchAll(/[\p{L}\p{N}]+/gu)].map((m) => ({
    norm: m[0].toLowerCase(),
    start: m.index ?? 0,
    end: (m.index ?? 0) + m[0].length
  }));
}

// Inline `code` spans, and a dot between two names inside one: `export.ts`, `os.path`.
const CODE_SPAN = /`[^`\n]+`/g;
const CODE_DOT = /[A-Za-z_]\w*\.(?=\w)/g;
// One character standing in for a dot that is said as "dot". It keeps every markdown offset.
const DOT_MARK = "\u0001";

// Spoken tokens of a reply's markdown (offsets into the markdown) and the text sent to CopySpeech.
// A TTS engine reads "." as a pause, so a dot in inline code is said as the word "dot".
// ponytail: pairs backticks left to right; one stray backtick can mark prose as code.
export function speakable(md: string) {
  const skip = [...md.matchAll(UNSPOKEN)].map((m) => ({
    s: m.index ?? 0,
    e: (m.index ?? 0) + m[0].length
  }));
  const skipped = (at: number) => skip.some((r) => at >= r.s && at < r.e);
  const dots = [...md.matchAll(CODE_SPAN)]
    .flatMap((span) =>
      [...span[0].matchAll(CODE_DOT)].map(
        (m) => (span.index ?? 0) + (m.index ?? 0) + m[0].length - 1
      )
    )
    .filter((at) => !skipped(at));
  const tokens = [
    ...wordTokens(md).filter((t) => !skipped(t.start)),
    ...dots.map((at) => ({ norm: "dot", start: at, end: at + 1 }))
  ].sort((a, b) => a.start - b.start);
  let marked = md;
  for (const at of dots) marked = marked.slice(0, at) + DOT_MARK + marked.slice(at + 1);
  const speech = marked
    .replace(UNSPOKEN, " ")
    .replace(/`+/g, "")
    .replace(/^[ \t]*(?:[-+*]|\d+[.)])[ \t]+/gm, "")
    .replace(/[#*_>|~[]+/g, " ")
    .replace(/[ \t]+/g, " ")
    .trim()
    .split(DOT_MARK)
    .join(" dot ");
  return { tokens, speech };
}

export type Excerpt = { mode: string; top: number; bottom: number };

function clampCount(n: number): number {
  return Number.isFinite(n)
    ? Math.min(EXCERPT_MAX, Math.max(EXCERPT_MIN, Math.round(n)))
    : EXCERPT_MIN;
}

// Same rules as copyspeech-hermes excerptSpeech: keep the first and last paragraphs.
export function pickParagraphs<T>(paras: T[], { mode, top, bottom }: Excerpt): T[] {
  if (paras.length < 2) return paras;
  const t = clampCount(top);
  const b = clampCount(bottom);
  if (mode === "top") return paras.slice(0, t);
  if (mode === "bottom") return paras.slice(-b);
  if (t + b >= paras.length) return paras;
  return paras.slice(0, t).concat(paras.slice(-b));
}

// Same length as md: each skipped line becomes spaces, so offsets into md still hold.
function maskSkipped(md: string): string {
  return md
    .split("\n")
    .map((line) => (SKIPPED.test(line) ? " ".repeat(line.length) : line))
    .join("\n");
}

export type Block = { start: number; end: number };

// Paragraphs as [start, end) offsets into md. Blank lines split them, and a skipped
// line is neither text nor a split, so a list between two sentences joins them.
function paragraphs(md: string, skip: boolean): Block[] {
  const out: Block[] = [];
  let from = -1;
  let to = -1;
  let inFence = false;
  let offset = 0;
  for (const line of md.split("\n")) {
    const start = offset;
    offset += line.length + 1;
    if (FENCE_LINE.test(line)) inFence = !inFence;
    if (skip && SKIPPED.test(line)) continue;
    if (line.trim() === "" && !inFence) {
      if (from >= 0) out.push({ start: from, end: to });
      from = -1;
    } else {
      if (from < 0) from = start;
      to = start + line.length;
    }
  }
  if (from >= 0) out.push({ start: from, end: to });
  return out;
}

export type Part = { row: string; md: string; from?: number; to?: number };
export type Spoken = { speech: string; tokens: RowToken[] };

// What a play-button or automatic read speaks, and the tokens the captions align to.
// Tokens hold offsets into their row's markdown, and they are exactly the words of
// `speech`: a skipped line or an excerpt gap adds no token.
export function prepareSpeech(parts: Part[], skip: boolean, excerpt: Excerpt | null): Spoken {
  const paras: { speech: string; tokens: RowToken[] }[] = [];
  for (const { row, md, from = 0, to = Infinity } of parts) {
    const masked = skip ? maskSkipped(md) : md;
    for (const p of paragraphs(md, skip)) {
      if (p.start < from || p.start >= to) continue;
      const piece = speakable(masked.slice(p.start, p.end));
      if (piece.speech === "") continue;
      const tokens = piece.tokens.map((t) => ({
        norm: t.norm,
        start: t.start + p.start,
        end: t.end + p.start,
        row
      }));
      paras.push({ speech: piece.speech, tokens });
    }
  }
  const kept = excerpt === null ? paras : pickParagraphs(paras, excerpt);
  return {
    speech: kept.map((p) => p.speech).join("\n\n"),
    tokens: kept.flatMap((p) => p.tokens)
  };
}

// Caption entries grouped per spoken word. ElevenLabs sends one entry per character.
export function captionWords(caption: Caption): CaptionWord[] {
  const entries = caption.words ?? [];
  const words: CaptionWord[] = [];
  for (const token of wordTokens(caption.text ?? "")) {
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

function findNear(tokens: Token[], norm: string, from: number): number {
  const highest = Math.min(tokens.length, from + ALIGN_WINDOW);
  for (let scan = from; scan < highest; scan += 1) if (tokens[scan]?.norm === norm) return scan;
  return -1;
}

function runAt(
  words: CaptionWord[],
  tokens: Token[],
  i: number,
  at: number,
  need: number
): boolean {
  let cursor = at + 1;
  for (let k = i + 1; k < i + need; k += 1) {
    const found = findNear(tokens, words[k]?.norm ?? "", cursor);
    if (found < 0) return false;
    cursor = found + 1;
  }
  return true;
}

function resync(words: CaptionWord[], tokens: Token[], i: number, from: number): number {
  const need = Math.min(RUN_NEEDED, words.length - i);
  if (need < 2) return -1;
  for (let scan = from; scan < tokens.length; scan += 1) {
    if (tokens[scan]?.norm === words[i]?.norm && runAt(words, tokens, i, scan, need)) return scan;
  }
  return -1;
}

// Same rules as copyspeech-hermes alignDetail: half the caption words must map.
export function align(words: CaptionWord[], tokens: Token[], from: number) {
  const map = words.map(() => -1);
  let cursor = from;
  let mapped = 0;
  for (let i = 0; i < words.length; i += 1) {
    let at = mapped === 0 ? -1 : findNear(tokens, words[i]?.norm ?? "", cursor);
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
  return { ok: mapped > 0 && mapped * 2 >= words.length, map, cursor };
}

export type Span = { text: string; bold?: true; italic?: true; mono?: true; hit?: true };

// Blank-line separated blocks, never splitting a fenced code block.
export function blocks(md: string): Block[] {
  const out: Block[] = [];
  let from = -1;
  let inFence = false;
  let offset = 0;
  for (const line of md.split("\n")) {
    const end = offset + line.length;
    if (FENCE_LINE.test(line)) inFence = !inFence;
    if (line.trim() === "" && !inFence) {
      if (from >= 0) out.push({ start: from, end: offset - 1 });
      from = -1;
    } else if (from < 0) from = offset;
    offset = end + 1;
  }
  if (from >= 0) out.push({ start: from, end: md.length });
  return out;
}

// One block's markdown as styled spans; `mark` (offsets into the whole reply) gets `hit`.
// ponytail: covers headings, list markers, **bold**, *italic*, `code`, [links](url); tables and fences stay raw text.
export function inlineSpans(md: string, block: Block, mark: [number, number] | null): Span[] {
  const chars: Span[] = [];
  const push = (text: string, flags: Omit<Span, "text">) => chars.push({ text, ...flags });
  let bold = false;
  let italic = false;
  let mono = false;
  let lineStart = true;
  let i = block.start;
  while (i < block.end) {
    const rest = md.slice(i, block.end);
    if (lineStart) {
      lineStart = false;
      const heading = /^#{1,6}\s+/.exec(rest);
      const bullet = /^(\s*)[-*+]\s+/.exec(rest);
      const quote = /^>\s?/.exec(rest);
      if (heading) {
        i += heading[0].length;
        bold = true;
        continue;
      }
      if (bullet) {
        push(`${bullet[1] ?? ""}• `, {});
        i += bullet[0].length;
        continue;
      }
      if (quote) {
        i += quote[0].length;
        continue;
      }
    }
    if (!mono && rest.startsWith("**")) {
      bold = !bold;
      i += 2;
      continue;
    }
    if (rest.startsWith("`")) {
      mono = !mono;
      i += 1;
      continue;
    }
    if (!mono && rest.startsWith("*")) {
      italic = !italic;
      i += 1;
      continue;
    }
    const link = mono ? null : /^\]\([^)\n]*\)/.exec(rest);
    if (link) {
      i += link[0].length;
      continue;
    }
    const ch = md[i] ?? "";
    if (!mono && ch === "[") {
      i += 1;
      continue;
    }
    if (ch === "\n") {
      push("\n", {});
      // A heading's bold ends with its line.
      bold = false;
      lineStart = true;
      i += 1;
      continue;
    }
    const span: Span = { text: ch };
    if (bold) span.bold = true;
    if (italic) span.italic = true;
    if (mono) span.mono = true;
    if (mark !== null && i >= mark[0] && i < mark[1]) span.hit = true;
    chars.push(span);
    i += 1;
  }
  // Merge neighbours that share every flag.
  const out: Span[] = [];
  for (const c of chars) {
    const last = out[out.length - 1];
    if (
      last &&
      last.bold === c.bold &&
      last.italic === c.italic &&
      last.mono === c.mono &&
      last.hit === c.hit
    ) {
      last.text += c.text;
    } else out.push({ ...c });
  }
  return out;
}

// Rendered markdown per AssistantMessage row, and the rows first drawn in the
// current turn (the auto-read target). Module variables restart on reload; the
// atoms survive.
const rows = new Map<string, string>();
let turnRows: string[] = [];
let counter = 0;
type Run = {
  tokens: RowToken[];
  cursor: number;
  key: string;
  words: CaptionWord[];
  map: number[] | null;
  // The spoken word and the pane block that holds it, as last written.
  mark: string;
  block: string;
  startedAt: number;
  lastActive: number;
};
function freshRun(tokens: RowToken[], now: number): Run {
  return {
    tokens,
    cursor: 0,
    key: "",
    words: [],
    map: null,
    mark: "",
    block: "",
    startedAt: now,
    lastActive: 0
  };
}
let run = freshRun([], 0);

type ControlBody = { request_id?: string; text?: string; wait?: boolean };

// Resolves null on success, else why the request failed.
function post($: EngineInterface, path: string, body: ControlBody): Promise<string | null> {
  return $.http
    .fetch(CONTROL + path, {
      method: "POST",
      headers: { "content-type": "text/plain" },
      body: JSON.stringify(body)
    })
    .then((r) => (r.ok ? null : `HTTP ${r.status} ${r.text.slice(0, 200)}`))
    .catch(String);
}

// Tokens of whole rows, for a selection, whose speech is a piece of one row.
function rowTokens(ids: string[]): RowToken[] {
  return ids.flatMap((row) => speakable(rows.get(row) ?? "").tokens.map((t) => ({ ...t, row })));
}

// The key of block `n` of `row` in the captions pane.
function blockKey(row: string, n: number): string {
  return `b-${row}-${n}`;
}

// Moves the spoken word to `at` in `row`; null clears it. Writes only a change, and
// scrolls the pane only when the word enters another block.
// ponytail: scrolls per block, so a block taller than the pane shows its top; split blocks into lines if that bites.
async function paint($: EngineInterface, row: string, at: [number, number] | null) {
  const key = at === null ? "" : `${row}:${at[0]}:${at[1]}`;
  if (key === run.mark) return;
  run.mark = key;
  await update($, spoken, () => (at === null ? null : { row, at }));
  if (at === null) return;
  const n = blocks(rows.get(row) ?? "").findIndex((b) => at[0] >= b.start && at[0] < b.end);
  const block = n < 0 ? "" : blockKey(row, n);
  if (block === "" || block === run.block) return;
  // A pane not drawn yet refuses the scroll; the next word tries again.
  const moved = await $.ui
    .scroll({ in: PANE, to: { key: block }, block: "center" })
    .catch(() => ({ deny: "scroll failed" }));
  if (moved.deny === undefined) run.block = block;
}

async function clear($: EngineInterface) {
  await paint($, "", null);
  await update($, reading, () => null);
}

// Clears the reading a poll read, unless a newer reading has started since.
async function endIfCurrent($: EngineInterface, id: string) {
  if ((await read($, reading))?.id === id) await clear($);
}

// `$.http.fetch` gives up after 30 s with no answer. CopySpeech answers a
// wait:false /speak once the reading is audible; queued behind another
// reading, that can take longer, so a give-up means the reading still runs
// and the /status poll owns it.
const STILL_RUNNING = /aborted|no complete answer/i;

// The replies a reading covers, as the captions pane shows them.
function shownRows(ids: string[]): ShownRow[] {
  return ids.filter((row) => rows.has(row)).map((row) => ({ row, md: rows.get(row) ?? "" }));
}

// Speaks `text`; the captions pane shows `ids`, the AssistantMessage rows it comes from.
async function start($: EngineInterface, ids: string[], text: Spoken) {
  const current = await read($, reading);
  if (current !== null) await post($, "/stop", { request_id: current.id });
  if (text.speech.trim() === "" || ids.length === 0) return;
  counter += 1;
  const id = `claude.${Date.now()}.${counter}`;
  await paint($, "", null);
  run = freshRun(text.tokens, await $.clock.now());
  await update($, history, () => shownRows(ids));
  await update($, reading, (): Reading => ({ id, rows: ids, paused: false }));
  const failure = await post($, "/speak", { text: text.speech, request_id: id, wait: false });
  if (failure === null || STILL_RUNNING.test(failure)) return;
  $.ui.toast(`CopySpeech /speak failed: ${failure}`);
  if ((await read($, reading))?.id === id) await clear($);
}

// Opens the captions pane while captions are on. Called from a press it is asked, so
// the terminal places it at any width; from auto-read it waits for 144 columns.
async function openPane($: EngineInterface, captions: boolean) {
  if (captions) await $.ui.open({ id: PANE, title: "CopySpeech companion" }).catch(() => undefined);
}

// A /stop with no request_id is CopySpeech's global stop, as the tray's Stop
// reading: it also aborts every queued and running synthesis.
async function stop($: EngineInterface) {
  await clear($);
  await post($, "/stop", {});
}

async function poll($: EngineInterface) {
  const current = await read($, reading);
  if (current === null) return;
  const now = await $.clock.now();
  const res = await $.http.fetch(CONTROL + "/status").catch(() => null);
  if (res === null || !res.ok) return;
  let s: Status;
  try {
    // SAFETY: /status is CopySpeech's own control-server contract, and every
    // Status field is optional and read with a fallback.
    s = JSON.parse(res.text) as Status;
  } catch {
    return;
  }
  if (s.active !== true) {
    // A bare `{ active: false }` is also the app's answer after a silent stretch, so only
    // the idle wait ends a reading here.
    const quiet = run.lastActive > 0 && now - run.lastActive > IDLE_STOP_MS;
    const neverStarted = run.lastActive === 0 && now - run.startedAt > NO_MATCH_STOP_MS;
    if (quiet || neverStarted) await endIfCurrent($, current.id);
    return;
  }
  run.lastActive = now;
  const caption = s.captions ?? {};
  const key = `${s.fragment_index ?? 0}:${caption.text ?? ""}`;
  if (key !== run.key) {
    run.words = captionWords(caption);
    const a = align(run.words, run.tokens, run.cursor);
    // A miss stays unkeyed, so the next poll retries.
    run.map = a.ok ? a.map : null;
    if (a.ok) {
      run.key = key;
      run.cursor = a.cursor;
    }
  }
  const paused = s.paused === true;
  if (paused !== current.paused) {
    await update($, reading, (r) => (r?.id === current.id ? { ...r, paused } : r));
  }
  const pos = s.position_ms ?? 0;
  const i = run.words.findIndex((w) => pos >= w.start_ms && pos < w.end_ms);
  const at = i >= 0 && run.map !== null ? (run.map[i] ?? -1) : -1;
  const token = at >= 0 ? run.tokens[at] : undefined;
  // Hold the last word between words, as Hermes does.
  if (token !== undefined) await paint($, token.row, [token.start, token.end]);
}

// The plugin's own `/config` row for one `userConfig` field. The plugin's name can
// carry an `@inline` or marketplace suffix, so the key is matched by prefix.
async function settingRow($: EngineInterface, field: string) {
  return (await $.config.list()).find(
    (r) => r.key.startsWith("copyspeech") && r.key.endsWith(`.${field}`)
  );
}

// Flips the "Read new replies aloud" /config row, as if the person changed it there.
// Resolves the reply text.
async function toggleAutoRead($: EngineInterface): Promise<string> {
  const row = await settingRow($, "autoRead");
  if (row === undefined) return "CopySpeech auto-read setting not found.";
  const enabled = row.value !== true;
  const { deny } = await $.config.set({ key: row.key, value: enabled });
  if (deny !== undefined) return `CopySpeech auto-read unchanged: ${deny}`;
  return enabled ? "CopySpeech auto-read is on." : "CopySpeech auto-read is off.";
}

// Writes one of the plugin's settings from the captions pane. Resolves a deny's reason.
async function setSetting(
  $: EngineInterface,
  field: string,
  value: boolean | number | string
): Promise<string | undefined> {
  const row = await settingRow($, field);
  if (row === undefined) return `CopySpeech ${field} setting not found.`;
  const { deny } = await $.config.set({ key: row.key, value });
  return deny;
}

export const register: Register = (on, options) => {
  const showWord = options.captions !== false;
  const skip = options.skipSections === true;
  const excerpt: Excerpt | null =
    options.excerpt === false
      ? null
      : {
          mode: String(options.excerptMode ?? "both"),
          top: Number(options.excerptTop ?? 2),
          bottom: Number(options.excerptBottom ?? 2)
        };
  // The play button names what it reads: an excerpt while a setting trims the reply.
  const playLabel = skip || excerpt !== null ? "Read excerpt" : "Read aloud";
  const forRead = (ids: string[]) =>
    prepareSpeech(
      ids.map((row) => ({ row, md: rows.get(row) ?? "" })),
      skip,
      excerpt
    );
  // The plugin's settings as the captions pane draws them. A change reloads the plugin,
  // so these follow the new values.
  const settingValues = {
    autoRead: options.autoRead === true,
    skipSections: skip,
    excerpt: options.excerpt !== false,
    excerptMode: String(options.excerptMode ?? "both"),
    excerptTop: Number(options.excerptTop ?? 2),
    excerptBottom: Number(options.excerptBottom ?? 2),
    captions: showWord
  };

  on("session.start", async ($, e, next) => {
    await $.command.register({
      name: "copyspeech-stop",
      description: "Stop the CopySpeech reading"
    });
    await $.command.register({
      name: "copyspeech-autoread",
      description: "Turn reading new replies aloud on or off"
    });
    await $.command.register({
      name: "copyspeech-settings",
      description: "Open the CopySpeech companion pane with its settings"
    });
    const health = await $.http
      .fetch(CONTROL + "/health")
      .then((r) => (r.ok ? null : `HTTP ${r.status}`))
      .catch(String);
    if (health !== null) $.ui.toast(`CopySpeech /health failed: ${health}`);
    // ponytail: one global 120 ms timer that idles without a reading; start/stop it per reading if it ever shows in profiles.
    $.clock.every(POLL_MS, () => void poll($).catch(() => undefined));
    return next(e);
  });

  on("command.run", { command: "copyspeech-stop" }, async ($) => {
    await stop($);
    return { text: "Stopped." };
  });

  on("command.run", { command: "copyspeech-autoread" }, async ($) => ({
    text: await toggleAutoRead($)
  }));

  on("command.run", { command: "copyspeech-settings" }, async ($) => {
    await $.ui.open({ id: PANE, title: "CopySpeech companion" }).catch(() => undefined);
    return { text: "CopySpeech settings are in the companion pane." };
  });

  on("turn.start", async ($, e, next) => {
    if (e.agentId === undefined) turnRows = [];
    return next(e);
  });

  on("turn.complete", async ($, e, next) => {
    const result = await next(e);
    if (
      options.autoRead === true &&
      e.reason === "answer" &&
      e.agentId === undefined &&
      turnRows.length > 0
    ) {
      // Every text block the turn drew, joined, as one reading.
      const ids = turnRows;
      void $.clock.after(0, async () => {
        await openPane($, showWord);
        await start($, ids, forRead(ids)).catch(() => undefined);
      });
    }
    return result;
  });

  on("ui.render", { component: "AssistantMessage" }, async ($, e, next) => {
    if (e.props.isSummary === true) return next(e);
    if (!rows.has(e.requestId)) turnRows.push(e.requestId);
    rows.set(e.requestId, e.props.text);
    const r = await read($, reading);
    const isMine = r !== null && r.rows.includes(e.requestId);
    const ui = $.ui.resolve(e);
    const { Box, Button } = ui;
    // The engine draws the reply's own Markdown; the captions pane shows the spoken word.
    const body = await next(e);
    const markSvg =
      e.surface !== "terminal" && "Svg" in ui ? (
        <ui.Svg source={ICON_SVG} alt="CopySpeech" width={14} height={14} />
      ) : null;
    const play = (
      <Button
        key={`play-${e.requestId}`}
        label={markSvg === null ? `▶ ${playLabel}` : `${MARK_ROOM}${playLabel}`}
        onPress={() => {
          void openPane($, showWord);
          void $.clock.after(
            0,
            () => void start($, [e.requestId], forRead([e.requestId])).catch(() => undefined)
          );
        }}
      />
    );
    // A Button takes no Svg child, so the mark is an absolute Box painted over the
    // room at the start of the label: it sits inside the button and moves nothing.
    const playWithMark =
      markSvg === null ? (
        play
      ) : (
        <Box>
          {play}
          <Box position="absolute" left={0} top={0} bottom={0} alignItems="center" paddingLeft="sm">
            {markSvg}
          </Box>
        </Box>
      );
    // The play button is absolute: it leaves the flow, so the hover reveal moves
    // nothing. It stays inside the row's own bounds (bottom right): the desktop clips
    // an absolute Box at its row, so a negative offset hides it behind the row above.
    // It keeps one shape at every state and hides on the reply being read: that
    // reply's Pause, Resume and Stop sit in the band above the prompt.
    const REVEAL = { display: "flex" } as const;
    return (
      <Box key={`row-${e.requestId}`} flexDirection="column">
        {body}
        <Box
          position="absolute"
          bottom={0}
          right={0}
          alignItems="center"
          display="none"
          hover={isMine ? undefined : REVEAL}
        >
          {playWithMark}
        </Box>
      </Box>
    );
  });

  // The band above the prompt holds the reading's Pause, Resume and Stop. It reads
  // only `reading`, which changes at start, pause, resume and stop, so it never
  // redraws per caption word: the buttons stay still while the spoken word moves.
  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    const r = await read($, reading);
    if (r === null) return next(e);
    const { Box, Button, Text } = $.ui.resolve(e);
    return (
      <Box gap="xs" alignItems="center">
        <Text dimColor>CopySpeech reading</Text>
        <Button
          key="pause"
          label={r.paused ? "Resume" : "Pause"}
          onPress={() =>
            void $.clock.after(0, () =>
              post($, r.paused ? "/resume" : "/pause", { request_id: r.id }).then(() => undefined)
            )
          }
        />
        <Button
          key="stop"
          label="Stop"
          onPress={() => void $.clock.after(0, () => void stop($).catch(() => undefined))}
        />
      </Box>
    );
  });

  // The captions pane: the reading's replies as plain text, one keyed Box per block,
  // the chip on the spoken word. It alone reads `spoken`, so a word redraws it alone.
  // A fence or a table shows its raw text and gets no chip.
  on("ui.render", { component: "Pane", requestId: PANE }, async ($, e) => {
    const r = await read($, reading);
    const s = await read($, spoken);
    const list = await read($, history);
    const open = await read($, settingsOpen);
    const { Box, Button, Text } = $.ui.resolve(e);
    // A Button redraws with every spoken word, so the settings show only while nothing is read.
    const save = (field: string, value: boolean | number | string) => () =>
      void $.clock.after(0, async () => {
        const deny = await setSetting($, field, value).catch(() => undefined);
        if (deny !== undefined) $.ui.toast(`CopySpeech ${field} unchanged: ${deny}`);
      });
    const toggle = (field: "autoRead" | "skipSections" | "excerpt" | "captions", label: string) => (
      <Button
        key={field}
        label={`${label}: ${settingValues[field] ? "On" : "Off"}`}
        onPress={save(field, !settingValues[field])}
      />
    );
    const count = (field: "excerptTop" | "excerptBottom", label: string) => (
      <Box flexDirection="row" alignItems="center" gap="xs">
        <Text>{label}</Text>
        <Button
          key={`${field}-down`}
          label="-"
          onPress={save(field, Math.max(1, settingValues[field] - 1))}
        />
        <Text>{String(settingValues[field])}</Text>
        <Button
          key={`${field}-up`}
          label="+"
          onPress={save(field, Math.min(20, settingValues[field] + 1))}
        />
      </Box>
    );
    const nextMode =
      EXCERPT_MODES[(EXCERPT_MODES.indexOf(settingValues.excerptMode) + 1) % EXCERPT_MODES.length] ??
      "both";
    // Collapsed at the bottom of the pane until the person expands it.
    const settingsBody = (
      <Box flexDirection="column" gap={1}>
        <Box flexDirection="column" gap="xs">
          <Text dimColor>Reading</Text>
          <Box flexDirection="row" flexWrap="wrap" gap="xs">
            {toggle("autoRead", "Auto-read")}
            {toggle("skipSections", "Skip sections")}
            {toggle("excerpt", "Excerpt")}
            <Button
              key="excerptMode"
              label={`Part: ${settingValues.excerptMode}`}
              onPress={save("excerptMode", nextMode)}
            />
          </Box>
          <Box flexDirection="row" flexWrap="wrap" gap="xs">
            {count("excerptTop", "Top")}
            {count("excerptBottom", "Bottom")}
          </Box>
        </Box>
        <Box flexDirection="column" gap="xs">
          <Text dimColor>Captions</Text>
          <Box flexDirection="row" flexWrap="wrap" gap="xs">
            {toggle("captions", "Show captions")}
          </Box>
        </Box>
      </Box>
    );
    const settings = (
      <Box flexDirection="column" gap={1}>
        <Button
          key="settings"
          label={open ? "Settings ▾" : "Settings ▸"}
          onPress={() => void $.clock.after(0, () => void update($, settingsOpen, (v) => !v))}
        />
        {open ? settingsBody : null}
      </Box>
    );
    // Reads one paragraph of the reply, [from, to), with the captions for that reading.
    const readParagraph = (row: string, md: string, from: number, to: number) => () =>
      void $.clock.after(0, () => {
        rows.set(row, md);
        void start($, [row], prepareSpeech([{ row, md, from, to }], skip, excerpt)).catch(
          () => undefined
        );
      });
    // The reply the latest reading covers. A paragraph's ▶ shows on hover at its side.
    const replies = list.map((entry) => (
      <Box key={`reply-${entry.row}`} flexDirection="column" gap={2}>
        {blocks(entry.md).map((b, n) => {
          const raw = entry.md.slice(b.start, b.end);
          const isRaw = FENCE_LINE.test(raw) || /^\s*\|/.test(raw);
          const mark = s !== null && s.row === entry.row ? s.at : null;
          return (
            <Box key={blockKey(entry.row, n)}>
              <Box>
                {isRaw ? (
                  <Text mono>{raw}</Text>
                ) : (
                  <Text>
                    {inlineSpans(entry.md, b, mark).map((sp) => (
                      <Text
                        bold={sp.bold}
                        italic={sp.italic}
                        mono={sp.mono}
                        {...(sp.hit ? CHIP : {})}
                      >
                        {sp.text}
                      </Text>
                    ))}
                  </Text>
                )}
              </Box>
              <Box position="absolute" right={0} top={0} display="none" hover={{ display: "flex" }}>
                <Button
                  key={`read-${entry.row}-${n}`}
                  plain={true}
                  label="▶"
                  onPress={readParagraph(entry.row, entry.md, b.start, b.end)}
                />
              </Box>
            </Box>
          );
        })}
      </Box>
    ));
    // Settings show only while nothing is read: their Buttons would redraw with each spoken word.
    // The reply area takes the free height, so the settings sit at the bottom of the pane.
    return (
      <Box flexDirection="column" flexGrow={1} gap={1}>
        <Box flexDirection="column" flexGrow={1} gap={3}>
          {replies.length === 0 ? <Text dimColor>Nothing read yet.</Text> : replies}
        </Box>
        {r === null ? settings : null}
      </Box>
    );
  });

  on("ui.render", { component: "ContextMenu" }, async ($, e, next) => {
    const selected = e.props.selection?.text;
    const row = e.props.row;
    if (selected === undefined || selected.trim() === "") return next(e);
    const { Button, Text } = $.ui.resolve(e);
    // A selection inside a reply paints there; elsewhere it reads without captions.
    const target = row?.component === "AssistantMessage" ? row.requestId : "none";
    return (
      <Button
        key="read-selection"
        onPress={() => {
          if (target !== "none") void openPane($, showWord);
          void $.clock.after(
            0,
            () =>
              void start($, [target], { speech: selected, tokens: rowTokens([target]) }).catch(
                () => undefined
              )
          );
        }}
      >
        Read aloud with CopySpeech <Text dimColor>Speaks the selection</Text>
      </Button>
    );
  });
};
