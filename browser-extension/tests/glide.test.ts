import { describe, test, expect } from "bun:test";
import { JSDOM } from "jsdom";
import { CaptionGlide } from "../src/glide";

type Rect = { left: number; top: number; width: number; height: number };

function fixture(
  body: string,
  options: { matchMedia?: (query: string) => { matches: boolean } } = {}
) {
  const dom = new JSDOM(body, { pretendToBeVisual: true });
  const view = dom.window;
  const d = view.document;
  // SAFETY: jsdom has no layout engine; per-range rects come from the test,
  // handed to the glide through Range.getClientRects.
  const rectStore = new WeakMap<Range, Rect[]>();
  view.Range.prototype.getClientRects = function (): DOMRectList {
    const rects = rectStore.get(this) ?? [];
    const list: Record<number, Rect> = {};
    rects.forEach((rect, index) => {
      list[index] = rect;
    });
    // SAFETY: hand-builds the DOMRectList shape; no layout engine exists to make one.
    return { length: rects.length, item: (index) => rects[index] ?? null, ...list } as DOMRectList;
  };
  if (options.matchMedia) {
    Object.assign(view, { matchMedia: options.matchMedia });
  }
  const setRects = (range: Range, rects: Rect[]) => rectStore.set(range, rects);
  const rect = (left: number, top: number, width: number, height: number): Rect => ({
    left,
    top,
    width,
    height
  });
  const mark = () => {
    const found = d.querySelector<HTMLElement>(".copyspeech-mark");
    if (found === null) throw new Error("the pill is not painted");
    return found;
  };
  return { view, d, setRects, rect, mark };
}

function wordRange(d: Document, selector: string, start: number, end: number): Range {
  const p = d.querySelector(selector);
  if (p === null || p.firstChild === null) throw new Error("fixture paragraph missing");
  const range = d.createRange();
  range.setStart(p.firstChild, start);
  range.setEnd(p.firstChild, end);
  return range;
}

const tick = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** The horizontal distance the pill still has to travel, in px. */
function remaining(mark: HTMLElement): number {
  return Math.abs(parseFloat(mark.style.transform.slice("translate(".length)) || 0);
}

/** Manual rAF: the test delivers every rendered frame at a chosen delay. */
function manualRaf(view: { performance: { now(): number } }) {
  const queue: { id: number; run: () => void }[] = [];
  const cancelled = new Set<number>();
  let nextId = 1;
  // SAFETY: replaces jsdom's timer rAF so tests control frame timing; the
  // glide only needs the schedule/cancel surface.
  Object.assign(view, {
    requestAnimationFrame: (cb: (now: number) => void) => {
      const id = nextId;
      nextId += 1;
      queue.push({ id, run: () => cb(view.performance.now()) });
      return id;
    },
    cancelAnimationFrame: (id: number) => {
      cancelled.add(id);
    }
  });
  return async (delayMs: number) => {
    await tick(delayMs);
    const entry = queue.shift();
    if (entry !== undefined && !cancelled.has(entry.id)) entry.run();
  };
}

describe("caption glide", () => {
  test("the first word paints the pill over it and settles instantly", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    setRects(range, [rect(40, 20, 30, 12)]);
    glide.paint(range, "0:3");
    // 4 px of horizontal and 2 px of vertical padding around the word.
    expect(mark().style.left).toBe("36px");
    expect(mark().style.top).toBe("18px");
    expect(mark().style.width).toBe("38px");
    expect(mark().style.height).toBe("16px");
    expect(mark().style.transform).toBe("");
    expect(mark().style.mixBlendMode).toBe("difference");
    // The white fill under difference inverts the word and its backdrop.
    expect(mark().style.backgroundColor).toBe("rgb(255, 255, 255)");
    expect(mark().style.zIndex).toBe("2147483646");
    expect(mark().getAttribute("aria-hidden")).toBe("true");
    expect(mark().parentElement).toBe(d.documentElement);
  });

  test("a second word glides swiftly, then settles over it", async () => {
    const { d, view, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const pump = manualRaf(view);
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    // The tween starts synchronously from the previous pill position.
    expect(mark().style.transform).toBe("translate(-40px)");
    expect(mark().style.width).toBe("38px");
    await pump(0);
    // One rendered frame later the swift curve has taken most of the way.
    await pump(40);
    expect(remaining(mark())).toBeLessThan(8);
    await pump(40);
    await pump(40);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
    expect(mark().style.width).toBe("28px");
  });

  test("the animation off switch jumps between words", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, false);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    expect(mark().style.left).toBe("76px");
    expect(mark().style.transform).toBe("");
  });

  test("reduced motion jumps between words even with animation on", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two</p></main>", {
      matchMedia: (query) => ({ matches: query.includes("reduce") })
    });
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    expect(mark().style.left).toBe("76px");
    expect(mark().style.transform).toBe("");
  });

  test("a repeated word only re-places a moved pill", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 4, 7);
    setRects(range, [rect(80, 20, 20, 12)]);
    glide.paint(range, "4:7");
    // The page moved the word: the same frame re-places the pill, no glide.
    setRects(range, [rect(200, 40, 20, 12)]);
    glide.paint(range, "4:7");
    expect(mark().style.left).toBe("196px");
    expect(mark().style.top).toBe("38px");
    expect(mark().style.transform).toBe("");
  });

  test("a wordless frame holds the paint so the next word still glides", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two three</p></main>");
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    /* A wordless state frame (caption gap, trailing fragment audio, a
       degraded fragment) must keep the pill: tearing the glide down makes
       the NEXT word settle instantly. */
    glide.hold();
    expect(mark().style.left).toBe("36px");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    expect(mark().style.transform).toBe("translate(-40px)");
  });

  test("a wordless frame mid-glide leaves the tween to settle", async () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two three</p></main>");
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    // The gap frame lands while the pill is still traveling to "two".
    glide.hold();
    await tick(250);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
    const third = wordRange(d, "p", 8, 13);
    setRects(third, [rect(110, 20, 24, 12)]);
    glide.paint(third, "8:13");
    expect(remaining(mark())).toBeGreaterThan(0);
  });

  test("a word with no measurable rect leaves no pill", () => {
    const { d, setRects, rect } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const collapsed = wordRange(d, "p", 4, 7);
    setRects(collapsed, [rect(0, 0, 0, 0)]);
    glide.paint(collapsed, "4:7");
    expect(d.querySelector(".copyspeech-mark")).toBeNull();
  });

  test("release strips the pill", () => {
    const { d, setRects, rect } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    setRects(range, [rect(40, 20, 30, 12)]);
    glide.paint(range, "0:3");
    glide.release();
    expect(d.querySelector(".copyspeech-mark")).toBeNull();
  });

  test("a late first frame anchors the glide instead of snapping", async () => {
    const { d, view, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const pump = manualRaf(view);
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    // A busy page delivers the first rendered frame late; the glide must
    // still start from its beginning, not land at the end position.
    await pump(130);
    expect(mark().style.transform).toBe("translate(-40px)");
  });

  test("late frames stretch the glide instead of collapsing it", async () => {
    const { d, view, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const pump = manualRaf(view);
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    // Each rendered frame advances at most one cap of the glide clock: three
    // late frames stretch 120 ms over ~400 ms of wall time and still settle.
    await pump(0);
    await pump(130);
    expect(mark().style.transform).not.toBe("");
    await pump(130);
    expect(mark().style.transform).not.toBe("");
    await pump(130);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
  });

  test("turning the animation off makes the next word jump", () => {
    const { d, setRects, rect, mark } = fixture("<main><p>one two</p></main>");
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, "0:3");
    glide.setAnimated(false);
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, "4:7");
    expect(mark().style.left).toBe("76px");
    expect(mark().style.transform).toBe("");
  });
});
