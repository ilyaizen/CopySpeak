import { expect, it } from "vitest";
import { activeCaptionWord, buildCaptions } from "./captions";

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
  const words = buildCaptions("One two. Three!");
  const duration = words.at(-1)!.end * 1000;
  expect(activeCaptionWord(words, 0, duration)).toBe(0);
  expect(activeCaptionWord(words, words[1].start * 1000, duration)).toBe(1);
  expect(activeCaptionWord(words, duration, duration)).toBe(2);
  expect(activeCaptionWord(words, duration * 2, duration)).toBe(2);
  expect(activeCaptionWord(words, 0, 0)).toBe(-1);
  expect(activeCaptionWord(words, NaN, duration)).toBe(-1);
  expect(activeCaptionWord(buildCaptions("..."), 0, 1000)).toBe(-1);
  expect(words[2].phrase).toBe(1);
});
