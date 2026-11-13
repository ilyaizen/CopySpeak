import { describe, test, expect } from "bun:test";
import { JSDOM } from "jsdom";
import { CaptionGlide } from "../src/glide";

type Rect = { left: number; top: number; width: number; height: number };

interface FakeHighlight {
  ranges: Range[];
  [Symbol.iterator](): Iterator<Range>;
}

function fixture(body: string, options: { matchMedia?: (query: string) => { matches: boolean } } = {}) {
  const dom = new JSDOM(body, { pretendToBeVisual: true });
  const view = dom.window;
  const d = view.document;
  const highlights = new Map<string, FakeHighlight>();
  class HighlightFake implements FakeHighlight {
    ranges: Range[];
    constructor(...ranges: Range[]) {
      this.ranges = ranges;
    }
    *[Symbol.iterator](): Iterator<Range> {
      yield* this.ranges;
    }
  }
  // SAFETY: installs a fake Custom Highlight API; jsdom ships none, and the
  // glide only needs a Map-like registry plus a range-holding constructor.
  Object.assign(view, { Highlight: HighlightFake, CSS: { highlights } });
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
  const painted = (name: string): string[] => {
    const set = highlights.get(name);
    if (set === undefined) return [];
    return [...set].map((range) => range.toString());
  };
  const mark = () => {
    const found = d.querySelector<HTMLElement>(".copyspeak-mark");
    if (found === null) throw new Error("the pill is not painted");
    return found;
  };
  return { view, d, highlights, setRects, rect, painted, mark, tick };
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

/** Manual rAF: the test delivers every rendered frame at a chosen delay. */
function manualRaf(view: Window) {
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
  test("the first word paints the pill and settles instantly", () => {
    const { d, setRects, rect, painted, mark } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    setRects(range, [rect(40, 20, 30, 12)]);
    glide.paint(range, p, "0:3");
    expect(painted("copyspeak-word")).toEqual(["one"]);
    expect(p.getAttribute("data-copyspeak-mark-host")).toBe("");
    expect(p.style.getPropertyValue("--copyspeak-word-bg")).toBe("transparent");
    expect(mark().style.left).toBe("38px");
    expect(mark().style.top).toBe("19px");
    expect(mark().style.width).toBe("34px");
    expect(mark().style.height).toBe("14px");
  });

  test("a second word glides, then settles with the pill over it", async () => {
    const { d, setRects, rect, painted, mark } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    // A concrete inherited color keeps the probe's plain-color resolve stable.
    p.style.color = "rgb(10, 20, 30)";
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    // Mid-glide: both words carry glide names, none carries the settled name.
    expect(painted("copyspeak-word")).toEqual([]);
    expect(painted("copyspeak-word-out")).toEqual(["one"]);
    expect(painted("copyspeak-word-in")).toEqual(["two"]);
    expect(mark().style.width).toBe("34px");
    await tick(250);
    expect(painted("copyspeak-word")).toEqual(["two"]);
    expect(painted("copyspeak-word-out")).toEqual([]);
    expect(painted("copyspeak-word-in")).toEqual([]);
    expect(mark().style.left).toBe("78px");
    expect(mark().style.width).toBe("24px");
    expect(mark().style.transform).toBe("");
    expect(p.style.getPropertyValue("--copyspeak-out-color")).toBe("");
    expect(p.style.getPropertyValue("--copyspeak-in-color")).toBe("");
  });

  test("the animation off switch jumps between words", () => {
    const { d, setRects, rect, painted } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, false);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    expect(painted("copyspeak-word")).toEqual(["two"]);
    expect(painted("copyspeak-word-out")).toEqual([]);
    expect(painted("copyspeak-word-in")).toEqual([]);
  });

  test("reduced motion jumps between words even with animation on", () => {
    const { d, setRects, rect, painted } = fixture("<main><p>one two</p></main>", {
      matchMedia: (query) => ({ matches: query.includes("reduce") })
    });
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    expect(painted("copyspeak-word")).toEqual(["two"]);
    expect(painted("copyspeak-word-out")).toEqual([]);
  });

  test("a repeated word only re-places a moved pill", async () => {
    const { d, setRects, rect, painted, mark } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    p.style.color = "rgb(10, 20, 30)";
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    await tick(250);
    expect(painted("copyspeak-word")).toEqual(["two"]);
    // The page moved the word: the same frame re-places the pill, no glide.
    setRects(second, [rect(200, 40, 20, 12)]);
    glide.paint(second, p, "4:7");
    expect(mark().style.left).toBe("198px");
    expect(mark().style.top).toBe("39px");
    expect(painted("copyspeak-word-out")).toEqual([]);
    expect(painted("copyspeak-word")).toEqual(["two"]);
  });

  test("clearWord drops the paint but keeps the pill layer", () => {
    const { d, setRects, rect, painted } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    setRects(range, [rect(40, 20, 30, 12)]);
    glide.paint(range, p, "0:3");
    glide.clearWord();
    expect(painted("copyspeak-word")).toEqual([]);
    expect(painted("copyspeak-word-out")).toEqual([]);
    expect(painted("copyspeak-word-in")).toEqual([]);
    expect(p.querySelector(".copyspeak-markLayer")).not.toBeNull();
    expect(p.getAttribute("data-copyspeak-mark-host")).toBe("");
  });

  test("release strips the pill, its layer, and the scope's style", () => {
    const { d, setRects, rect } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    setRects(range, [rect(40, 20, 30, 12)]);
    glide.paint(range, p, "0:3");
    glide.release();
    expect(p.getAttribute("data-copyspeak-mark-host")).toBeNull();
    expect(p.querySelector(".copyspeak-markLayer")).toBeNull();
    expect(p.style.getPropertyValue("--copyspeak-word-bg")).toBe("");
  });

  test("a browser without the highlight registry paints nothing", () => {
    const dom = new JSDOM("<main><p>one</p></main>", { pretendToBeVisual: true });
    const d = dom.window.document;
    const p = d.querySelector("p")!;
    const glide = new CaptionGlide(d, true);
    const range = wordRange(d, "p", 0, 3);
    glide.paint(range, p, "0:3");
    expect(p.getAttribute("data-copyspeak-mark-host")).toBeNull();
    expect(p.querySelector(".copyspeak-markLayer")).toBeNull();
  });

  test("a late first frame anchors the glide instead of snapping", async () => {
    const { d, view, setRects, rect, painted, mark } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    p.style.color = "rgb(10, 20, 30)";
    const pump = manualRaf(view);
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    // A busy page delivers the first rendered frame late; the glide must
    // still start from its beginning, not land at the end position.
    await pump(130);
    expect(painted("copyspeak-word")).toEqual([]);
    expect(painted("copyspeak-word-out")).toEqual(["one"]);
    expect(painted("copyspeak-word-in")).toEqual(["two"]);
    expect(mark().style.transform).toBe("translate(-40px,0px)");
  });

  test("late frames stretch the glide instead of collapsing it", async () => {
    const { d, view, setRects, rect, painted, mark } = fixture("<main><p>one two</p></main>");
    const p = d.querySelector("p")!;
    p.style.color = "rgb(10, 20, 30)";
    const pump = manualRaf(view);
    const glide = new CaptionGlide(d, true);
    const first = wordRange(d, "p", 0, 3);
    setRects(first, [rect(40, 20, 30, 12)]);
    glide.paint(first, p, "0:3");
    const second = wordRange(d, "p", 4, 7);
    setRects(second, [rect(80, 20, 20, 12)]);
    glide.paint(second, p, "4:7");
    // Each rendered frame advances at most one cap of the glide clock: three
    // late frames stretch 120 ms over ~400 ms of wall time and still settle.
    await pump(0);
    await pump(130);
    expect(painted("copyspeak-word")).toEqual([]);
    await pump(130);
    expect(painted("copyspeak-word")).toEqual([]);
    await pump(130);
    expect(painted("copyspeak-word")).toEqual(["two"]);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("78px");
  });
});
