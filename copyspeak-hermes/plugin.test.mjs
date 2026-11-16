// Run from this folder: node plugin.test.mjs
// plugin.js imports @hermes/plugin-sdk, which Node cannot resolve. The Hermes
// loader rejects relative imports, so this test strips the import lines into a
// temp copy and loads that copy.
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(join(here, "plugin.js"), "utf8");
const stripped = source.replace(/^import\s[\s\S]*?from\s+['"][^'"]+['"];?\s*$/gm, "");

const scratch = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "copyspeak-hermes-test-"));
const copy = join(scratch, "plugin-under-test.mjs");
writeFileSync(copy, stripped);
let mod;
try {
  mod = await import(pathToFileURL(copy).href);
} finally {
  rmSync(scratch, { recursive: true, force: true });
}

const {
  toSpeech,
  excerptSpeech,
  shouldSpeak,
  speakVerdict,
  withClaim,
  normalizeSettings,
  clampCount,
  wordTokens,
  captionWords,
  activeWordIndex,
  alignWords
} = mod;
let passed = 0;
function test(name, fn) {
  fn();
  passed += 1;
  console.log("ok - " + name);
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
    captions: true
  });
});
test("normalizeSettings keeps a saved captions value and rejects a non-boolean", () => {
  assert.equal(normalizeSettings({ captions: false }).captions, false);
  assert.equal(normalizeSettings({ captions: "yes" }).captions, true);
});

test("gate: happy path speaks", () => {
  assert.equal(shouldSpeak(happyEvent(), happyState()), true);
});
test("gate: replayed event is skipped", () => {
  assert.equal(shouldSpeak(happyEvent({ replayed: true }), happyState()), false);
});
test("gate: unarmed session is skipped", () => {
  assert.equal(shouldSpeak(happyEvent(), happyState({ armed: new Set() })), false);
});
test("gate: error status is skipped", () => {
  assert.equal(
    shouldSpeak(happyEvent({ payload: { status: "error", text: "boom" } }), happyState()),
    false
  );
});
test("gate: non-focused session is skipped", () => {
  assert.equal(shouldSpeak(happyEvent(), happyState({ focusedSessionId: "s2" })), false);
});
test("gate: duplicate claim is skipped", () => {
  const claims = { "claim.s1.5": 1 };
  assert.equal(shouldSpeak(happyEvent(), happyState({ claims })), false);
});
test("gate: built-in auto-read on is skipped", () => {
  const verdict = speakVerdict(happyEvent(), happyState({ builtInOn: true }));
  assert.equal(verdict.speak, false);
  assert.equal(verdict.reason, "Hermes built-in auto-read is on");
});
test("gate: autoRead off is skipped", () => {
  assert.equal(
    shouldSpeak(happyEvent(), happyState({ settings: { ...ON, autoRead: false } })),
    false
  );
});
test("gate: consumes the armed flag once", () => {
  const state = happyState();
  assert.equal(shouldSpeak(happyEvent(), state), true);
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

test("alignWords maps plain words in order", () => {
  const tokens = wordTokens("The quick brown fox.");
  const words = [
    { norm: "the", start_ms: 0, end_ms: 1 },
    { norm: "quick", start_ms: 1, end_ms: 2 },
    { norm: "brown", start_ms: 2, end_ms: 3 },
    { norm: "fox", start_ms: 3, end_ms: 4 }
  ];
  assert.deepEqual(alignWords(words, tokens, 0).map, [0, 1, 2, 3]);
});

test("alignWords ignores case and punctuation in the rendered text", () => {
  const tokens = wordTokens("**Hello**, World!");
  const words = [
    { norm: "hello", start_ms: 0, end_ms: 1 },
    { norm: "world", start_ms: 1, end_ms: 2 }
  ];
  assert.deepEqual(alignWords(words, tokens, 0).map, [0, 1]);
});

test("alignWords resyncs across an excerpt gap", () => {
  const filler = Array.from({ length: 40 }, (_, i) => "filler" + i).join(" ");
  const tokens = wordTokens("alpha beta gamma " + filler + " delta epsilon zeta eta theta");
  const first = [
    { norm: "alpha", start_ms: 0, end_ms: 1 },
    { norm: "beta", start_ms: 1, end_ms: 2 },
    { norm: "gamma", start_ms: 2, end_ms: 3 }
  ];
  const a = alignWords(first, tokens, 0);
  assert.deepEqual(a.map, [0, 1, 2]);
  const second = ["delta", "epsilon", "zeta", "eta", "theta"].map((norm, i) => ({
    norm,
    start_ms: i,
    end_ms: i + 1
  }));
  const b = alignWords(second, tokens, a.cursor);
  assert.deepEqual(b.map, [43, 44, 45, 46, 47]);
});

test("alignWords does not anchor on a common word without a following run", () => {
  const filler = Array.from({ length: 30 }, (_, i) => "x" + i).join(" ");
  const tokens = wordTokens("the " + filler + " the cat sat");
  const words = ["the", "cat", "sat"].map((norm, i) => ({ norm, start_ms: i, end_ms: i + 1 }));
  assert.deepEqual(alignWords(words, tokens, 0).map, [31, 32, 33]);
});

test("alignWords returns null for text that is not in the reply", () => {
  const tokens = wordTokens("Completely different sentence about cats and dogs.");
  const words = ["red", "green", "blue", "yellow"].map((norm, i) => ({
    norm,
    start_ms: i,
    end_ms: i + 1
  }));
  assert.equal(alignWords(words, tokens, 0), null);
});

test("alignWords returns null when under half the words map", () => {
  const tokens = wordTokens("one two");
  const words = ["one", "x", "y", "z"].map((norm, i) => ({ norm, start_ms: i, end_ms: i + 1 }));
  assert.equal(alignWords(words, tokens, 0), null);
});

console.log(`\n${passed} tests passed`);
