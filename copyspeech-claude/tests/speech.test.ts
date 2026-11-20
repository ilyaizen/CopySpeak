import { expect, test } from "claude-code/testing";

import {
  align,
  captionWords,
  blocks,
  inlineSpans,
  pickParagraphs,
  prepareSpeech,
  speakable,
  wordTokens
} from "../hooks/register";

test("excerpt keeps top and bottom paragraphs, as Hermes does", async () => {
  const paras = ["a", "b", "c", "d", "e"];
  expect(pickParagraphs(paras, { mode: "both", top: 2, bottom: 2 })).toEqual(["a", "b", "d", "e"]);
  expect(pickParagraphs(paras, { mode: "top", top: 1, bottom: 9 })).toEqual(["a"]);
  expect(pickParagraphs(paras, { mode: "bottom", top: 9, bottom: 1 })).toEqual(["e"]);
  expect(pickParagraphs(paras, { mode: "both", top: 3, bottom: 2 })).toEqual(paras);
  expect(pickParagraphs(paras, { mode: "both", top: 0, bottom: 99 })).toEqual(paras);
});

test("skip drops headings, items, rules, and table rows but keeps text under a heading", async () => {
  const md = "## Answer\nYes it works.\n\n- one\n1. two\n---\n| a | b |\nDone.";
  expect(prepareSpeech([{ row: "r", md }], true, null).speech).toBe("Yes it works.\n\nDone.");
  expect(prepareSpeech([{ row: "r", md: "- one\n- two" }], false, null).speech).toBe("one\ntwo");
});

const MD = "The **quick** fox. Run `npm test` then [read docs](https://x.y/z) now.";

test("speakable reads inline code and link text, and skips link targets", async () => {
  const { tokens, speech } = speakable(MD);
  expect(tokens.map((t) => t.norm)).toEqual([
    "the",
    "quick",
    "fox",
    "run",
    "npm",
    "test",
    "then",
    "read",
    "docs",
    "now"
  ]);
  expect(speech).toBe("The quick fox. Run npm test then read docs now.");
});

test("speakable skips fenced code blocks of both fence kinds", async () => {
  const md =
    "Before.\n\n```ts\nconst hidden = 1;\n```\n\nMid `kept`.\n\n~~~\nalso hidden\n~~~\n\nAfter.";
  const { tokens, speech } = speakable(md);
  expect(tokens.map((t) => t.norm)).toEqual(["before", "mid", "kept", "after"]);
  expect(speech).not.toContain("hidden");
  expect(speech).toContain("Mid kept.");
});

// One caption entry per spoken word, 300 ms each, as CopySpeech sends them.
function captionFor(speech: string) {
  return {
    text: speech,
    words: wordTokens(speech).map((t, i) => ({
      text_start: t.start,
      text_end: t.end,
      start_ms: i * 300,
      end_ms: i * 300 + 300
    }))
  };
}

const LONG = [
  "## Plan",
  "",
  "- Read the file",
  "- Check the output",
  "",
  "Run the build first. It takes a while.",
  "",
  "Then open the log and read every line of the output for the warnings, the errors, and the timing numbers that the build prints at the end.",
  "",
  "Finally, report the result to the user."
].join("\n");

test("captions align to the spoken words across a heading, a list, and an excerpt gap", async () => {
  const spoken = prepareSpeech([{ row: "reply", md: LONG }], true, {
    mode: "both",
    top: 1,
    bottom: 1
  });
  expect(spoken.speech).toBe(
    "Run the build first. It takes a while.\n\nFinally, report the result to the user."
  );
  // Tokens are exactly the spoken words, and each offset points at its word in the markdown.
  expect(spoken.tokens.map((t) => t.norm)).toEqual(wordTokens(spoken.speech).map((t) => t.norm));
  for (const t of spoken.tokens) {
    expect(LONG.slice(t.start, t.end).toLowerCase()).toBe(t.norm);
    expect(t.row).toBe("reply");
  }
  const words = captionWords(captionFor(spoken.speech));
  const a = align(words, spoken.tokens, 0);
  expect(a.ok).toBe(true);
  words.forEach((w, i) => expect(spoken.tokens[a.map[i] ?? -1]?.norm).toBe(w.norm));
  const finally_ = spoken.tokens.find((t) => t.norm === "finally");
  expect(finally_).toBeDefined();
  if (finally_ === undefined) return;
  const para = blocks(LONG).find((b) => b.start <= finally_.start && finally_.start < b.end);
  if (para === undefined) return;
  expect(inlineSpans(LONG, para, [finally_.start, finally_.end])[0]).toEqual({
    text: "Finally",
    hit: true
  });
});

test("blocks keep fences whole and split on blank lines", async () => {
  const md =
    "# Title\n\nSay **bold** and [link](https://a.b).\n\n```\nx\n\ny\n```\n\n~~~\np\n\nq\n~~~\n\n- item one";
  const bs = blocks(md);
  expect(bs.map((b) => md.slice(b.start, b.end))).toEqual([
    "# Title",
    "Say **bold** and [link](https://a.b).",
    "```\nx\n\ny\n```",
    "~~~\np\n\nq\n~~~",
    "- item one"
  ]);
});

test("inline spans carry the chip on the spoken word only", async () => {
  const md = "# Title\n\nSay **bold** and [link](https://a.b).\n\n- item one";
  const bs = blocks(md);
  const para = bs[1];
  if (para === undefined) return;
  const at = md.indexOf("bold");
  const spans = inlineSpans(md, para, [at, at + 4]);
  expect(spans.map((s) => s.text).join("")).toBe("Say bold and link.");
  expect(spans.find((s) => s.hit)).toEqual({ text: "bold", bold: true, hit: true });
  const list = bs[2];
  if (list === undefined) return;
  expect(
    inlineSpans(md, list, null)
      .map((s) => s.text)
      .join("")
  ).toBe("• item one");
});
