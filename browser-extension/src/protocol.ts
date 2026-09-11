export type Status = "playing" | "paused" | "buffering" | "completed" | "cancelled" | "error";
export type NativeEvent =
  | { v: 1; type: "accepted"; request_id: string; reading_id: string }
  | { v: 1; type: "rejected"; request_id: string; reason: string }
  | { v: 1; type: "probe"; request_id: string }
  | {
      v: 1;
      type: "state";
      reading_id: string;
      seq: number;
      status: Status;
      word: { start: number; end: number } | null;
      word_available: boolean;
    };

export type ControlAction = "pause" | "resume" | "stop";

/**
 * Messages exchanged over the extension's own chrome.runtime channel (content
 * script <-> service worker). Both ends live in this repo, so the channel
 * contract is declared here and every listener branches on these variants.
 */
export type CopyCommand =
  | { type: "capture"; request_id: string; probe?: boolean }
  | { type: "disconnect" }
  | { type: "native"; document_token: string; event: NativeEvent }
  | { type: "control"; document_token: string; reading_id: string; action: ControlAction }
  | { type: "settings" };

/** Reply sent back by a content-script frame answering a capture command. */
export type CaptureReply = { text: string; document_token: string };

// Wire boundary helpers for chrome.runtime / native-host messages (the values
// chrome hands to listeners are untyped JSON, which chrome itself types as
// `any`). The prototype tag stands in for `typeof`, and the field readers
// return null instead of narrowing so every parser constructs domain values
// from checked fields rather than casting.

type WireMessage = Parameters<
  Parameters<(typeof chrome.runtime.onMessage)["addListener"]>[0]
>[0];

const tag = (x: WireMessage): string => Object.prototype.toString.call(x).slice(8, -1);
const str = (x: WireMessage): string | null => (tag(x) === "String" ? x : null);
const int = (x: WireMessage): number | null => (Number.isSafeInteger(x) ? x : null);
const boolean = (x: WireMessage): boolean | null => (x === true || x === false ? x : null);
const bag = (x: WireMessage): WireMessage | null => (tag(x) === "Object" ? x : null);
const bounded = (x: string, max: number): string | null =>
  x.length > 0 && x.length <= max ? x : null;

const STATUSES: Status[] = ["playing", "paused", "buffering", "completed", "cancelled", "error"];

export const terminal = (status: Status) => ["completed", "cancelled", "error"].includes(status);

// SAFETY: membership in the Status list is the only fact behind the cast;
// the value already passed the prototype-tag string check.
const status = (x: string): Status | null =>
  (STATUSES as string[]).includes(x) ? (x as Status) : null;

export function parseEvent(value: WireMessage): NativeEvent | null {
  const x = bag(value);
  if (x === null || x.v !== 1) return null;
  const requestId = str(x.request_id) === null ? null : bounded(x.request_id, 128);
  if (x.type === "accepted") {
    const readingId = str(x.reading_id) === null ? null : bounded(x.reading_id, 128);
    if (requestId === null || readingId === null) return null;
    return { v: 1, type: "accepted", request_id: requestId, reading_id: readingId };
  }
  if (x.type === "rejected") {
    const reason = str(x.reason);
    if (requestId === null || reason === null || reason.length > 512) return null;
    return { v: 1, type: "rejected", request_id: requestId, reason };
  }
  if (x.type === "probe") {
    return requestId !== null ? { v: 1, type: "probe", request_id: requestId } : null;
  }
  if (x.type !== "state") return null;
  const statusValue = str(x.status) === null ? null : status(x.status);
  const seq = int(x.seq);
  const wordAvailable = boolean(x.word_available);
  if (
    str(x.reading_id) === null ||
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
    if (
      w === null ||
      start === null ||
      end === null ||
      start < 0 ||
      end <= start ||
      end > 131072
    )
      return null;
    if (!wordAvailable || terminal(statusValue) || statusValue === "buffering") return null;
    word = { start, end };
  }
  return {
    v: 1,
    type: "state",
    reading_id: str(x.reading_id) ?? "",
    seq,
    status: statusValue,
    word,
    word_available: wordAvailable
  };
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
    if (e.type !== "state" || e.reading_id !== this.readingId || e.seq <= this.seq) return false;
    this.seq = e.seq;
    this.ended = terminal(e.status);
    return true;
  }
}
