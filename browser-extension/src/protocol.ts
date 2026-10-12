export type Status = "playing" | "paused" | "buffering" | "completed" | "cancelled" | "error";
/** What started a desktop speak the page may follow: CopySpeak's double-copy or hotkey. */
export type TriggerSource = "double_copy" | "hotkey";
type StateFields = {
  v: 1;
  type: "state";
  seq: number;
  status: Status;
  word: { start: number; end: number } | null;
  word_available: boolean;
};
export type NativeEvent =
  // `skip`: the app takes skip-back/skip-forward controls (older apps omit it).
  | { v: 1; type: "accepted"; request_id: string; reading_id: string; skip: boolean }
  | { v: 1; type: "rejected"; request_id: string; reason: string }
  | (StateFields & { reading_id: string })
  // A desktop trigger speak, mirrored: word offsets index into its raw_text.
  | (StateFields & { trigger_id: string })
  | {
      v: 1;
      type: "trigger-started";
      trigger_id: string;
      source: TriggerSource;
      raw_text: string;
    };
export type TriggerStarted = Extract<NativeEvent, { type: "trigger-started" }>;

export type ControlAction = "pause" | "resume" | "stop" | "skip-back" | "skip-forward";

/**
 * Messages exchanged over the extension's own chrome.runtime channel (content
 * script <-> service worker). Both ends live in this repo, so the channel
 * contract is declared here and every listener branches on these variants.
 */
export type CopyCommand =
  | { type: "capture"; request_id: string; paragraph?: boolean }
  | { type: "read-paragraph" }
  | { type: "site-access" }
  | { type: "refresh-site-access" }
  // `trigger_id` scopes a disconnect to one followed trigger speak.
  | { type: "disconnect"; trigger_id?: string }
  | { type: "native"; document_token: string; event: NativeEvent }
  // Worker -> every frame of every permitted tab: does your selection match?
  | { type: "trigger"; event: NativeEvent }
  // Frame -> worker: my selection matched; route this trigger's states here.
  | { type: "trigger-claim"; document_token: string; trigger_id: string }
  | { type: "control"; document_token: string; reading_id: string; action: ControlAction }
  | { type: "control"; document_token: string; trigger_id: string; action: ControlAction }
  | { type: "settings" };

/** Reply sent back by a content-script frame answering a capture command. */
export type CaptureReply = { text: string; document_token: string };

// Wire boundary helpers for chrome.runtime / native-host messages (the values
// chrome hands to listeners are untyped JSON, which chrome itself types as
// `any`). The prototype tag stands in for `typeof`, and the field readers
// return null instead of narrowing so every parser constructs domain values
// from checked fields rather than casting.

type WireMessage = Parameters<Parameters<(typeof chrome.runtime.onMessage)["addListener"]>[0]>[0];

const tag = (x: WireMessage): string => Object.prototype.toString.call(x).slice(8, -1);
const str = (x: WireMessage): string | null => (tag(x) === "String" ? x : null);
const int = (x: WireMessage): number | null => (Number.isSafeInteger(x) ? x : null);
const boolean = (x: WireMessage): boolean | null => (x === true || x === false ? x : null);
const bag = (x: WireMessage): Record<string, WireMessage> | null =>
  tag(x) === "Object" ? x : null;
const bounded = (x: string, max: number): string | null =>
  x.length > 0 && x.length <= max ? x : null;

const STATUSES: Status[] = ["playing", "paused", "buffering", "completed", "cancelled", "error"];

export const terminal = (status: Status) => ["completed", "cancelled", "error"].includes(status);

// SAFETY: membership in the Status list is the only fact behind the cast;
// the value already passed the prototype-tag string check.
const status = (x: string): Status | null =>
  (STATUSES as string[]).includes(x) ? (x as Status) : null;

const source = (x: string | null): TriggerSource | null =>
  x === "double_copy" || x === "hotkey" ? x : null;

const id = (x: WireMessage): string | null => {
  const raw = str(x);
  return raw === null ? null : bounded(raw, 128);
};

export function parseEvent(value: WireMessage): NativeEvent | null {
  const x = bag(value);
  if (x === null || x.v !== 1) return null;
  const requestId = id(x.request_id);
  if (x.type === "accepted") {
    const readingId = id(x.reading_id);
    if (requestId === null || readingId === null) return null;
    return {
      v: 1,
      type: "accepted",
      request_id: requestId,
      reading_id: readingId,
      skip: x.skip === true
    };
  }
  if (x.type === "trigger-started") {
    const triggerId = id(x.trigger_id);
    const triggerSource = source(str(x.source));
    const rawText = str(x.raw_text);
    if (
      triggerId === null ||
      triggerSource === null ||
      rawText === null ||
      bounded(rawText, 65536) === null ||
      rawText.trim().length === 0
    )
      return null;
    return {
      v: 1,
      type: "trigger-started",
      trigger_id: triggerId,
      source: triggerSource,
      raw_text: rawText
    };
  }
  if (x.type === "rejected") {
    const reason = str(x.reason);
    if (requestId === null || reason === null || reason.length > 512) return null;
    return { v: 1, type: "rejected", request_id: requestId, reason };
  }
  if (x.type !== "state") return null;
  const statusName = str(x.status);
  const statusValue = statusName === null ? null : status(statusName);
  const seq = int(x.seq);
  const wordAvailable = boolean(x.word_available);
  // A state frame belongs to exactly one reading or one trigger speak.
  const readingId = str(x.reading_id);
  const triggerId = id(x.trigger_id);
  if (
    (readingId === null) === (triggerId === null) ||
    statusValue === null ||
    seq === null ||
    seq <= 0 ||
    wordAvailable === null
  )
    return null;
  let word: { start: number; end: number } | null = null;
  if (x.word !== null) {
    const w = bag(x.word);
    const start = w === null ? null : int(w.start);
    const end = w === null ? null : int(w.end);
    if (w === null || start === null || end === null || start < 0 || end <= start || end > 131072)
      return null;
    if (!wordAvailable || terminal(statusValue) || statusValue === "buffering") return null;
    word = { start, end };
  }
  const fields: StateFields = {
    v: 1,
    type: "state",
    seq,
    status: statusValue,
    word,
    word_available: wordAvailable
  };
  return triggerId !== null
    ? { ...fields, trigger_id: triggerId }
    : { ...fields, reading_id: readingId ?? "" };
}

/** Parse a capture-report reply from a content-script frame. */
export function parseCaptureReply(value: WireMessage): CaptureReply | null {
  const x = bag(value);
  if (x === null) return null;
  const text = str(x.text);
  const documentToken = str(x.document_token);
  if (text === null || documentToken === null) return null;
  if (text.length === 0 || text.length > 65536 || text.trim().length === 0) return null;
  return { text, document_token: documentToken };
}

export class ReadingOwner {
  readingId: string | null = null;
  seq = 0;
  ended = false;
  constructor(readonly requestId: string) {}
  apply(value: WireMessage): boolean {
    const e = parseEvent(value);
    if (!e || this.ended) return false;
    if (e.type === "accepted") {
      if (e.request_id !== this.requestId || this.readingId !== null) return false;
      this.readingId = e.reading_id;
      return true;
    }
    if (e.type === "rejected" && e.request_id === this.requestId) {
      this.ended = true;
      return true;
    }
    if (
      e.type !== "state" ||
      !("reading_id" in e) ||
      e.reading_id !== this.readingId ||
      e.seq <= this.seq
    )
      return false;
    this.seq = e.seq;
    this.ended = terminal(e.status);
    return true;
  }
}

/**
 * One followed trigger speak (double-copy / hotkey), keyed by trigger_id the
 * way ReadingOwner keys a reading: ordered states only, nothing after a
 * terminal one. A newer trigger replaces the owner rather than updating it.
 */
export class TriggerOwner {
  seq = 0;
  ended = false;
  constructor(readonly triggerId: string) {}
  apply(value: WireMessage): boolean {
    const e = parseEvent(value);
    if (
      !e ||
      this.ended ||
      e.type !== "state" ||
      !("trigger_id" in e) ||
      e.trigger_id !== this.triggerId ||
      e.seq <= this.seq
    )
      return false;
    this.seq = e.seq;
    this.ended = terminal(e.status);
    return true;
  }
}
