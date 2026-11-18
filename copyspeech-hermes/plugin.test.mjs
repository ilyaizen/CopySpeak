// Run from this folder: node plugin.test.mjs
// plugin.js imports @hermes/plugin-sdk, which Node cannot resolve. The Hermes
// loader rejects relative imports, so this test strips the import lines into a
// temp copy and loads that copy.
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(join(here, "plugin.js"), "utf8");
const stripped = source.replace(/^import\s[\s\S]*?from\s+['"][^'"]+['"];?\s*$/gm, "");

const scratch = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "copyspeech-hermes-test-"));
const copy = join(scratch, "plugin-under-test.mjs");
// Test-only hooks, appended to the copy: they expose the module's caption state.
const hooks = [
  "export function liveCaptionRun() { return captionRun; }",
  "export function livePill() { return pill; }",
  "export function playbackRequestIdNow() { return playbackRequestId; }",
  "export function clearPlayback() { playbackForget(); }",
  "export { alignDetail, startCaptions, stopCaptions, livePillEnv, makeMessageButton, selectionInsideReply, playbackRemember, speechForRead, prepareSpeech };"
].join("\n");
writeFileSync(copy, stripped + "\n" + hooks + "\n");
let mod;
try {
  mod = await import(pathToFileURL(copy).href);
} finally {
  rmSync(scratch, { recursive: true, force: true });
}

const {
  toSpeech,
  excerptSpeech,
  prepareSpeech,
  speechForRead,
  dropSkippedMarkup,
  barStateFor,
  speakVerdict,
  withClaim,
  normalizeSettings,
  clampCount,
  wordTokens,
  captionWords,
  activeWordIndex,
  alignDetail,
  pendingEdits,
  draftView,
  applyEdit,
  createGlidePill,
  measureWord,
  pollPlan,
  startCaptions,
  stopCaptions,
  livePillEnv,
  liveCaptionRun,
  livePill,
  playbackRequestIdNow,
  clearPlayback
} = mod;
const plugin = mod.default;
let passed = 0;
const cases = [];
// Cases run in order at the end, so async cases can await.
function test(name, fn) {
  cases.push({ name, fn });
}

const ON = { autoRead: true, excerpt: true, excerptMode: "both", excerptTop: 2, excerptBottom: 2 };

function happyState(overrides = {}) {
  return {
    settings: ON,
    focusedSessionId: "s1",
    armed: new Set(["s1"]),
    claims: {},
    builtInOn: false,
    ...overrides
  };
}

function happyEvent(overrides = {}) {
  return {
    type: "message.complete",
    session_id: "s1",
    seq: 5,
    replayed: false,
    payload: { status: "complete", text: "Hello there." },
    ...overrides
  };
}

const para = ["a", "b", "c", "d"].join("\n\n");

test("excerpt both keeps first and last paragraph", () => {
  assert.equal(excerptSpeech(para, "both", 1, 1), "a\n\nd");
});
test("excerpt top keeps first two paragraphs", () => {
  assert.equal(excerptSpeech(para, "top", 2, 2), "a\n\nb");
});
test("excerpt bottom keeps last two paragraphs", () => {
  assert.equal(excerptSpeech(para, "bottom", 2, 2), "c\n\nd");
});
test("excerpt returns short text unchanged", () => {
  assert.equal(excerptSpeech("one block only", "both", 1, 1), "one block only");
});
test("message read keeps top two and bottom two of five paragraphs", () => {
  const five = "one\n\ntwo\n\nthree\n\nfour\n\nfive";
  assert.equal(speechForRead("message", five, ON), "one\n\ntwo\n\nfour\n\nfive");
});
test("message read reads in full when excerpt is off", () => {
  const five = "one\n\ntwo\n\nthree\n\nfour\n\nfive";
  assert.equal(speechForRead("message", five, { ...ON, excerpt: false }), five);
});
test("selection read ignores excerpt settings and reads in full", () => {
  const five = "one\n\ntwo\n\nthree\n\nfour\n\nfive";
  assert.equal(speechForRead("selection", five, ON), five);
});
const markup =
  "## Answer\n\nIntro line.\n\n- first\n- second\n\n1. one\n\n---\n\n| a | b |\n\nClosing.";
test("dropSkippedMarkup removes headings, bullets, numbered items, rules, and table rows", () => {
  assert.equal(
    dropSkippedMarkup(markup),
    ["", "Intro line.", "", "", "", "", "", "Closing."].join("\n")
  );
});
test("dropSkippedMarkup keeps bold text, hashtags, and dashes inside a sentence", () => {
  assert.equal(
    dropSkippedMarkup("**Bold** start\n#tag here\n-5 degrees\nA - B"),
    "**Bold** start\n#tag here\n-5 degrees\nA - B"
  );
});
test("prepareSpeech with skip on reads the text under a heading and drops the list", () => {
  assert.equal(
    prepareSpeech(markup, { excerpt: false, skipMarkdownSections: true }),
    "Intro line.\n\nClosing."
  );
});
test("prepareSpeech with skip off keeps the list items", () => {
  assert.equal(
    prepareSpeech("## Answer\n\n- first", { excerpt: false, skipMarkdownSections: false }).includes(
      "first"
    ),
    true
  );
});
test("barStateFor hides the bar when no reading plays", () => {
  assert.deepEqual(barStateFor(null, true), { active: false, paused: false });
});
test("barStateFor shows a paused reading as paused", () => {
  assert.deepEqual(barStateFor("req-1", true), { active: true, paused: true });
});
test("barStateFor shows a playing reading as not paused", () => {
  assert.deepEqual(barStateFor("req-1", false), { active: true, paused: false });
});
test("selection read ignores the skip setting and reads in full", () => {
  assert.equal(speechForRead("selection", markup, { ...ON, skipMarkdownSections: true }), markup);
});
test("excerpt returns whole text when top and bottom overlap", () => {
  const three = "a\n\nb\n\nc";
  assert.equal(excerptSpeech(three, "both", 2, 2), three);
});
test("clamp keeps counts between 1 and 20", () => {
  assert.equal(clampCount(0), 1);
  assert.equal(clampCount(99), 20);
  assert.equal(clampCount(7.4), 7);
});
test("toSpeech strips a fenced code block", () => {
  const out = toSpeech("Intro line.\n\n```js\nconst x = 1\n```\n\nOutro line.");
  assert.ok(out.includes("Intro line."));
  assert.ok(out.includes("Outro line."));
  assert.ok(!out.includes("const x"));
  assert.ok(!out.includes("```"));
});
test("toSpeech keeps link labels and drops URLs", () => {
  assert.equal(toSpeech("See [the docs](https://example.com)."), "See the docs.");
});
test("normalizeSettings applies the documented defaults", () => {
  assert.deepEqual(normalizeSettings(undefined), {
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
  });
});
test("normalizeSettings keeps a saved skipMarkdownSections value and rejects a non-boolean", () => {
  assert.equal(normalizeSettings({ skipMarkdownSections: true }).skipMarkdownSections, true);
  assert.equal(normalizeSettings({ skipMarkdownSections: "yes" }).skipMarkdownSections, false);
});
test("normalizeSettings keeps a saved follow value and rejects a non-boolean", () => {
  assert.equal(normalizeSettings({ follow: true }).follow, true);
  assert.equal(normalizeSettings({ follow: "yes" }).follow, false);
});
test("normalizeSettings keeps a saved glide value and rejects a non-boolean", () => {
  assert.equal(normalizeSettings({ glide: true }).glide, true);
  assert.equal(normalizeSettings({ glide: "yes" }).glide, false);
});
test("normalizeSettings keeps a saved captions value and rejects a non-boolean", () => {
  assert.equal(normalizeSettings({ captions: false }).captions, false);
  assert.equal(normalizeSettings({ captions: "yes" }).captions, true);
});

test("gate: happy path speaks", () => {
  assert.equal(speakVerdict(happyEvent(), happyState()).speak, true);
});
test("gate: replayed event is skipped", () => {
  assert.equal(speakVerdict(happyEvent({ replayed: true }), happyState()).speak, false);
});
test("gate: unarmed session is skipped", () => {
  assert.equal(speakVerdict(happyEvent(), happyState({ armed: new Set() })).speak, false);
});
test("gate: error status is skipped", () => {
  assert.equal(
    speakVerdict(happyEvent({ payload: { status: "error", text: "boom" } }), happyState()).speak,
    false
  );
});
test("gate: non-focused session is skipped", () => {
  assert.equal(speakVerdict(happyEvent(), happyState({ focusedSessionId: "s2" })).speak, false);
});
test("gate: duplicate claim is skipped", () => {
  const claims = { "claim.s1.5": 1 };
  assert.equal(speakVerdict(happyEvent(), happyState({ claims })).speak, false);
});
test("gate: built-in auto-read on is skipped", () => {
  const verdict = speakVerdict(happyEvent(), happyState({ builtInOn: true }));
  assert.equal(verdict.speak, false);
  assert.equal(verdict.reason, "Hermes built-in auto-read is on");
});
test("gate: autoRead off is skipped", () => {
  assert.equal(
    speakVerdict(happyEvent(), happyState({ settings: { ...ON, autoRead: false } })).speak,
    false
  );
});
test("gate: consumes the armed flag once", () => {
  const state = happyState();
  assert.equal(speakVerdict(happyEvent(), state).speak, true);
  assert.equal(state.armed.has("s1"), false);
});

test("withClaim keeps at most 50 newest claims", () => {
  let claims = {};
  for (let i = 0; i < 60; i += 1) claims = withClaim(claims, "claim.s." + i, i);
  const keys = Object.keys(claims);
  assert.equal(keys.length, 50);
  assert.ok(!("claim.s.0" in claims));
  assert.ok("claim.s.59" in claims);
});

// Caption helpers. Entries use the /status shape: character ranges in caption.text.
function wordEntries(text, spans) {
  return spans.map(([start, end, startMs, endMs]) => ({
    text_start: start,
    text_end: end,
    start_ms: startMs,
    end_ms: endMs
  }));
}
function captionOf(text, entries) {
  return { text, words: entries };
}
const norms = (tokens) => tokens.map((token) => token.norm);

test("wordTokens splits letters and digits and drops punctuation", () => {
  assert.deepEqual(norms(wordTokens("Don't stop, 42 times!")), ["don", "t", "stop", "42", "times"]);
});

test("captionWords keeps plain word-level timings", () => {
  const caption = captionOf(
    "Hello there.",
    wordEntries("Hello there.", [
      [0, 5, 0, 400],
      [6, 11, 450, 900]
    ])
  );
  assert.deepEqual(captionWords(caption), [
    { norm: "hello", start_ms: 0, end_ms: 400 },
    { norm: "there", start_ms: 450, end_ms: 900 }
  ]);
});

test("captionWords keeps a trailing punctuation mark inside the word timing", () => {
  const caption = captionOf(
    "Hi, you",
    wordEntries("Hi, you", [
      [0, 3, 0, 300],
      [4, 7, 350, 600]
    ])
  );
  assert.deepEqual(captionWords(caption), [
    { norm: "hi", start_ms: 0, end_ms: 300 },
    { norm: "you", start_ms: 350, end_ms: 600 }
  ]);
});

test("captionWords groups per-character entries under each word", () => {
  const text = "Hi you";
  const chars = [
    [0, 1, 0, 100],
    [1, 2, 100, 200],
    [2, 3, 200, 250],
    [3, 4, 250, 300],
    [4, 5, 300, 350],
    [5, 6, 350, 400]
  ];
  assert.deepEqual(captionWords(captionOf(text, wordEntries(text, chars))), [
    { norm: "hi", start_ms: 0, end_ms: 200 },
    { norm: "you", start_ms: 250, end_ms: 400 }
  ]);
});

test("captionWords skips a word that has no timing", () => {
  const caption = captionOf("Hi you", wordEntries("Hi you", [[0, 2, 0, 200]]));
  assert.deepEqual(captionWords(caption), [{ norm: "hi", start_ms: 0, end_ms: 200 }]);
});

test("activeWordIndex holds -1 in a gap between words", () => {
  const words = [
    { norm: "a", start_ms: 0, end_ms: 100 },
    { norm: "b", start_ms: 150, end_ms: 250 }
  ];
  assert.equal(activeWordIndex(words, 50), 0);
  assert.equal(activeWordIndex(words, 120), -1);
  assert.equal(activeWordIndex(words, 200), 1);
  assert.equal(activeWordIndex(words, 999), -1);
});

test("alignDetail maps plain words in order", () => {
  const tokens = wordTokens("The quick brown fox.");
  const words = [
    { norm: "the", start_ms: 0, end_ms: 1 },
    { norm: "quick", start_ms: 1, end_ms: 2 },
    { norm: "brown", start_ms: 2, end_ms: 3 },
    { norm: "fox", start_ms: 3, end_ms: 4 }
  ];
  const detail = alignDetail(words, tokens, 0);
  assert.equal(detail.ok, true);
  assert.deepEqual(detail.map, [0, 1, 2, 3]);
});

test("alignDetail does not anchor a common word inside a skipped excerpt gap", () => {
  // Top + bottom excerpt: the caption skips the middle paragraph. "the" appears
  // inside the skipped gap, close enough to match, but the bottom's "the model"
  // run only follows the later occurrence.
  const filler = Array.from({ length: 30 }, (_, i) => "filler" + i).join(" ");
  const text = "alpha beta the cat sat in a box " + filler + " the model is good";
  const tokens = wordTokens(text);
  const bottomThe = tokens.map((t) => t.norm).lastIndexOf("the");
  const words = [
    { norm: "alpha", start_ms: 0, end_ms: 1 },
    { norm: "beta", start_ms: 1, end_ms: 2 },
    { norm: "the", start_ms: 2, end_ms: 3 },
    { norm: "model", start_ms: 3, end_ms: 4 },
    { norm: "is", start_ms: 4, end_ms: 5 },
    { norm: "good", start_ms: 5, end_ms: 6 }
  ];
  const detail = alignDetail(words, tokens, 0);
  assert.equal(detail.ok, true);
  assert.equal(detail.map[2], bottomThe);
  assert.equal(detail.map[3], bottomThe + 1);
});

test("alignDetail ignores case and punctuation in the rendered text", () => {
  const tokens = wordTokens("**Hello**, World!");
  const words = [
    { norm: "hello", start_ms: 0, end_ms: 1 },
    { norm: "world", start_ms: 1, end_ms: 2 }
  ];
  const detail = alignDetail(words, tokens, 0);
  assert.equal(detail.ok, true);
  assert.deepEqual(detail.map, [0, 1]);
});

test("alignDetail resyncs across an excerpt gap", () => {
  const filler = Array.from({ length: 40 }, (_, i) => "filler" + i).join(" ");
  const tokens = wordTokens("alpha beta gamma " + filler + " delta epsilon zeta eta theta");
  const first = [
    { norm: "alpha", start_ms: 0, end_ms: 1 },
    { norm: "beta", start_ms: 1, end_ms: 2 },
    { norm: "gamma", start_ms: 2, end_ms: 3 }
  ];
  const a = alignDetail(first, tokens, 0);
  assert.equal(a.ok, true);
  assert.deepEqual(a.map, [0, 1, 2]);
  const second = ["delta", "epsilon", "zeta", "eta", "theta"].map((norm, i) => ({
    norm,
    start_ms: i,
    end_ms: i + 1
  }));
  const b = alignDetail(second, tokens, a.cursor);
  assert.equal(b.ok, true);
  assert.deepEqual(b.map, [43, 44, 45, 46, 47]);
});

test("alignDetail does not anchor on a common word without a following run", () => {
  const filler = Array.from({ length: 30 }, (_, i) => "x" + i).join(" ");
  const tokens = wordTokens("the " + filler + " the cat sat");
  const words = ["the", "cat", "sat"].map((norm, i) => ({ norm, start_ms: i, end_ms: i + 1 }));
  const detail = alignDetail(words, tokens, 0);
  assert.equal(detail.ok, true);
  assert.deepEqual(detail.map, [31, 32, 33]);
});

test("alignDetail rejects text that is not in the reply", () => {
  const tokens = wordTokens("Completely different sentence about cats and dogs.");
  const words = ["red", "green", "blue", "yellow"].map((norm, i) => ({
    norm,
    start_ms: i,
    end_ms: i + 1
  }));
  assert.equal(alignDetail(words, tokens, 0).ok, false);
});

test("alignDetail rejects text when under half the words map", () => {
  const tokens = wordTokens("one two");
  const words = ["one", "x", "y", "z"].map((norm, i) => ({ norm, start_ms: i, end_ms: i + 1 }));
  assert.equal(alignDetail(words, tokens, 0).ok, false);
});

const SAVED = {
  autoRead: false,
  excerpt: true,
  excerptMode: "both",
  excerptTop: 2,
  excerptBottom: 2,
  captions: true
};

test("pendingEdits keeps only draft fields that differ from saved", () => {
  assert.deepEqual(pendingEdits(SAVED, { excerpt: false, captions: true }), { excerpt: false });
});
test("pendingEdits drops a draft edit that matches saved again", () => {
  assert.deepEqual(pendingEdits(SAVED, { autoRead: false }), {});
});
test("pendingEdits keeps an unsaved edit when a palette toggle changes another field", () => {
  const draft = { excerpt: false };
  const afterPalette = { ...SAVED, autoRead: true };
  assert.deepEqual(pendingEdits(afterPalette, draft), { excerpt: false });
});
test("pendingEdits drops a draft edit that an outside change made equal", () => {
  assert.deepEqual(pendingEdits({ ...SAVED, autoRead: true }, { autoRead: true }), {});
});
test("draftView shows unsaved edits and saved values for the rest", () => {
  const view = draftView({ ...SAVED, autoRead: true }, { excerpt: false });
  assert.equal(view.excerpt, false);
  assert.equal(view.autoRead, true);
  assert.equal(view.excerptTop, 2);
});

test("regression: edit then revert leaves no draft", () => {
  let draft = applyEdit(SAVED, {}, { autoRead: true });
  draft = applyEdit(SAVED, draft, { autoRead: false });
  assert.deepEqual(draft, {});
});
test("regression: reverted edit does not return after a palette toggle", () => {
  let draft = applyEdit(SAVED, {}, { autoRead: true });
  draft = applyEdit(SAVED, draft, { autoRead: false });
  assert.deepEqual(pendingEdits({ ...SAVED, autoRead: true }, draft), {});
});
test("regression: edit made equal by a saved change is pruned and never returns", () => {
  let draft = applyEdit(SAVED, {}, { autoRead: true });
  draft = pendingEdits({ ...SAVED, autoRead: true }, draft);
  assert.deepEqual(draft, {});
  assert.deepEqual(pendingEdits({ ...SAVED, autoRead: false }, draft), {});
});
test("regression: unsaved edit survives a palette toggle of another field", () => {
  const draft = applyEdit(SAVED, {}, { excerpt: false });
  assert.deepEqual(pendingEdits({ ...SAVED, autoRead: true }, draft), { excerpt: false });
});
test("regression: Save compares with stored settings, so another window's value is not overwritten by an untouched field", () => {
  const stored = { ...SAVED, autoRead: true };
  assert.deepEqual(pendingEdits(stored, { excerpt: false }), { excerpt: false });
});
test("regression: Save writes an edit whose value differs from stored settings", () => {
  const stored = { ...SAVED, excerpt: false };
  assert.deepEqual(pendingEdits(stored, { excerpt: true }), { excerpt: true });
});
test("regression: Save skips an edit already equal to stored settings", () => {
  const stored = { ...SAVED, autoRead: true };
  assert.deepEqual(pendingEdits(stored, { autoRead: true }), {});
});

// Glide pill. The harness fakes rAF, the element and the word rects: a test moves a
// word by changing its rect, the way a scroll or a re-layout does.
function box(left, top, width, height) {
  return { left, top, width, height, right: left + width, bottom: top + height };
}
function wordAt(left, top, width, height) {
  return { startContainer: { isConnected: true }, rect: box(left, top, width, height) };
}
function pillHarness(reduced = false) {
  let nextId = 1;
  let frames = [];
  let motion = reduced;
  const attached = new Set();
  const listeners = new Set();
  const env = {
    make() {
      return {
        style: {},
        setAttribute() {},
        remove() {
          attached.delete(this);
        }
      };
    },
    attach(el) {
      attached.add(el);
    },
    measure(range) {
      return range.rect === null ? null : { ...range.rect };
    },
    clip() {
      return UNBOUNDED;
    },
    reduced() {
      return motion;
    },
    raf(fn) {
      const id = nextId;
      nextId += 1;
      frames.push({ id, fn });
      return id;
    },
    cancelRaf(id) {
      frames = frames.filter((frame) => frame.id !== id);
    },
    listen(type, fn) {
      const entry = { type, fn };
      listeners.add(entry);
      return () => {
        listeners.delete(entry);
      };
    }
  };
  return {
    env,
    attached,
    listeners,
    pending: () => frames.length,
    step(ts) {
      const due = frames;
      frames = [];
      for (const frame of due) frame.fn(ts);
    }
  };
}
function finishGlide(harness, start) {
  let at = start;
  for (let guard = 0; harness.pending() > 0; guard += 1) {
    assert.ok(guard < 100, "glide must end");
    harness.step(at);
    at += 16;
  }
  return at;
}
const leftOf = (pill) => parseFloat(pill.element().style.left);

test("glide pill: first word lands on its rect with no CSS transition", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  pill.move(wordAt(10, 20, 40, 16));
  const style = pill.element().style;
  assert.equal(style.left, "10px");
  assert.equal(style.top, "20px");
  assert.equal(style.width, "40px");
  assert.equal(style.height, "16px");
  assert.equal(style.mixBlendMode, "difference");
  assert.equal(style.transition, undefined);
  assert.equal(h.pending(), 0);
  assert.equal(h.attached.size, 1);
});

test("glide pill: a moved word glides over several frames and lands exactly", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(wordAt(100, 0, 60, 20));
  const lefts = [];
  let at = 1000;
  while (h.pending() > 0) {
    assert.equal(h.pending(), 1, "one frame request at a time");
    h.step(at);
    lefts.push(leftOf(pill));
    at += 16;
    assert.ok(lefts.length < 100, "glide must end");
  }
  assert.ok(lefts.length >= 3, "glide spans several frames");
  assert.equal(lefts[0], 0);
  for (let i = 1; i < lefts.length; i += 1)
    assert.ok(lefts[i] >= lefts[i - 1], "left never moves back");
  assert.equal(lefts[lefts.length - 1], 100);
  assert.equal(pill.element().style.width, "60px");
  assert.equal(pill.moving(), false);
});

test("glide pill: a new word mid-glide starts from the drawn place", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(wordAt(100, 0, 60, 20));
  h.step(1000);
  h.step(1016);
  const drawn = leftOf(pill);
  assert.ok(drawn > 0 && drawn < 100, "pill is mid-glide");
  pill.move(wordAt(300, 0, 60, 20));
  assert.equal(h.pending(), 1, "retarget keeps a single frame request");
  h.step(1032);
  assert.equal(leftOf(pill), drawn, "first frame of the retarget is the drawn place");
  finishGlide(h, 1048);
  assert.equal(leftOf(pill), 300);
});

test("glide pill: a scroll during a glide lands on the moved word", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  const word = wordAt(100, 0, 60, 20);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(word);
  h.step(1000);
  h.step(1016);
  word.rect.left = 130;
  pill.reposition();
  assert.equal(h.pending(), 1, "reposition leaves the running loop to read the live rect");
  finishGlide(h, 1032);
  assert.equal(leftOf(pill), 130);
});

test("glide pill: a scroll while idle repaints at the word's new place", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  const word = wordAt(100, 0, 60, 20);
  pill.move(word);
  word.rect.top = 50;
  pill.reposition();
  assert.equal(pill.element().style.top, "50px");
  assert.equal(h.pending(), 0);
});

test("glide pill: a finished glide schedules no further frames", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(wordAt(100, 0, 60, 20));
  finishGlide(h, 1000);
  for (let i = 0; i < 5; i += 1) h.step(2000 + i * 16);
  assert.equal(leftOf(pill), 100);
  assert.equal(h.pending(), 0, "the loop stops when the glide ends");
});

test("glide pill: destroy mid-glide cancels the frame and removes element and listeners", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(wordAt(100, 0, 60, 20));
  h.step(1000);
  assert.equal(h.listeners.size, 2, "scroll and resize listeners");
  pill.destroy();
  assert.equal(h.pending(), 0);
  assert.equal(h.attached.size, 0);
  assert.equal(h.listeners.size, 0);
  assert.equal(pill.element(), null);
  assert.equal(pill.moving(), false);
});

test("glide pill: reduced motion moves the pill in one step", () => {
  const h = pillHarness(true);
  const pill = createGlidePill(h.env);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(wordAt(100, 0, 60, 20));
  assert.equal(h.pending(), 0);
  assert.equal(leftOf(pill), 100);
});

test("glide pill: a word scrolled out of its port hides the pill and stops the loop", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  const word = wordAt(100, 0, 60, 20);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(word);
  h.step(1000);
  word.rect = null;
  h.step(1016);
  assert.equal(pill.element().style.display, "none");
  assert.equal(h.pending(), 0);
  word.rect = box(100, 0, 60, 20);
  pill.reposition();
  assert.equal(pill.element().style.display, "block");
  assert.equal(leftOf(pill), 100);
});

test("glide pill: a word removed from the page destroys the pill mid-glide", () => {
  const h = pillHarness();
  const pill = createGlidePill(h.env);
  const word = wordAt(100, 0, 60, 20);
  pill.move(wordAt(0, 0, 50, 20));
  pill.move(word);
  h.step(1000);
  word.startContainer.isConnected = false;
  h.step(1016);
  assert.equal(h.attached.size, 0);
  assert.equal(h.listeners.size, 0);
  assert.equal(h.pending(), 0);
  assert.equal(pill.element(), null);
});

// measureWord: the padded rect, cut to every clipping ancestor and the viewport.
const plainStyle = () => ({ overflowX: "visible", overflowY: "visible", display: "block" });
const scrollStyle = (el) => ({ overflowX: el.overflow, overflowY: el.overflow, display: "block" });
const VIEW = box(0, 0, 1000, 1000);
// A clipping ancestor. In this fake the padding box equals the border box.
function clipper(overflow, rect, parent = null) {
  return {
    parentElement: parent,
    overflow: overflow,
    getBoundingClientRect: () => rect,
    clientLeft: 0,
    clientTop: 0,
    clientWidth: rect.width,
    clientHeight: rect.height
  };
}

test("measureWord pads the first painted rect", () => {
  const range = {
    startContainer: { parentElement: null },
    getClientRects: () => [box(10, 10, 40, 16)]
  };
  assert.deepEqual(measureWord(range, plainStyle, VIEW), {
    left: 6,
    top: 8,
    width: 48,
    height: 20
  });
});

test("measureWord cuts the word to its scroll port", () => {
  const port = clipper("auto", box(0, 12, 500, 300));
  const range = {
    startContainer: { parentElement: port },
    getClientRects: () => [box(10, 10, 40, 16)]
  };
  assert.deepEqual(measureWord(range, scrollStyle, VIEW), {
    left: 6,
    top: 12,
    width: 48,
    height: 16
  });
});

test("measureWord skips ancestors that do not clip", () => {
  const port = clipper("scroll", box(0, 0, 500, 9));
  const plain = { parentElement: port, overflow: "visible" };
  const range = {
    startContainer: { parentElement: plain },
    getClientRects: () => [box(10, 10, 40, 16)]
  };
  assert.deepEqual(measureWord(range, scrollStyle, VIEW), {
    left: 6,
    top: 8,
    width: 48,
    height: 1
  });
});

test("measureWord returns null when the word is scrolled out of its port", () => {
  const port = clipper("auto", box(0, 40, 500, 300));
  const range = {
    startContainer: { parentElement: port },
    getClientRects: () => [box(10, 10, 40, 16)]
  };
  assert.equal(measureWord(range, scrollStyle, VIEW), null);
});

test("measureWord cuts a word at the viewport edge", () => {
  const range = {
    startContainer: { parentElement: null },
    getClientRects: () => [box(10, -10, 40, 16)]
  };
  assert.deepEqual(measureWord(range, plainStyle, VIEW), { left: 6, top: 0, width: 48, height: 8 });
});

test("measureWord returns null when the word has no painted rect", () => {
  const range = { startContainer: { parentElement: null }, getClientRects: () => [] };
  assert.equal(measureWord(range, plainStyle, VIEW), null);
});

// pollPlan: what a caption poll does when settings or focus change.
const runOf = { sessionId: "s1", mode: "plain" };

test("pollPlan stops when captions are off", () => {
  assert.equal(pollPlan(runOf, { captions: false, glide: false }, "s1"), "stop");
});
test("pollPlan stops when focus moves to another session", () => {
  assert.equal(pollPlan(runOf, { captions: true, glide: false }, "s2"), "stop");
});
test("pollPlan switches paint mode when the glide setting flips", () => {
  assert.equal(pollPlan(runOf, { captions: true, glide: true }, "s1"), "switch");
});
test("pollPlan keeps the run when nothing changed", () => {
  assert.equal(pollPlan(runOf, { captions: true, glide: false }, "s1"), "keep");
});
test("pollPlan keeps a follow-only run with captions off", () => {
  const run = { sessionId: "s1", mode: "none" };
  assert.equal(pollPlan(run, { captions: false, glide: false, follow: true }, "s1"), "keep");
});
test("pollPlan stops when captions and follow are both off", () => {
  assert.equal(pollPlan(runOf, { captions: false, glide: false, follow: false }, "s1"), "stop");
});
test("pollPlan switches a follow-only run when captions turn on", () => {
  const run = { sessionId: "s1", mode: "none" };
  assert.equal(pollPlan(run, { captions: true, glide: false, follow: true }, "s1"), "switch");
});
test("pollPlan keeps a controlled run alive with every paint switch off", () => {
  const run = { sessionId: "s1", mode: "none", controlled: true };
  assert.equal(pollPlan(run, { captions: false, glide: false, follow: false }, "s1"), "keep");
});
test("pollPlan keeps a selection read painting with captions off", () => {
  const run = { sessionId: "s1", mode: "plain", forceCaptions: true };
  assert.equal(pollPlan(run, { captions: false, glide: false, follow: false }, "s1"), "keep");
});
test("pollPlan switches a selection read to the pill when glide turns on", () => {
  const run = { sessionId: "s1", mode: "plain", forceCaptions: true };
  assert.equal(pollPlan(run, { captions: true, glide: true, follow: false }, "s1"), "switch");
});

// Production paint and status wiring. A small fake browser stands in for the DOM: text laid out at
// 8 px per character, 16 px tall, at its parent's top-left plus `x`. Tests move boxes to simulate
// a scroll or a re-layout, the way the glide harness above does.
const UNBOUNDED = { left: -Infinity, top: -Infinity, right: Infinity, bottom: Infinity };
const CHAR_W = 8;
const CHAR_H = 16;
const REPLY = "Hello there friend";
const REPLY_RECT = { left: 100, top: 100, width: 400, height: 20 };
const REPLY_WORDS = wordEntries(REPLY, [
  [0, 5, 0, 400],
  [6, 11, 450, 900],
  [12, 18, 950, 1400]
]);
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
const bodyAt = (position) => ({
  active: true,
  position_ms: position,
  fragment_index: 0,
  captions: captionOf(REPLY, REPLY_WORDS)
});
const pillStyle = () => livePill().element().style;

function fakeEl(rect) {
  return {
    rect: rect,
    parentElement: null,
    overflowX: "visible",
    overflowY: "visible",
    display: "block",
    clientLeft: 0,
    clientTop: 0,
    clientWidth: rect.width,
    clientHeight: rect.height,
    style: {},
    isConnected: true,
    texts: [],
    getBoundingClientRect() {
      const r = this.rect;
      return {
        left: r.left,
        top: r.top,
        width: r.width,
        height: r.height,
        right: r.left + r.width,
        bottom: r.top + r.height
      };
    },
    appendChild(child) {
      child.parentElement = this;
      return child;
    },
    closest() {
      return null;
    },
    handlers: [],
    scrollTop: 0,
    contains(node) {
      for (let el = node.parentElement; el; el = el.parentElement) {
        if (el === this) return true;
      }
      return false;
    },
    addEventListener(type, fn, options) {
      this.handlers.push({ type: type, fn: fn, capture: options === true });
    },
    removeEventListener(type, fn) {
      this.handlers = this.handlers.filter((h) => h.type !== type || h.fn !== fn);
    },
    dispatch(type, event) {
      for (const h of this.handlers.filter((x) => x.type === type)) h.fn(event);
    },
    setAttribute() {},
    remove() {
      this.isConnected = false;
    }
  };
}

function fakeRange() {
  return {
    startContainer: null,
    start: 0,
    end: 0,
    setStart(node, offset) {
      this.startContainer = node;
      this.start = offset;
    },
    setEnd(node, offset) {
      this.end = offset;
    },
    getClientRects() {
      const node = this.startContainer;
      const origin = node.parentElement.rect;
      const left = origin.left + node.x + this.start * CHAR_W;
      return [
        {
          left: left,
          top: origin.top + (node.y ?? 0),
          width: (this.end - this.start) * CHAR_W,
          height: CHAR_H
        }
      ];
    }
  };
}

function rangeOver(node, start, end) {
  const range = fakeRange();
  range.setStart(node, start);
  range.setEnd(node, end);
  return range;
}

let installedWorld = false;
// Installs fake window, document, host and fetch, then stops the last test's run.
function freshWorld() {
  if (installedWorld) stopCaptions(); // while the last test's fakes still exist
  installedWorld = true;
  const html = fakeEl({ left: 0, top: 0, width: 1000, height: 800 });
  const body = html.appendChild(fakeEl({ left: 0, top: 0, width: 1000, height: 800 }));
  const replies = [];
  const world = {
    body: body,
    replies: replies,
    highlights: new Map(),
    frames: [],
    listeners: [],
    rafCount: 0,
    calls: [],
    timers: [],
    focus: { current: "s1" },
    settings: { captions: true, glide: false, follow: false },
    fetchUrls: []
  };
  let nextId = 1;
  globalThis.window = {
    CSS: { highlights: world.highlights },
    Highlight: class Highlight {
      constructor(range) {
        this.range = range;
      }
    },
    getComputedStyle: (el) => ({
      overflowX: el.overflowX,
      overflowY: el.overflowY,
      display: el.display
    }),
    matchMedia: () => ({ matches: false }),
    requestAnimationFrame(fn) {
      world.rafCount += 1;
      const id = nextId;
      nextId += 1;
      world.frames.push({ id: id, fn: fn });
      return id;
    },
    cancelAnimationFrame(id) {
      world.frames = world.frames.filter((frame) => frame.id !== id);
    },
    addEventListener(type, fn) {
      world.listeners.push({ type: type, fn: fn });
    },
    removeEventListener(type, fn) {
      world.listeners = world.listeners.filter((l) => l.type !== type || l.fn !== fn);
    }
  };
  const root = { querySelectorAll: () => replies };
  globalThis.document = {
    documentElement: html,
    head: fakeEl({ left: 0, top: 0, width: 0, height: 0 }),
    getElementById: () => null,
    createElement: () => fakeEl({ left: 0, top: 0, width: 0, height: 0 }),
    createRange: fakeRange,
    createTreeWalker(scope) {
      const queue = scope.texts.slice();
      return { nextNode: () => (queue.length > 0 ? queue.shift() : null) };
    },
    querySelectorAll: (selector) => (selector.includes("assistant-message-root") ? [root] : [])
  };
  globalThis.NodeFilter = { SHOW_TEXT: 4 };
  globalThis.host = {
    state: { focusedSessionId: { get: () => world.focus.current } },
    notify() {}
  };
  // Speech requests resolve at once. Each status request waits until the test answers it.
  globalThis.fetch = (url) => {
    world.fetchUrls.push(fetchUrlText(url));
    if (fetchUrlText(url) === SPEAK_URL) return Promise.resolve({ ok: true });
    return new Promise((resolve) => {
      world.calls.push({
        resolve: (answer) => resolve({ ok: true, json: () => Promise.resolve(answer) })
      });
    });
  };
  world.ctx = {
    setInterval(fn) {
      const timer = { fn: fn, disposed: false };
      world.timers.push(timer);
      return () => {
        timer.disposed = true;
      };
    }
  };
  world.addReply = (rect, text) => {
    const md = body.appendChild(fakeEl({ ...rect }));
    md.texts = [{ nodeValue: text, parentElement: md, isConnected: true, x: 0 }];
    replies.push(md);
    return md;
  };
  world.startRun = (sessionId = world.focus.current) => {
    startCaptions(world.ctx, () => world.settings, sessionId);
    return liveCaptionRun();
  };
  // Fires the newest interval, or the one at `index`. A disposed timer still fires, to test the guard.
  world.poll = (index = world.timers.length - 1) => world.timers[index].fn();
  // One poll, then answer its request with `answer`.
  world.tick = async (answer) => {
    const before = world.calls.length;
    world.poll();
    assert.equal(world.calls.length, before + 1, "one status request per poll");
    world.calls[before].resolve(answer);
    await flush();
  };
  world.stepOnce = (ts) => {
    const due = world.frames;
    world.frames = [];
    for (const frame of due) frame.fn(ts);
  };
  world.stepFrames = (ts) => {
    let at = ts;
    for (let guard = 0; world.frames.length > 0; guard += 1) {
      assert.ok(guard < 200, "glide must end");
      world.stepOnce(at);
      at += 16;
    }
    return at;
  };
  world.dispatch = (type) => {
    for (const listener of world.listeners.filter((l) => l.type === type)) {
      listener.fn({ type: type });
    }
  };
  return world;
}

test("captions paint in the focused chat when another chat is last in the document", async () => {
  const world = freshWorld();
  const focusedReply = world.addReply(REPLY_RECT, REPLY);
  const otherReply = world.addReply(REPLY_RECT, "Unrelated previous assistant response");
  const focusedRoot = { querySelectorAll: () => [focusedReply] };
  const otherRoot = { querySelectorAll: () => [otherReply] };
  const focusedPane = {
    querySelectorAll: (selector) =>
      selector.includes("assistant-message-root") ? [focusedRoot] : []
  };
  const otherPane = {
    querySelectorAll: (selector) => (selector.includes("assistant-message-root") ? [otherRoot] : [])
  };
  focusedPane.dataset = { sessionAnchor: "session-tile:focused" };
  otherPane.dataset = { sessionAnchor: "workspace" };
  globalThis.host.state.focusedStoredSessionId = { get: () => "focused" };
  globalThis.host.state.activeSessionId = { get: () => "other-session" };
  globalThis.document.querySelectorAll = (selector) => {
    if (selector.includes("assistant-message-root")) return [focusedRoot, otherRoot];
    if (selector.includes("data-session-anchor")) return [focusedPane, otherPane];
    return [];
  };
  globalThis.document.querySelector = (selector) => {
    if (selector.includes("session-tile:focused")) return focusedPane;
    if (selector.includes("workspace")) return otherPane;
    return null;
  };
  world.startRun();
  await world.tick(bodyAt(100));
  assert.equal(
    world.highlights.get("copyspeech-word")?.range.startContainer,
    focusedReply.texts[0],
    "the word is painted in the focused pane, not the document's last pane"
  );
});

test("a caption miss is retried on the next poll, so a reply rendered later still paints", async () => {
  const world = freshWorld();
  const reply = world.addReply(REPLY_RECT, "Unrelated previous assistant response");
  world.startRun();
  await world.tick(bodyAt(100));
  assert.equal(world.highlights.size, 0, "the first render does not align");
  reply.texts = [{ nodeValue: REPLY, parentElement: reply, isConnected: true, x: 0 }];
  await world.tick(bodyAt(100));
  assert.equal(
    world.highlights.get("copyspeech-word")?.range.startContainer,
    reply.texts[0],
    "the next poll aligns the reply that now renders"
  );
});

test("glide: repeated status ticks on one word do not restart the glide", async () => {
  const world = freshWorld();
  world.settings.glide = true;
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  await world.tick(bodyAt(100));
  assert.equal(pillStyle().left, "96px", "first word lands on its rect");
  assert.equal(world.rafCount, 0, "first word does not glide");
  await world.tick(bodyAt(150));
  await world.tick(bodyAt(300));
  assert.equal(world.rafCount, 0, "more ticks on the same word start no glide");
  await world.tick(bodyAt(500));
  assert.equal(world.rafCount, 1, "a new word starts one glide");
  await world.tick(bodyAt(600));
  assert.equal(world.rafCount, 1, "ticks mid-glide on the same word start no glide");
});

test("glide: a held gap keeps the last word painted and stops nothing", async () => {
  const world = freshWorld();
  world.settings.glide = true;
  world.addReply(REPLY_RECT, REPLY);
  const run = world.startRun();
  await world.tick(bodyAt(100));
  await world.tick(bodyAt(420));
  assert.equal(pillStyle().left, "96px", "the pill holds its word in the gap");
  assert.equal(liveCaptionRun(), run, "a gap does not stop the run");
  assert.equal(world.rafCount, 0, "a gap starts no glide");
  await world.tick(bodyAt(500));
  assert.equal(world.rafCount, 1, "the next word glides from the held place");
});

test("stale response after a focus change does not paint", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  world.poll();
  world.focus.current = "s2";
  world.calls[0].resolve(bodyAt(100));
  await flush();
  assert.equal(world.highlights.size, 0, "no highlight from the old chat");
  assert.equal(livePill(), null, "no pill from the old chat");
  assert.equal(liveCaptionRun(), null, "the run stops");
});

test("stale response after captions turn off does not paint", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  world.poll();
  world.settings.captions = false;
  world.calls[0].resolve(bodyAt(100));
  await flush();
  assert.equal(world.highlights.size, 0);
  assert.equal(livePill(), null);
  assert.equal(liveCaptionRun(), null);
});

test("a focus change during a pending request stops the run without a new request", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  world.poll();
  world.focus.current = "s2";
  world.poll();
  assert.equal(world.calls.length, 1, "no second request while the first is pending");
  assert.equal(liveCaptionRun(), null, "the poll saw the focus change despite the pending request");
  world.calls[0].resolve(bodyAt(100));
  await flush();
  assert.equal(world.highlights.size, 0, "the late answer paints nothing");
});

test("a stale response from a replaced run does not stop the replacement", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun("s1");
  world.poll();
  world.focus.current = "s2";
  const replacement = world.startRun("s2");
  world.calls[0].resolve(bodyAt(100));
  await flush();
  assert.equal(
    liveCaptionRun(),
    replacement,
    "the old run's late answer leaves the replacement alive"
  );
  assert.equal(world.highlights.size, 0);
  const sent = world.calls.length;
  world.poll(0);
  assert.equal(world.calls.length, sent, "the disposed old run sends no request");
});

test("a glide change during a pending request paints in the new mode", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  const run = world.startRun();
  world.poll();
  world.settings.glide = true;
  world.calls[0].resolve(bodyAt(100));
  await flush();
  assert.equal(run.mode, "glide", "the run switched before it painted");
  assert.equal(world.highlights.size, 0, "no CSS highlight in glide mode");
  assert.equal(pillStyle().left, "96px", "the pill paints the word");
});

test("switch mode: plain to glide and back repaints the word in each mode", async () => {
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  await world.tick(bodyAt(100));
  assert.equal(
    world.highlights.has("copyspeech-word"),
    true,
    "plain mode paints the CSS highlight"
  );
  world.settings.glide = true;
  world.poll();
  assert.equal(world.highlights.size, 0, "switching to glide clears the highlight");
  assert.equal(pillStyle().left, "96px", "switching to glide shows the pill on the same word");
  world.calls[world.calls.length - 1].resolve(bodyAt(500));
  await flush();
  assert.equal(world.rafCount, 1, "the next word glides in glide mode");
  world.settings.glide = false;
  await world.tick(bodyAt(500));
  assert.equal(livePill(), null, "switching back destroys the pill");
  assert.equal(
    world.highlights.has("copyspeech-word"),
    true,
    "plain mode paints the highlight again"
  );
});

test("glide: a scroll mid-glide does not jump the pill, and the glide lands on the moved word", async () => {
  const world = freshWorld();
  world.settings.glide = true;
  const reply = world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  await world.tick(bodyAt(100));
  await world.tick(bodyAt(500));
  world.stepOnce(1000);
  world.stepOnce(1016);
  const midGlide = pillStyle().left;
  assert.ok(parseFloat(midGlide) > 96 && parseFloat(midGlide) < 144, "the pill is mid-glide");
  reply.rect.left -= 30; // the chat scrolls left under the word
  world.dispatch("scroll");
  assert.equal(pillStyle().left, midGlide, "a scroll mid-glide leaves the pill where it is");
  world.stepFrames(1032);
  assert.equal(pillStyle().left, "114px", "the glide lands on the word's new place");
});

test("clip: a word is cut by a hidden ancestor above a visible one, and by a nearer auto box", async () => {
  const world = freshWorld();
  const outer = world.body.appendChild(fakeEl({ left: 100, top: 100, width: 200, height: 100 }));
  outer.overflowX = "hidden";
  const middle = outer.appendChild(fakeEl({ left: 100, top: 100, width: 400, height: 100 }));
  const inner = middle.appendChild(fakeEl({ left: 100, top: 100, width: 400, height: 100 }));
  inner.overflowX = "auto";
  const para = inner.appendChild(fakeEl({ left: 280, top: 100, width: 40, height: 20 }));
  para.texts = [{ nodeValue: "abcde", parentElement: para, isConnected: true, x: 0 }];
  // The word spans 280..320 and pads to 276..324. The hidden outer box ends at 300.
  const rect = livePillEnv().measure(rangeOver(para.texts[0], 0, 5));
  assert.deepEqual(rect, { left: 276, top: 100, width: 24, height: 18 });
});

test("clip: every drawn glide frame stays inside the target word's clip", async () => {
  const world = freshWorld();
  const wide = world.body.appendChild(fakeEl({ left: 100, top: 100, width: 300, height: 100 }));
  wide.overflowX = "hidden";
  const paraA = wide.appendChild(fakeEl({ left: 100, top: 100, width: 300, height: 20 }));
  paraA.texts = [{ nodeValue: "wordA", parentElement: paraA, isConnected: true, x: 280 }];
  const narrow = world.body.appendChild(fakeEl({ left: 100, top: 300, width: 100, height: 100 }));
  narrow.overflowX = "hidden";
  const paraB = narrow.appendChild(fakeEl({ left: 100, top: 300, width: 100, height: 20 }));
  paraB.texts = [{ nodeValue: "wordB", parentElement: paraB, isConnected: true, x: 80 }];
  // Word A clips at x 400, word B at x 200. The glide starts in A's clip and must not paint past 200.
  const pill = createGlidePill(livePillEnv());
  pill.move(rangeOver(paraA.texts[0], 0, 5));
  pill.move(rangeOver(paraB.texts[0], 0, 5));
  const style = pill.element().style;
  let steps = 0;
  while (world.frames.length > 0) {
    world.stepOnce(1000 + steps * 16);
    steps += 1;
    assert.ok(steps < 200, "glide must end");
    if (style.display !== "none") {
      const left = parseFloat(style.left);
      const right = left + parseFloat(style.width);
      assert.ok(
        left >= 100 && right <= 200,
        "frame " + steps + " stays in the clip: " + left + ".." + right
      );
    }
  }
  assert.ok(steps >= 3, "the glide drew several frames");
  assert.equal(style.left, "176px");
  assert.equal(style.width, "24px");
});

// Follow: captions on and off, manual input, and cleanup. A fake clock drives the pause and interval timers.
const SPEAK_URL = "http://127.0.0.1:43117/speak";
// fetch accepts a string, a URL, or a Request. Normalize to the absolute URL text.
function fetchUrlText(input) {
  return input instanceof Request ? input.url : new URL(input).href;
}
const clock = { now: 1000000 };
function useFakeClock() {
  clock.now = 1000000;
  Date.now = () => clock.now;
}
globalThis.PALETTE_AREA = "palette";
globalThis.atom = (initial) => {
  let value = initial;
  return {
    get: () => value,
    set: (next) => {
      value = next;
    }
  };
};
const LINE_ONE = "Alpha beta gamma";
const LINE_TWO = "Delta epsilon zeta";
const FOLLOW_TEXT = LINE_ONE + " " + LINE_TWO;
const FOLLOW_WORDS = wordEntries(FOLLOW_TEXT, [
  [0, 5, 0, 400],
  [6, 10, 450, 900],
  [11, 16, 950, 1400],
  [17, 22, 1500, 1900],
  [23, 30, 1950, 2400],
  [31, 35, 2450, 2900]
]);
const followBody = (position) => ({
  active: true,
  position_ms: position,
  fragment_index: 0,
  captions: captionOf(FOLLOW_TEXT, FOLLOW_WORDS)
});

// A reply in a scrolling box. Its second line starts below the comfort band (y 350 of 400).
function addScrolledReply(world) {
  const scroller = world.body.appendChild(fakeEl({ left: 0, top: 0, width: 1000, height: 400 }));
  scroller.overflowY = "auto";
  scroller.scrollHeight = 2000;
  scroller.scrollTop = 0;
  scroller.scrollLog = [];
  const md = scroller.appendChild(fakeEl({ left: 0, top: 0, width: 1000, height: 2000 }));
  md.texts = [
    { nodeValue: LINE_ONE, parentElement: md, isConnected: true, x: 0, y: 100 },
    { nodeValue: LINE_TWO, parentElement: md, isConnected: true, x: 0, y: 350 }
  ];
  world.replies.push(md);
  // A fake scroll moves the reply's layout, the way a real scroll moves its words.
  scroller.scrollTo = function (options) {
    const delta = options.top - this.scrollTop;
    this.scrollTop = options.top;
    md.rect = { ...md.rect, top: md.rect.top - delta };
    this.scrollLog.push(options);
  };
  return { scroller, md };
}

// Move the reply's layout, the way a re-render or an outside scroll would.
function relayout(md, top) {
  md.rect = { ...md.rect, top: top };
}

// Wire the plugin's own register(ctx) with a fake SDK context, so the message.complete path runs.
function wireRegister(world, saved) {
  const data = new Map(saved ? [["settings", saved]] : []);
  const storage = {
    get: (key, fallback) => (data.has(key) ? data.get(key) : fallback),
    set: (key, value) => {
      data.set(key, value);
    }
  };
  const handlers = {};
  const ctx = {
    storage: storage,
    onEvent: (type, fn) => {
      handlers[type] = fn;
    },
    onDispose: () => {},
    addEventListener: () => {},
    setInterval: world.ctx.setInterval,
    register: () => {},
    registerSettingsPage: () => {}
  };
  plugin.register(ctx);
  return handlers;
}

function announceReply(handlers, text) {
  handlers["message.start"]({ session_id: "s1" });
  handlers["message.complete"]({
    type: "message.complete",
    session_id: "s1",
    seq: 9,
    replayed: false,
    payload: { status: "complete", text: text }
  });
}

test("captions resume after an inactive synthesis stall", async () => {
  useFakeClock();
  const world = freshWorld();
  const reply = world.addReply(REPLY_RECT, REPLY);
  const run = world.startRun();
  await world.tick(bodyAt(100));
  clock.now += 3000;
  await world.tick({ ...bodyAt(100), active: false });
  assert.equal(liveCaptionRun(), run, "inactive status with captions keeps the run");
  await world.tick(bodyAt(500));
  assert.equal(world.highlights.get("copyspeech-word").range.start, 6);
  assert.equal(world.highlights.get("copyspeech-word").range.startContainer, reply.texts[0]);
});

test("an end status clears a matched caption run", async () => {
  useFakeClock();
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  world.startRun();
  await world.tick(bodyAt(100));
  // One inactive poll can be a stale report, so an end takes two in a row.
  await world.tick({ active: false });
  assert.notEqual(liveCaptionRun(), null, "one inactive poll keeps the run");
  await world.tick({ active: false });
  assert.equal(liveCaptionRun(), null);
  assert.equal(world.highlights.size, 0);
  assert.equal(world.timers[0].disposed, true);
});

test("an inactive status before the first match does not end the run", async () => {
  useFakeClock();
  const world = freshWorld();
  const run = world.startRun();
  await world.tick({ active: false });
  assert.equal(liveCaptionRun(), run);
  clock.now += 59999;
  await world.tick(null);
  assert.equal(liveCaptionRun(), run);
  clock.now += 1;
  await world.tick(null);
  assert.equal(liveCaptionRun(), null);
});

test("silence stops a matched caption run at 60 seconds", async () => {
  useFakeClock();
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  const run = world.startRun();
  await world.tick(bodyAt(100));
  clock.now += 59999;
  await world.tick({ ...bodyAt(100), active: false });
  assert.equal(liveCaptionRun(), run);
  clock.now += 1;
  await world.tick({ ...bodyAt(100), active: false });
  assert.equal(liveCaptionRun(), null);
  assert.equal(world.highlights.size, 0);
});

test("follow off: a word out of the band never scrolls and adds no scroller listeners", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = false;
  world.startRun();
  await world.tick(followBody(1500));
  assert.deepEqual(scroller.scrollLog, []);
  assert.equal(scroller.handlers.length, 0);
  assert.equal(world.highlights.size, 1, "captions still paint");
});

test("follow on: a word that leaves the band scrolls its line to the anchor, once", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(100));
  assert.equal(scroller.scrollLog.length, 0, "a word inside the band does not move the view");
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1);
  assert.equal(scroller.scrollLog[0].top, 214, "the line lands on the anchor");
  assert.equal(scroller.scrollLog[0].behavior, "smooth");
  assert.equal(
    world.highlights.get("copyspeech-word").range.startContainer,
    world.replies[0].texts[1]
  );
});

test("follow on: words on a line already in the band do not move the view again", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(1500));
  clock.now += 1000;
  await world.tick(followBody(1950));
  await world.tick(followBody(2450));
  assert.equal(scroller.scrollLog.length, 1, "later words on the moved line do not scroll");
});

test("follow on with captions off: scrolls, paints nothing, and runs one poll", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1);
  assert.equal(world.highlights.size, 0, "no highlight");
  assert.equal(livePill(), null, "no pill");
  assert.equal(liveCaptionRun().mode, "none");
  assert.equal(world.timers.length, 1);
});

test("both off: a finished reply starts no status poll", () => {
  useFakeClock();
  const world = freshWorld();
  const handlers = wireRegister(world, { autoRead: true, captions: false, follow: false });
  announceReply(handlers, FOLLOW_TEXT);
  assert.equal(world.timers.length, 0, "no poll loop");
  assert.equal(liveCaptionRun(), null);
  assert.deepEqual(world.fetchUrls, [SPEAK_URL], "only the speech request");
  assert.match(playbackRequestIdNow(), /^hermes\./, "the reading carries a request id");
  clearPlayback();
});

test("auto-read sends the request id and wait false", () => {
  useFakeClock();
  const world = freshWorld();
  const bodies = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = (url, options) => {
    if (fetchUrlText(url) === SPEAK_URL) {
      bodies.push(JSON.parse(options.body));
      return Promise.resolve({ ok: true });
    }
    return previousFetch(url, options);
  };
  const handlers = wireRegister(world, { autoRead: true, captions: false, follow: false });
  announceReply(handlers, REPLY);
  globalThis.fetch = previousFetch;
  assert.equal(bodies.length, 1);
  assert.match(bodies[0].request_id, /^hermes\./);
  assert.equal(bodies[0].wait, false);
  clearPlayback();
});

test("a finished auto read retires the playback record", async () => {
  useFakeClock();
  const world = freshWorld();
  world.addReply(REPLY_RECT, REPLY);
  const handlers = wireRegister(world, { autoRead: true, captions: true, follow: false });
  announceReply(handlers, REPLY);
  assert.match(playbackRequestIdNow(), /^hermes\./);
  await world.tick(bodyAt(100));
  assert.notEqual(liveCaptionRun(), null, "the run is live while the reading plays");
  await world.tick({ active: false });
  await world.tick({ active: false });
  assert.equal(liveCaptionRun(), null, "the ended status stops the run");
  assert.equal(playbackRequestIdNow(), null, "the bar retires with the reading");
  clearPlayback();
});

test("follow only: speech goes first, one poll starts, and the reply scrolls without paint", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  const handlers = wireRegister(world, { autoRead: true, captions: false, follow: true });
  announceReply(handlers, FOLLOW_TEXT);
  assert.equal(world.fetchUrls[0], SPEAK_URL, "the speech request goes out before any poll");
  assert.equal(world.timers.length, 1, "one poll for follow");
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1);
  assert.equal(world.highlights.size, 0);
});

test("turning follow off mid-run removes the scroller listeners and stops scrolling", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(100));
  assert.equal(scroller.handlers.length, 4, "wheel, touch, scroll keys and scrollbar press");
  world.settings.follow = false;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0);
  assert.equal(scroller.handlers.length, 0);
  world.settings.follow = true;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "turning follow back on scrolls again");
});

test("stopping the run removes the scroller listeners and disposes the poll", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(100));
  assert.equal(scroller.handlers.length, 4);
  stopCaptions();
  assert.equal(scroller.handlers.length, 0);
  assert.equal(world.timers[0].disposed, true);
  assert.equal(liveCaptionRun(), null);
});

test("a focus change stops the run and removes the scroller listeners", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(100));
  world.focus.current = "s2";
  world.poll();
  assert.equal(liveCaptionRun(), null);
  assert.equal(scroller.handlers.length, 0);
});

test("a wheel or touch on the reply pauses follow for five seconds", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  scroller.dispatch("wheel", {});
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0, "the view stays where the reader put it");
  clock.now += 4999;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0, "still paused before five seconds");
  clock.now += 1;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "follow resumes at five seconds");
});

test("a press on the scrollbar pauses follow, a press on a word does not", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  scroller.dispatch("pointerdown", { target: md });
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "a press on a word leaves follow on");
  relayout(md, 0);
  clock.now += 1000;
  scroller.dispatch("pointerdown", { target: scroller });
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "a press on the scrollbar pauses follow");
});

test("scroll keys pause follow, other keys do not", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  scroller.dispatch("keydown", { key: "a" });
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "a letter key leaves follow on");
  relayout(md, 0);
  clock.now += 1000;
  scroller.dispatch("keydown", { key: "ArrowDown" });
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "a scroll key pauses follow");
});

test("follow scrolls at most once per 400 ms", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(1500));
  relayout(md, 0);
  await world.tick(followBody(1950));
  assert.equal(scroller.scrollLog.length, 1, "a second move inside 400 ms waits");
  clock.now += 400;
  await world.tick(followBody(1950));
  assert.equal(scroller.scrollLog.length, 2, "the move follows once the interval passes");
});

test("a gap between words holds the view", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings.follow = true;
  relayout(md, 300);
  world.startRun();
  await world.tick(followBody(1450));
  assert.equal(scroller.scrollLog.length, 0, "the gap after gamma moves nothing");
  await world.tick(followBody(1000));
  assert.equal(scroller.scrollLog.length, 1, "gamma, out of the band, moves the view");
});

test("a status answer for a stopped run does not scroll", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  world.poll();
  stopCaptions();
  world.calls[0].resolve(followBody(1500));
  await flush();
  assert.equal(scroller.scrollLog.length, 0);
});

test("a late status answer from the old chat does not scroll the new chat", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  world.poll();
  world.focus.current = "s2";
  world.calls[0].resolve(followBody(1500));
  await flush();
  assert.equal(scroller.scrollLog.length, 0, "the answer lands after the focus change");
  assert.equal(liveCaptionRun(), null);
  assert.equal(scroller.handlers.length, 0);
});

test("reduced motion: the follow move is an instant jump", async () => {
  useFakeClock();
  const world = freshWorld();
  window.matchMedia = () => ({ matches: true });
  const { scroller } = addScrolledReply(world);
  world.settings.follow = true;
  world.startRun();
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog[0].behavior, "instant");
});

test("captions turned on mid-run paint the current word and keep following", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(1500));
  world.settings.captions = true;
  await world.tick(followBody(1950));
  assert.equal(
    world.highlights.get("copyspeech-word").range.startContainer,
    world.replies[0].texts[1]
  );
  assert.equal(scroller.scrollLog.length, 1, "the reply moved once");
  assert.equal(liveCaptionRun().mode, "plain");
});

test("a touch on a word pauses follow, unlike a mouse press on a word", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  scroller.dispatch("touchstart", { target: md });
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0, "a touch on a word pauses follow");
});

test("a text selection in the reply holds follow until it clears", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller, md } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  const selection = { isCollapsed: false, anchorNode: md, focusNode: md };
  window.getSelection = () => selection;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0, "a selection inside the reply holds the view");
  selection.isCollapsed = true;
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 1, "follow resumes once the selection clears");
});

test("without the Highlight API, plain captions start no run and glide still paints", async () => {
  useFakeClock();
  const world = freshWorld();
  window.CSS = undefined;
  world.addReply(REPLY_RECT, REPLY);
  world.settings = { captions: true, glide: false, follow: false };
  assert.equal(world.startRun(), null, "plain captions need the highlight API");
  world.settings.glide = true;
  world.startRun();
  await world.tick(bodyAt(100));
  assert.equal(pillStyle().display, "block", "the glide pill paints without the highlight API");
  assert.equal(pillStyle().left, "96px");
});

test("turning follow on mid-reading scrolls from the next status poll", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: true, glide: false, follow: false };
  world.startRun();
  await world.tick(followBody(1500));
  assert.equal(scroller.scrollLog.length, 0, "follow off: the line stays put");
  world.settings.follow = true;
  await world.tick(followBody(1950));
  assert.equal(scroller.scrollLog.length, 1, "follow on: the next poll scrolls the line");
});

test("follow off mid-run with captions off stops the run and removes the wheel listeners", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  assert.equal(scroller.handlers.length, 4);
  world.settings.follow = false;
  world.poll();
  assert.equal(liveCaptionRun(), null, "both toggles off stop the run");
  assert.equal(world.calls.length, 1, "stopping sends no status request");
  assert.equal(scroller.handlers.length, 0, "stopping removes the listeners");
});

test("follow turned on after the run stopped waits for the next reply", async () => {
  useFakeClock();
  const world = freshWorld();
  const { scroller } = addScrolledReply(world);
  world.settings = { captions: false, glide: false, follow: true };
  world.startRun();
  await world.tick(followBody(100));
  world.settings.follow = false;
  world.poll();
  world.settings.follow = true;
  world.poll();
  assert.equal(liveCaptionRun(), null, "nothing restarts the stopped run mid-reading");
  assert.equal(world.calls.length, 1, "no status request after the stop");
  assert.equal(scroller.scrollLog.length, 0, "the view does not move");
});

test("message button has a label on insertion and can switch to stop", () => {
  const dom = new JSDOM(
    '<div data-slot="aui_assistant-message-root" data-role="assistant"><div class="aui-message-actions-desktop"></div></div>'
  );
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  const root = document.querySelector('[data-role="assistant"]');
  const row = document.querySelector(".aui-message-actions-desktop");
  const button = mod.makeMessageButton(root);
  row.appendChild(button);
  assert.equal(button.getAttribute("aria-label"), "Read aloud with CopySpeech");
  mod.playbackRemember("message", "test-id", root, null);
  assert.equal(button.getAttribute("aria-label"), "Stop CopySpeech reading");
  assert.equal(button.getAttribute("aria-pressed"), "true");
  const stopPath = button.querySelector("path");
  assert.equal(stopPath.getAttribute("stroke"), "currentColor");
  assert.equal(stopPath.getAttribute("fill"), "none");
  mod.clearPlayback();
  assert.equal(button.getAttribute("aria-label"), "Read aloud with CopySpeech");
  dom.window.close();
});

test("message button shows a themed tooltip on hover and hides it on leave", async () => {
  const dom = new JSDOM(
    '<div data-slot="aui_assistant-message-root" data-role="assistant"><div class="aui-message-actions-desktop"></div></div>',
    { pretendToBeVisual: true }
  );
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  const root = document.querySelector('[data-role="assistant"]');
  const button = mod.makeMessageButton(root);
  document.querySelector(".aui-message-actions-desktop").appendChild(button);
  button.dispatchEvent(new dom.window.Event("pointerenter"));
  await new Promise((resolve) => dom.window.setTimeout(resolve, 250));
  const tooltip = document.querySelector('[role="tooltip"]');
  assert.ok(tooltip, "hover shows a tooltip");
  assert.equal(tooltip.textContent, "Read aloud with CopySpeech");
  mod.playbackRemember("message", "tooltip-test", root, null);
  assert.equal(tooltip.textContent, "Stop CopySpeech reading");
  button.dispatchEvent(new dom.window.Event("pointerleave"));
  assert.equal(document.querySelector('[role="tooltip"]'), null);
  mod.clearPlayback();
  dom.window.close();
});

test("selection can start at a reply paragraph boundary", () => {
  const dom = new JSDOM(
    '<div data-slot="aui_assistant-message-root" data-role="assistant"><div class="aui-md"><p>First</p><p>Second</p></div></div>'
  );
  const body = dom.window.document.querySelector(".aui-md");
  const range = dom.window.document.createRange();
  range.setStart(body, 0);
  range.setEnd(body.lastChild.firstChild, 6);
  assert.equal(mod.selectionInsideReply(range), true);
  dom.window.close();
});

test("clicking another reply replaces a manual reading still waiting for completion", async () => {
  const dom = new JSDOM(
    '<div data-slot="aui_assistant-message-root" data-role="assistant"><div class="aui-md">First reply</div><div class="aui-message-actions-desktop"></div></div>' +
      '<div data-slot="aui_assistant-message-root" data-role="assistant"><div class="aui-md">Second reply</div><div class="aui-message-actions-desktop"></div></div>',
    { url: "https://hermes.local", pretendToBeVisual: true }
  );
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  globalThis.NodeFilter = dom.window.NodeFilter;
  globalThis.host = { state: { focusedSessionId: { get: () => "s1" } }, notify() {} };
  const requests = [];
  globalThis.fetch = (url, options) => {
    requests.push({ url: fetchUrlText(url), body: JSON.parse(options.body) });
    if (fetchUrlText(url).endsWith("/speak")) return new Promise(() => {});
    return Promise.resolve({ ok: true });
  };
  const disposers = [];
  const ctx = {
    storage: { get: (_key, fallback) => fallback, set() {} },
    onEvent() {},
    onDispose(fn) {
      disposers.push(fn);
    },
    addEventListener() {},
    setInterval() {
      return () => {};
    },
    register() {},
    registerSettingsPage() {}
  };
  plugin.register(ctx);
  const buttons = document.querySelectorAll("[data-copyspeech-play]");
  assert.equal(buttons.length, 2);
  buttons[0].click();
  await flush();
  assert.equal(requests.length, 1);
  assert.equal(requests[0].body.text, "First reply");
  assert.equal(requests[0].body.wait, true);
  buttons[1].click();
  await flush();
  assert.equal(requests.length, 3, "replacement must stop the first request and start the second");
  assert.ok(requests[1].url.endsWith("/stop"));
  assert.equal(requests[1].body.request_id, requests[0].body.request_id);
  assert.equal(requests[2].body.text, "Second reply");
  for (const dispose of disposers) dispose();
  dom.window.close();
});

for (const { name, fn } of cases) {
  await fn();
  passed += 1;
  console.log("ok - " + name);
}

console.log(`\n${passed} tests passed`);
