import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { CaptionGlider, captionGlide } from "../caption-glide";

type Rect = { left: number; top: number; width: number; height: number };

function rect(left: number, top: number, width: number, height: number): Rect {
  return { left, top, width, height };
}

function setRects(el: HTMLElement, rects: Rect[]) {
  vi.spyOn(el, "getClientRects").mockImplementation(() => {
    const list: Record<number, Rect> = {};
    rects.forEach((r, index) => {
      list[index] = r;
    });
    // SAFETY: hand-builds the DOMRectList shape; no layout engine exists to make one.
    return {
      length: rects.length,
      item: (index: number) => rects[index] ?? null,
      ...list
    } as DOMRectList;
  });
}

function fixture() {
  const host = document.createElement("div");
  const a = document.createElement("span");
  const b = document.createElement("span");
  a.setAttribute("data-word-offset", "0");
  b.setAttribute("data-word-offset", "5");
  host.append(a, b);
  document.body.appendChild(host);
  // SAFETY: a plain rect object stands in for the layout engine.
  vi.spyOn(host, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 200,
    height: 100
  } as DOMRect);
  const mark = () => {
    const found = host.querySelector<HTMLElement>(".caption-glide-mark");
    if (found === null) throw new Error("the pill is not painted");
    return found;
  };
  return { host, a, b, mark };
}

function manualRaf() {
  const queue: { id: number; run: () => void }[] = [];
  const cancelled = new Set<number>();
  let nextId = 1;
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    const id = nextId;
    nextId += 1;
    queue.push({ id, run: () => cb(performance.now()) });
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    cancelled.add(id);
  });
  return async (delayMs: number) => {
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    const entry = queue.shift();
    if (entry !== undefined && !cancelled.has(entry.id)) entry.run();
  };
}

describe("caption glide (KISS: mix-blend-mode: difference)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  it("the first word paints the pill and settles instantly", () => {
    const { host, a, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    expect(mark().style.left).toBe("36px");
    expect(mark().style.top).toBe("18px");
    expect(mark().style.width).toBe("38px");
    expect(mark().style.height).toBe("16px");
    expect(mark().style.mixBlendMode).toBe("difference");
    // The white fill under difference inverts the word and its backdrop.
    expect(mark().style.backgroundColor).toBe("rgb(255, 255, 255)");
    expect(mark().style.borderRadius).toBe("4px");
  });

  it("a second word glides, then settles with the pill over it", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    // Mid-glide: pill animates from a to b, no color blending needed
    expect(mark().style.transform).not.toBe("");
    expect(mark().style.width).toBe("38px");
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(mark().style.left).toBe("76px");
    expect(mark().style.width).toBe("28px");
    expect(mark().style.transform).toBe("");
    // No inline colors on words - difference blend handles it
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
  });

  it("the animation off switch jumps between words", () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const glide = new CaptionGlider(host, false);
    glide.paint(a);
    glide.paint(b);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.left).toBe("76px");
    expect(mark().style.transform).toBe("");
  });

  it("reduced motion jumps between words even with animation on", async () => {
    const { host, a, b, mark } = fixture();
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("reduce") }));
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    await pump(0);
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    expect(a.style.color).toBe("");
    expect(mark().style.left).toBe("76px");
  });

  it("unmeasurable word settles without marks", () => {
    const { host, b } = fixture();
    const glide = new CaptionGlider(host, true);
    glide.paint(b);
    expect(host.querySelectorAll(".caption-glide-mark")).toHaveLength(0);
    expect(b.style.color).toBe("");
  });

  it("clear drops the paint and the mark", () => {
    const { host, a } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.clear();
    expect(host.querySelectorAll(".caption-glide-mark")).toHaveLength(0);
  });

  it("release strips the mark and the paint", () => {
    const { host, a } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.release();
    expect(host.querySelector(".caption-glide-mark")).toBeNull();
    expect(a.style.color).toBe("");
  });

  it("a gap between words holds the pill (no teardown)", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(null);
    expect(mark().style.left).toBe("36px");
    glide.paint(b);
    expect(mark().style.transform).not.toBe("");
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    glide.paint(null);
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
  });

  it("a new resetKey clears the held paint", () => {
    const { host, a, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const words = [{ text: "one", offset: 0, start: 0, end: 100, phrase: 0 }];
    const action = captionGlide(host, { words, activeIndex: 0, animated: true, resetKey: "r1" });
    action.update?.({ words, activeIndex: -1, animated: true, resetKey: "r1" });
    expect(mark().style.left).toBe("36px");
    action.update?.({ words, activeIndex: 0, animated: true, resetKey: "r2" });
    expect(a.style.color).toBe("");
    expect(mark().style.left).toBe("36px");
    expect(mark().style.transform).toBe("");
    action.destroy?.();
  });

  it("the action holds the paint on -1 and clears on destroy", () => {
    const { host, a } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const words = [{ text: "one", offset: 0, start: 0, end: 100, phrase: 0 }];
    const action = captionGlide(host, { words, activeIndex: 0, animated: true });
    expect(host.querySelector(".caption-glide-mark")).not.toBeNull();
    action.update?.({ words, activeIndex: -1, animated: true });
    expect(host.querySelector(".caption-glide-mark")).not.toBeNull();
    action.destroy?.();
    expect(host.querySelector(".caption-glide-mark")).toBeNull();
    expect(a.style.color).toBe("");
  });

  it("painting the same word mid-glide does not restart the glide", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    await pump(0);
    await pump(60);
    const midFlight = mark().style.transform;
    expect(midFlight).not.toBe("translate(-40px,0px)");
    glide.paint(b);
    expect(mark().style.transform).toBe(midFlight);
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
  });

  it("a repeated word only re-places a moved pill", () => {
    const { host, b, mark } = fixture();
    setRects(b, [rect(80, 20, 20, 12)]);
    const glide = new CaptionGlider(host, true);
    glide.paint(b);
    vi.restoreAllMocks();
    setRects(b, [rect(200, 40, 20, 12)]);
    glide.paint(b);
    expect(mark().style.left).toBe("196px");
    expect(mark().style.top).toBe("38px");
  });

  it("late frames stretch the glide instead of collapsing it", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    await pump(0);
    await pump(130);
    expect(mark().style.transform).not.toBe("");
    await pump(130);
    expect(mark().style.transform).not.toBe("");
    await pump(130);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("76px");
  });

  it("a late first frame anchors the glide instead of snapping", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    await pump(130);
    expect(mark().style.transform).not.toBe("");
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
  });

  it("a word on a new line snaps to that line and animates its width there", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    // The wrapped word starts on the next line.
    setRects(b, [rect(80, 44, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    // No diagonal travel across the old line: the pill sits on the new line
    // from the first frame and only its width morphs.
    expect(mark().style.left).toBe("76px");
    expect(mark().style.top).toBe("42px");
    expect(mark().style.transform).not.toBe("");
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(mark().style.left).toBe("76px");
    expect(mark().style.top).toBe("42px");
    expect(mark().style.width).toBe("28px");
    expect(mark().style.transform).toBe("");
  });
});
