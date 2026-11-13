import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { CaptionGlider, captionGlide } from "./caption-glide";
import type { GlideColors } from "./caption-glide";

type Rect = { left: number; top: number; width: number; height: number };

/* Explicit rgb() colors: jsdom resolves an inline color to its literal
   string, so the probe parser produces real numbers for the blends. */
const COLORS: GlideColors = {
  knockout: "rgb(255, 255, 255)",
  spoken: "rgb(10, 20, 30)",
  plain: "rgb(40, 50, 60)"
};

function rect(left: number, top: number, width: number, height: number): Rect {
  return { left, top, width, height };
}

/** jsdom has no layout engine; each test hands the glide its own word rects. */
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
  vi.spyOn(host, "getBoundingClientRect").mockReturnValue(
    // SAFETY: a plain rect object stands in for the layout engine.
    { left: 0, top: 0, width: 200, height: 100 } as DOMRect
  );
  const mark = () => {
    const found = host.querySelector<HTMLElement>(".caption-glide-mark");
    if (found === null) throw new Error("the pill is not painted");
    return found;
  };
  const layer = () => host.querySelector<HTMLElement>(".caption-glide-layer");
  return { host, a, b, mark, layer };
}

/** Manual rAF: the test delivers every rendered frame at a chosen delay. */
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

describe("caption glide", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  it("the first word paints the pill and settles instantly", () => {
    const { host, a, mark, layer } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    expect(layer()).not.toBeNull();
    expect(host.style.getPropertyValue("--caption-word-bg")).toBe("transparent");
    expect(mark().style.left).toBe("38px");
    expect(mark().style.top).toBe("19px");
    expect(mark().style.width).toBe("34px");
    expect(mark().style.height).toBe("14px");
  });

  it("a second word glides, then settles with the pill over it", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    // Mid-glide: the incoming word starts plain, the outgoing stays knockout.
    expect(a.style.color).toBe("rgb(255, 255, 255)");
    expect(b.style.color).toBe("rgb(40, 50, 60)");
    expect(mark().style.width).toBe("34px");
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(b.style.color).toBe("");
    expect(a.style.color).toBe("");
    expect(mark().style.left).toBe("78px");
    expect(mark().style.width).toBe("24px");
    expect(mark().style.transform).toBe("");
  });

  it("the animation off switch jumps between words", () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const glide = new CaptionGlider(host, false, COLORS);
    glide.paint(a);
    glide.paint(b);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.left).toBe("78px");
    expect(mark().style.transform).toBe("");
  });

  it("reduced motion jumps between words even with animation on", async () => {
    const { host, a, b, mark } = fixture();
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("reduce") }));
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    await pump(0);
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    expect(a.style.color).toBe("");
    expect(mark().style.left).toBe("78px");
  });

  it("unresolvable colors keep the instant, correct swap", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    await pump(0);
    // jsdom cannot resolve var() tokens, so the blend targets stay unknown.
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    expect(mark().style.left).toBe("78px");
    expect(mark().style.transform).toBe("");
    expect(b.style.color).toBe("");
  });

  it("oklch tokens reported by Chromium glide with blended sRGB colors", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    /* Chromium serializes computed oklch tokens space-preserving as
       oklch(...) (probed on WebView2/Edge 157): the probe hands the parser
       oklch strings for every var() token. */
    vi.spyOn(window, "getComputedStyle").mockImplementation((el: Element) => {
      const probe = el instanceof HTMLElement ? el.style.color : "";
      const color =
        probe === "var(--primary-foreground)"
          ? "oklch(0.99 0.01 0)"
          : probe === "var(--muted-foreground)"
            ? "oklch(0.5 0.01 190)"
            : probe === "var(--foreground)"
              ? "oklch(0.2 0.01 190)"
              : "";
      // SAFETY: the glide reads only the color property off the declaration.
      return { color } as CSSStyleDeclaration;
    });
    const glide = new CaptionGlider(host, true);
    glide.paint(a);
    glide.paint(b);
    // Expected values are Chromium's own oklch-to-sRGB conversion, read back
    // from a canvas on the same engine (Edge/WebView2 157).
    // jsdom re-serializes inline colors with spaces after the commas.
    expect(a.style.color).toBe("rgb(255, 249, 251)");
    expect(b.style.color).toBe("rgb(17, 24, 23)");
    expect(mark().style.transform).not.toBe("");
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("78px");
  });

  it("oklch forms with percentages, degrees and alpha still glide", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const COLORS: GlideColors = {
      knockout: "oklch(96% 0.01 190deg / 0.5)",
      spoken: "oklch(0.5 0.01 190)",
      plain: "oklch(0.2 0.01 190)"
    };
    vi.spyOn(window, "getComputedStyle").mockImplementation((el: Element) => {
      const value = el instanceof HTMLElement ? el.style.color : "";
      // SAFETY: the glide reads only the color property off the declaration.
      return { color: value } as CSSStyleDeclaration;
    });
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    // Alpha is dropped like the rgb() branch does; the blend targets are the
    // opaque sRGB conversions (canvas ground truth on the same engine).
    expect(a.style.color).toBe("rgb(235, 244, 243)");
    expect(b.style.color).toBe("rgb(17, 24, 23)");
    expect(mark().style.transform).not.toBe("");
  });

  it("a late first frame anchors the glide instead of snapping", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    // A busy main window delivers the first rendered frame late; the glide
    // must still start from its beginning, not land at the end position.
    await pump(130);
    expect(mark().style.transform).toBe("translate(-40px,0px)");
    expect(a.style.color).toBe("rgb(255, 255, 255)");
    expect(b.style.color).toBe("rgb(40, 50, 60)");
  });

  it("late frames stretch the glide instead of collapsing it", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    // Each rendered frame advances at most one cap of the glide clock: three
    // late frames stretch 120 ms over ~400 ms of wall time and still settle.
    await pump(0);
    await pump(130);
    expect(a.style.color).not.toBe("");
    await pump(130);
    expect(b.style.color).not.toBe("");
    await pump(130);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("78px");
  });

  it("painting the same word mid-glide does not restart the glide", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.paint(b);
    await pump(0);
    await pump(60);
    const midFlight = mark().style.transform;
    expect(midFlight).not.toBe("translate(-40px,0px)");
    // The caption clock repeats the same word every 25 ms; the re-paint is
    // synchronous (reposition early-returns) and must not restart the glide.
    glide.paint(b);
    expect(mark().style.transform).toBe(midFlight);
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("78px");
  });

  it("a repeated word only re-places a moved pill", () => {
    const { host, b, mark } = fixture();
    setRects(b, [rect(80, 20, 20, 12)]);
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(b);
    // The surface moved the word: the same paint call re-places the pill.
    vi.restoreAllMocks();
    setRects(b, [rect(200, 40, 20, 12)]);
    glide.paint(b);
    expect(mark().style.left).toBe("198px");
    expect(mark().style.top).toBe("39px");
  });

  it("an unmeasurable word settles without marks", () => {
    const { host, b } = fixture();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(b);
    expect(host.querySelectorAll(".caption-glide-mark")).toHaveLength(0);
    expect(host.style.getPropertyValue("--caption-word-bg")).toBe("transparent");
    expect(b.style.color).toBe("");
  });

  it("clear drops the paint but keeps the layer", () => {
    const { host, a } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.clear();
    expect(host.querySelectorAll(".caption-glide-mark")).toHaveLength(0);
    expect(host.querySelector(".caption-glide-layer")).not.toBeNull();
    expect(host.style.getPropertyValue("--caption-word-bg")).toBe("transparent");
  });

  it("release strips the layer, the marks, and the host property", () => {
    const { host, a } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    glide.release();
    expect(host.querySelector(".caption-glide-layer")).toBeNull();
    expect(host.style.getPropertyValue("--caption-word-bg")).toBe("");
  });

  it("a gap between words keeps the glide and holds the knockout", async () => {
    const { host, a, b, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    setRects(b, [rect(80, 20, 20, 12)]);
    const pump = manualRaf();
    const glide = new CaptionGlider(host, true, COLORS);
    glide.paint(a);
    // The caption clock lands between word timings: paint(null) must hold
    // the pill and the knockout color, not tear the glide state down.
    glide.paint(null);
    expect(a.style.color).toBe("rgb(255, 255, 255)");
    expect(mark().style.left).toBe("38px");
    glide.paint(b);
    expect(mark().style.transform).not.toBe("");
    expect(a.style.color).toBe("rgb(255, 255, 255)");
    expect(b.style.color).toBe("rgb(40, 50, 60)");
    // A gap tick mid-glide must not cancel the tween either.
    glide.paint(null);
    for (let i = 0; i < 10; i += 1) await pump(30);
    expect(a.style.color).toBe("");
    expect(b.style.color).toBe("");
    expect(mark().style.transform).toBe("");
    expect(mark().style.left).toBe("78px");
  });

  it("a new resetKey clears the held paint", () => {
    const { host, a, mark } = fixture();
    setRects(a, [rect(40, 20, 30, 12)]);
    const words = [{ text: "one", offset: 0, start: 0, end: 100, phrase: 0 }];
    const action = captionGlide(host, { words, activeIndex: 0, animated: true, resetKey: "r1" });
    action.update?.({ words, activeIndex: -1, animated: true, resetKey: "r1" });
    expect(mark().style.left).toBe("38px");
    // A new reading or fragment (different resetKey) clears before painting.
    action.update?.({ words, activeIndex: 0, animated: true, resetKey: "r2" });
    expect(a.style.color).toBe("");
    expect(mark().style.left).toBe("38px");
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
    expect(host.querySelector(".caption-glide-layer")).toBeNull();
    expect(host.style.getPropertyValue("--caption-word-bg")).toBe("");
    expect(a.style.color).toBe("");
  });
});
