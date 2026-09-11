import { expect, it } from "vite-plus/test";
import { activeCaptionWord, buildCaptions, captionPhrase } from "./captions";

it("preserves multilingual text and punctuation while making short phrases", () => {
  for (const text of [
    "Hello, world! This is a longer sentence with several words to read aloud.",
    "שלום עולם! זה טקסט בעברית, עם סימני פיסוק.",
    "你好世界。这是一个测试。",
    "  “Read this!” Then continue."
  ]) {
    const words = buildCaptions(text);
    expect(words.map((word) => word.text).join("")).toBe(text);
    for (const phrase of new Set(words.map((word) => word.phrase))) {
      const group = words.filter((word) => word.phrase === phrase);
      expect(group.length).toBeLessThanOrEqual(7);
      expect(group.map((word) => word.text).join("").length).toBeLessThanOrEqual(42);
    }
  }
});

it("selects words at audio boundaries and safely handles missing timing", () => {
  const text = "One two. Three!";
  const words = buildCaptions(text, {
    text,
    words: [
      { text_start: 0, text_end: 3, start_ms: 120, end_ms: 220 },
      { text_start: 4, text_end: 7, start_ms: 650, end_ms: 700 },
      { text_start: 9, text_end: 14, start_ms: 1300, end_ms: 1900 }
    ]
  });
  expect(activeCaptionWord(words, 0)).toBe(-1);
  expect(activeCaptionWord(words, 120)).toBe(0);
  expect(activeCaptionWord(words, 220)).toBe(-1);
  expect(activeCaptionWord(words, 650)).toBe(1);
  expect(activeCaptionWord(words, 1000)).toBe(-1);
  expect(activeCaptionWord(words, 1300)).toBe(2);
  expect(activeCaptionWord(words, 1900)).toBe(-1);
  expect(activeCaptionWord(words, NaN)).toBe(-1);
  expect(activeCaptionWord(buildCaptions(text), 650)).toBe(-1);
  expect(captionPhrase(words, 2000)).toBe(1);
  expect(words[2].phrase).toBe(1);
});

it("uses normalized character timings, handles UTF-16, and rejects missing or corrupt spans", () => {
  const text = "😀 Hi, twenty!";
  const timing = {
    text,
    words: [
      { text_start: 3, text_end: 4, start_ms: 200, end_ms: 240 },
      { text_start: 4, text_end: 5, start_ms: 240, end_ms: 330 },
      { text_start: 7, text_end: 13, start_ms: 800, end_ms: 1000 }
    ]
  };
  expect(buildCaptions(text, timing)[0].start).toBe(200);
  expect(activeCaptionWord(buildCaptions(text, timing), 820)).toBe(1);
  expect(buildCaptions(text, { ...timing, words: timing.words.slice(1) })[0].start).toBeNull();
  expect(buildCaptions(text, { ...timing, text: "different" })[0].start).toBeNull();
  expect(
    buildCaptions(text, { ...timing, words: [{ ...timing.words[0], end_ms: -1 }] })[0].start
  ).toBeNull();
});
