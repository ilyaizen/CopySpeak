import { atom, read, update } from "claude-code";
import type { EngineInterface, Register } from "claude-code";

import type { Reading } from "../types";

// Port of copyspeech-hermes to Claude Code. CopySpeech's control server does the
// speaking; this mod draws the play button, the in-reply word highlight, the
// playback bar, and the Read aloud menu row.
const CONTROL = "http://127.0.0.1:43117";
const POLL_MS = 120;
const IDLE_STOP_MS = 2000;
const NO_MATCH_STOP_MS = 60000;
const ALIGN_WINDOW = 24;
const RUN_NEEDED = 3;
const EXCERPT_MIN = 1;
const EXCERPT_MAX = 20;

const reading = atom({ plugin: "copyspeech", key: "reading" } as const, null);

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

// Markdown that is not spoken: fenced and inline code, link targets, HTML tags.
const UNSPOKEN = /```[\s\S]*?(?:```|$)|`[^`\n]*`|\]\([^)\n]*\)|<[^>\n]+>/g;

export function wordTokens(text: string): Token[] {
  return [...text.matchAll(/[\p{L}\p{N}]+/gu)].map((m) => ({
    norm: m[0].toLowerCase(),
    start: m.index ?? 0,
    end: (m.index ?? 0) + m[0].length
  }));
}

// Spoken tokens of a reply's markdown (offsets into the markdown) and the text sent to CopySpeech.
export function speakable(md: string) {
  const skip = [...md.matchAll(UNSPOKEN)].map((m) => ({
    s: m.index ?? 0,
    e: (m.index ?? 0) + m[0].length
  }));
  const tokens = wordTokens(md).filter((t) => !skip.some((r) => t.start >= r.s && t.start < r.e));
  const speech = md
    .replace(UNSPOKEN, " ")
    .replace(/^[ \t]*(?:[-+*]|\d+[.)])[ \t]+/gm, "")
    .replace(/[#*_>|~]+/g, " ")
    .replace(/[ \t]+/g, " ")
    .trim();
  return { tokens, speech };
}

// Same rules as copyspeech-hermes dropSkippedMarkup: drop headings, bullet and
// numbered items, horizontal rules, and table rows. Text under a heading stays.
export function dropSkippedMarkup(md: string): string {
  const skipped = /^\s{0,3}(?:#{1,6}\s|[-*+]\s|\d+[.)]\s|\|| ?([-*_]) ?(?:\1 ?){2,}$)/;
  return md
    .split("\n")
    .filter((line) => !skipped.test(line))
    .join("\n");
}

export type Excerpt = { mode: string; top: number; bottom: number };

function clampCount(n: number): number {
  return Number.isFinite(n)
    ? Math.min(EXCERPT_MAX, Math.max(EXCERPT_MIN, Math.round(n)))
    : EXCERPT_MIN;
}

// Same rules as copyspeech-hermes excerptSpeech: keep the first and last blank-line paragraphs.
export function excerptSpeech(text: string, { mode, top, bottom }: Excerpt): string {
  const paras = text
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter((p) => p !== "");
  if (paras.length < 2) return text;
  const t = clampCount(top);
  const b = clampCount(bottom);
  if (mode === "top") return paras.slice(0, t).join("\n\n");
  if (mode === "bottom") return paras.slice(-b).join("\n\n");
  if (t + b >= paras.length) return text;
  return paras.slice(0, t).concat(paras.slice(-b)).join("\n\n");
}

// Speech for a play-button or automatic read. A selection reads in full and skips this.
export function prepareSpeech(md: string, skip: boolean, excerpt: Excerpt | null): string {
  const { speech } = speakable(skip ? dropSkippedMarkup(md) : md);
  return excerpt === null ? speech : excerptSpeech(speech, excerpt);
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

// The desktop renderer shows HTML tags literally, so the spoken word rides markdown syntax.
const MARKS = new Map([
  ["code", ["`", "`"]],
  ["bold", ["**", "**"]]
]);

export function markWord(md: string, range: [number, number], style: string): string {
  const pair = MARKS.get(style);
  if (pair === undefined) return md;
  const [s, e] = range;
  return md.slice(0, s) + pair[0] + md.slice(s, e) + pair[1] + md.slice(e);
}

// Markdown cannot paint a background and the desktop shows HTML literally, so
// the chip style draws the block holding the spoken word as Text spans. A
// background changes no glyph metrics, so words after the chip never move.
// Desktop: Hermes's ::highlight(copyspeech-word) paints the accent at 30 % over
// the word and keeps its text color; 8-digit hex carries that alpha in light and
// dark themes. Terminal: a solid accent chip with inverse text, since it has no alpha.
const CHIP_DESKTOP = { backgroundColor: "#D777574D" };
const CHIP_TERMINAL = { backgroundColor: "claude", color: "inverseText" };

export type Block = { start: number; end: number };
export type Span = { text: string; bold?: true; italic?: true; mono?: true; hit?: true };

// Blank-line separated blocks, never splitting a fenced code block.
export function blocks(md: string): Block[] {
  const out: Block[] = [];
  let from = -1;
  let inFence = false;
  let offset = 0;
  for (const line of md.split("\n")) {
    const end = offset + line.length;
    if (/^\s*```/.test(line)) inFence = !inFence;
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
// ponytail: covers headings, list markers, **bold**, *italic*, `code`, [links](url); tables and nested quotes draw raw.
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
    const hit = mark !== null && i >= mark[0] && i < mark[1];
    const span: Span = { text: ch };
    if (bold) span.bold = true;
    if (italic) span.italic = true;
    if (mono) span.mono = true;
    if (hit) span.hit = true;
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
// reading atom survives.
const rows = new Map<string, string>();
let turnRows: string[] = [];
let counter = 0;
type Run = {
  tokens: RowToken[];
  cursor: number;
  key: string;
  words: CaptionWord[];
  map: number[] | null;
  startedAt: number;
  lastActive: number;
};
let run: Run = {
  tokens: [],
  cursor: 0,
  key: "",
  words: [],
  map: null,
  startedAt: 0,
  lastActive: 0
};

type ControlBody = { request_id: string; text?: string; wait?: boolean; delta_ms?: number };

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

function rowsOf(r: Reading): string[] {
  return r.rows ?? [r.row];
}

// `$.http.fetch` gives up after 30 s with no answer. CopySpeech answers a
// wait:false /speak once the reading is audible; queued behind another
// reading, that can take longer, so a give-up means the reading still runs
// and the /status poll owns it.
const STILL_RUNNING = /aborted|no complete answer/i;

// Speaks `speech`; captions paint in `ids`, the AssistantMessage rows it comes from.
async function start($: EngineInterface, ids: string[], speech: string) {
  const current = await read($, reading);
  if (current !== null) await post($, "/stop", { request_id: current.id });
  if (speech.trim() === "" || ids.length === 0) return;
  const tokens = ids.flatMap((row) =>
    speakable(rows.get(row) ?? "").tokens.map((t) => ({ ...t, row }))
  );
  counter += 1;
  const id = `claude.${Date.now()}.${counter}`;
  const now = await $.clock.now();
  run = { tokens, cursor: 0, key: "", words: [], map: null, startedAt: now, lastActive: 0 };
  const first = ids[0] ?? "";
  await update($, reading, (): Reading => ({
    id,
    row: first,
    rows: ids,
    paused: false,
    mark: null
  }));
  const failure = await post($, "/speak", { text: speech, request_id: id, wait: false });
  if (failure === null || STILL_RUNNING.test(failure)) return;
  $.ui.toast(`CopySpeech /speak failed: ${failure}`);
  await update($, reading, (r) => (r?.id === id ? null : r));
}

async function stop($: EngineInterface) {
  const current = await read($, reading);
  if (current === null) return;
  await update($, reading, () => null);
  await post($, "/stop", { request_id: current.id });
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
    const quiet = run.lastActive > 0 && now - run.lastActive > IDLE_STOP_MS;
    const neverStarted = run.lastActive === 0 && now - run.startedAt > NO_MATCH_STOP_MS;
    if (quiet || neverStarted) await update($, reading, () => null);
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
  const pos = s.position_ms ?? 0;
  const i = run.words.findIndex((w) => pos >= w.start_ms && pos < w.end_ms);
  const at = i >= 0 && run.map !== null ? (run.map[i] ?? -1) : -1;
  const token = at >= 0 ? run.tokens[at] : undefined;
  const paused = s.paused === true;
  await update($, reading, (r) => {
    if (r === null || r.id !== current.id) return r;
    // Hold the last word between words, as Hermes does.
    const mark: [number, number] | null = token ? [token.start, token.end] : r.mark;
    const row = token ? token.row : r.row;
    const same =
      r.paused === paused &&
      r.row === row &&
      r.mark?.[0] === mark?.[0] &&
      r.mark?.[1] === mark?.[1];
    return same ? r : { ...r, paused, row, mark };
  });
}

// Flips the "Read new replies aloud" /config row, as if the person changed it
// there. The row's key is `<plugin>.autoRead`, and the plugin's name can carry
// an `@inline` or marketplace suffix. Resolves the reply text.
async function toggleAutoRead($: EngineInterface): Promise<string> {
  const row = (await $.config.list()).find(
    (r) => r.key.startsWith("copyspeech") && r.key.endsWith(".autoRead")
  );
  if (row === undefined) return "CopySpeech auto-read setting not found.";
  const enabled = row.value !== true;
  const { deny } = await $.config.set({ key: row.key, value: enabled });
  if (deny !== undefined) return `CopySpeech auto-read unchanged: ${deny}`;
  return enabled ? "CopySpeech auto-read is on." : "CopySpeech auto-read is off.";
}

export const register: Register = (on, options) => {
  const style = String(options.highlight ?? "chip");
  const skip = options.skipSections === true;
  const excerpt: Excerpt | null =
    options.excerpt === false
      ? null
      : {
          mode: String(options.excerptMode ?? "both"),
          top: Number(options.excerptTop ?? 2),
          bottom: Number(options.excerptBottom ?? 2)
        };
  const forRead = (ids: string[]) =>
    prepareSpeech(ids.map((row) => rows.get(row) ?? "").join("\n\n"), skip, excerpt);

  on("session.start", async ($, e, next) => {
    await $.command.register({
      name: "copyspeech-stop",
      description: "Stop the CopySpeech reading"
    });
    await $.command.register({
      name: "copyspeech-autoread",
      description: "Turn reading new replies aloud on or off"
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
      void $.clock.after(0, () => void start($, ids, forRead(ids)).catch(() => undefined));
    }
    return result;
  });

  on("ui.render", { component: "AssistantMessage" }, async ($, e, next) => {
    if (e.props.isSummary === true) return next(e);
    if (!rows.has(e.requestId)) turnRows.push(e.requestId);
    rows.set(e.requestId, e.props.text);
    const r = await read($, reading);
    const isMine = r !== null && rowsOf(r).includes(e.requestId);
    const mark = isMine && r.row === e.requestId ? r.mark : null;
    const { Box, Button, Markdown, Text } = $.ui.resolve(e);
    const chip = e.surface === "desktop" ? CHIP_DESKTOP : CHIP_TERMINAL;
    let body;
    if (isMine && style === "chip") {
      // Prose blocks become Text for the whole reading, so the layout switches once, not per word.
      const md = e.props.text;
      body = (
        <Box flexDirection="column" gap={1}>
          {blocks(md).map((b, n) => {
            const raw = md.slice(b.start, b.end);
            if (/^\s*```/.test(raw) || /^\s*\|/.test(raw))
              return <Markdown key={`b${n}`} text={raw} />;
            return (
              <Text key={`b${n}`}>
                {inlineSpans(md, b, mark).map((s) => (
                  <Text bold={s.bold} italic={s.italic} mono={s.mono} {...(s.hit ? chip : {})}>
                    {s.text}
                  </Text>
                ))}
              </Text>
            );
          })}
        </Box>
      );
    } else {
      const marked =
        mark !== null && style !== "off"
          ? { ...e, props: { ...e.props, text: markWord(e.props.text, mark, style) } }
          : e;
      body = await next(marked);
    }
    const key = `row-${e.requestId}`;
    return (
      <Box key={key} flexDirection="column">
        {body}
        <Box {...(isMine ? {} : { display: "none" as const, hover: { display: "flex" as const } })}>
          <Button
            key={`play-${e.requestId}`}
            label={isMine ? "■ Stop reading" : "▶ Read aloud"}
            onPress={() =>
              void $.clock.after(
                0,
                () =>
                  void (isMine ? stop($) : start($, [e.requestId], forRead([e.requestId]))).catch(
                    () => undefined
                  )
              )
            }
          />
        </Box>
      </Box>
    );
  });

  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    const r = await read($, reading);
    if (r === null) return next(e);
    const { Box, Button, Text } = $.ui.resolve(e);
    return (
      <Box gap={1}>
        <Text dimColor>CopySpeech</Text>
        <Button
          key="pause"
          label={r.paused ? "▶ Resume" : "⏸ Pause"}
          onPress={() =>
            void $.clock.after(
              0,
              () => void post($, r.paused ? "/resume" : "/pause", { request_id: r.id })
            )
          }
        />
        <Button
          key="back"
          label="−5s"
          onPress={() =>
            void $.clock.after(
              0,
              () => void post($, "/seek", { request_id: r.id, delta_ms: -5000 })
            )
          }
        />
        <Button
          key="fwd"
          label="+5s"
          onPress={() =>
            void $.clock.after(0, () => void post($, "/seek", { request_id: r.id, delta_ms: 5000 }))
          }
        />
        <Button
          key="stop"
          label="■ Stop"
          onPress={() => void $.clock.after(0, () => void stop($).catch(() => undefined))}
        />
        <Button
          key="show"
          label="Show spoken word"
          onPress={() => $.ui.scroll({ to: { requestId: r.row }, block: "center" })}
        />
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
        onPress={() =>
          void $.clock.after(0, () => void start($, [target], selected).catch(() => undefined))
        }
      >
        Read aloud with CopySpeech <Text dimColor>Speaks the selection</Text>
      </Button>
    );
  });
};
