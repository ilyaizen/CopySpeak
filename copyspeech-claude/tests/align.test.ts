import { expect, test } from "claude-code/testing";

import {
  align,
  blocks,
  captionWords,
  dropSkippedMarkup,
  excerptSpeech,
  inlineSpans,
  markWord,
  prepareSpeech,
  speakable
} from "../hooks/register";

test("excerpt keeps top and bottom paragraphs, as Hermes does", async () => {
  const text = "a\n\nb\n\nc\n\nd\n\ne";
  expect(excerptSpeech(text, { mode: "both", top: 2, bottom: 2 })).toBe("a\n\nb\n\nd\n\ne");
  expect(excerptSpeech(text, { mode: "top", top: 1, bottom: 9 })).toBe("a");
  expect(excerptSpeech(text, { mode: "bottom", top: 9, bottom: 1 })).toBe("e");
  expect(excerptSpeech(text, { mode: "both", top: 3, bottom: 2 })).toBe(text);
  expect(excerptSpeech(text, { mode: "both", top: 0, bottom: 99 })).toBe(text);
});

test("skip drops headings, items, rules, and table rows but keeps text under a heading", async () => {
  const md = "## Answer\nYes it works.\n\n- one\n1. two\n---\n| a | b |\nDone.";
  expect(dropSkippedMarkup(md)).toBe("Yes it works.\n\nDone.");
  expect(prepareSpeech(md, true, null)).toBe("Yes it works.\n\nDone.");
  expect(prepareSpeech("- one\n- two", false, null)).toBe("one\ntwo");
});

const MD = "The **quick** fox. Run `npm test` then [read docs](https://x.y/z) now.";

test("speakable skips code and link targets", async () => {
  const { tokens, speech } = speakable(MD);
  expect(tokens.map((t) => t.norm)).toEqual([
    "the",
    "quick",
    "fox",
    "run",
    "then",
    "read",
    "docs",
    "now"
  ]);
  expect(speech).not.toContain("npm");
  expect(speech).not.toContain("https");
});

test("caption words map onto markdown offsets and mark the spoken word", async () => {
  const { tokens } = speakable(MD);
  const text = "quick fox run";
  const caption = {
    text,
    words: [
      { text_start: 0, text_end: 5, start_ms: 0, end_ms: 300 },
      { text_start: 6, text_end: 9, start_ms: 300, end_ms: 600 },
      { text_start: 10, text_end: 13, start_ms: 600, end_ms: 900 }
    ]
  };
  const words = captionWords(caption);
  const a = align(words, tokens, 0);
  expect(a.ok).toBe(true);
  const fox = tokens[a.map[1] ?? -1];
  expect(fox?.norm).toBe("fox");
  if (fox === undefined) return;
  expect(markWord(MD, [fox.start, fox.end], "code")).toContain("`fox`");
  expect(markWord(MD, [fox.start, fox.end], "off")).toBe(MD);
});

test("blocks keep fences whole and spans carry the chip", async () => {
  const md = "# Title\n\nSay **bold** and [link](https://a.b).\n\n```\nx\n\ny\n```\n\n- item one";
  const bs = blocks(md);
  expect(bs.map((b) => md.slice(b.start, b.end))).toEqual([
    "# Title",
    "Say **bold** and [link](https://a.b).",
    "```\nx\n\ny\n```",
    "- item one"
  ]);
  const para = bs[1];
  if (para === undefined) return;
  const at = md.indexOf("bold");
  const spans = inlineSpans(md, para, [at, at + 4]);
  expect(spans.map((s) => s.text).join("")).toBe("Say bold and link.");
  expect(spans.find((s) => s.hit)).toEqual({ text: "bold", bold: true, hit: true });
  const list = bs[3];
  if (list === undefined) return;
  expect(
    inlineSpans(md, list, null)
      .map((s) => s.text)
      .join("")
  ).toBe("• item one");
});
