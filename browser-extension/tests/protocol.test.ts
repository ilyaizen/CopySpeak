import { test, expect } from "bun:test";
import { parseEvent, ReadingOwner, TriggerOwner } from "../src/protocol";
test("binds acceptance to request and rejects stale/wrong-reading transitions", () => {
  const owner = new ReadingOwner("request");
  expect(owner.apply({ v: 1, type: "accepted", request_id: "other", reading_id: "r" })).toBe(false);
  expect(owner.apply({ v: 1, type: "accepted", request_id: "request", reading_id: "r" })).toBe(
    true
  );
  const state = {
    v: 1,
    type: "state",
    reading_id: "r",
    seq: 2,
    status: "playing",
    word: { start: 2, end: 6 },
    word_available: true
  };
  expect(owner.apply(state)).toBe(true);
  expect(owner.apply({ ...state, seq: 1 })).toBe(false);
  expect(owner.apply({ ...state, seq: 3, reading_id: "other" })).toBe(false);
  expect(owner.apply({ ...state, seq: 3, status: "completed", word: null })).toBe(true);
  expect(owner.apply({ ...state, seq: 4 })).toBe(false);
  for (const invalid of [
    { ...state, v: 2 },
    { ...state, word: { start: 2, end: 2 } },
    { ...state, seq: NaN },
    null,
    []
  ])
    expect(parseEvent(invalid)).toBeNull();
});
test("parses trigger speaks and keeps their states apart from readings", () => {
  const started = {
    v: 1,
    type: "trigger-started",
    trigger_id: "trigger-1-0",
    source: "double_copy",
    raw_text: "Copied\r\n\r\ntext"
  } as const;
  expect(parseEvent(started)).toEqual(started);
  expect(parseEvent({ ...started, source: "hotkey" })?.type).toBe("trigger-started");
  for (const invalid of [
    { ...started, source: "tray" },
    { ...started, raw_text: " \n" },
    { ...started, raw_text: "x".repeat(65537) },
    { ...started, trigger_id: "" }
  ])
    expect(parseEvent(invalid)).toBeNull();

  const state = {
    v: 1,
    type: "state",
    trigger_id: "trigger-1-0",
    seq: 1,
    status: "playing",
    word: { start: 0, end: 6 },
    word_available: true
  } as const;
  expect(parseEvent(state)).toEqual(state);
  expect(parseEvent({ ...state, reading_id: "r" })).toBeNull();
  expect(parseEvent({ ...state, trigger_id: undefined })).toBeNull();

  const owner = new TriggerOwner("trigger-1-0");
  expect(owner.apply({ ...state, trigger_id: "older" })).toBe(false);
  expect(owner.apply({ ...state, trigger_id: undefined, reading_id: "trigger-1-0" })).toBe(false);
  expect(owner.apply(state)).toBe(true);
  expect(owner.apply(state)).toBe(false);
  expect(owner.apply({ ...state, seq: 2, status: "cancelled", word: null })).toBe(true);
  expect(owner.ended).toBe(true);
  expect(owner.apply({ ...state, seq: 3 })).toBe(false);

  // A reading owner never takes a trigger's state, even with a colliding id.
  const reading = new ReadingOwner("request");
  reading.apply({ v: 1, type: "accepted", request_id: "request", reading_id: "trigger-1-0" });
  expect(reading.apply(state)).toBe(false);
});
test("accepted frames advertise skip support only when the app sends it", () => {
  const accepted = { v: 1, type: "accepted", request_id: "q", reading_id: "r" } as const;
  expect(parseEvent(accepted)).toEqual({ ...accepted, skip: false });
  expect(parseEvent({ ...accepted, skip: true })).toEqual({ ...accepted, skip: true });
});
