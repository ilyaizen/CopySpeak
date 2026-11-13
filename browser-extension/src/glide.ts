/**
 * Animated word captions for the companion: a rounded brand pill behind the
 * spoken word that glides from word to word while the word text recolors in
 * lockstep. Ported from the DSH plugin's caption glide (copyspeak-dsh), with
 * the companion's fixed brand colors and no follow scroll.
 *
 * content.ts owns one instance per reading. Each state frame paints its word
 * span (captured text offsets) through paint(); the pill draws behind the
 * page's own text, so the instance only reads layout and never scrolls.
 *
 * The pill lives in a layer appended to the reading's scope element (the
 * anchor root). isolation on the host creates a stacking context, so the
 * layer's negative z-index stays above the page background but behind the
 * text. During a glide one rAF loop tweens the pill position and width and
 * blends both words' text colors inside coverage windows computed from the
 * two word boxes, so neither word darkens over bare background mid-glide.
 * The animated flag decides glide vs jump; reduced motion always jumps.
 */

export const GLIDE_STYLE = [
  /* The spoken word wears the knockout white. With the pill live the
     background goes transparent (the scope carries --copyspeak-word-bg), so
     the rounded pill shows behind the text. Without the pill the rectangle
     paints exactly as before. */
  "::highlight(copyspeak-word){color:#fff;background-color:var(--copyspeak-word-bg,#1672de)}",
  /* Glide-only names: the outgoing and incoming word during one glide. The
     custom properties default to the plain text color, so an unmanaged gap
     (cancel, release) paints plain text instead of white-on-white. */
  "::highlight(copyspeak-word-out){color:var(--copyspeak-out-color,currentColor)}",
  "::highlight(copyspeak-word-in){color:var(--copyspeak-in-color,currentColor)}",
  /* The rounded pill: a positioned layer inside the scope's own stacking
     context. Negative z-index keeps it behind the text. */
  "[data-copyspeak-mark-host]{isolation:isolate}",
  ".copyspeak-markLayer{position:absolute;top:0;left:0;width:0;height:0;z-index:-1;pointer-events:none}",
  ".copyspeak-mark{position:absolute;border-radius:4px;background-color:#1672de;pointer-events:none}"
].join("");

/* The pill's glide duration; the recolor must land when the pill arrives,
   not when the word becomes active, so the tween owns both. */
const MARK_TRANSITION_MS = 120;
/* One rendered frame may advance the glide clock by at most this much: the
   state frame's task can end long before the first rAF, and a wall clock
   stamped at paint time snaps the pill across every late frame. */
const MARK_FRAME_CAP_MS = 40;
const MARK_PADDING_X = 2;
const MARK_PADDING_Y = 1;
/* Knockout text inside the pill: the companion's fixed highlight colors. */
const KNOCKOUT = { r: 255, g: 255, b: 255 };

type MarkRect = { left: number; top: number; width: number; height: number };
type Rgb = { r: number; g: number; b: number };

type GlideTween = {
  raf: number;
  /* Played glide milliseconds. The clock anchors at the first rendered frame
     and caps each frame's step, so late frames stretch the glide, never skip
     it. */
  elapsed: number;
  last: number | null;
  mark: HTMLDivElement;
  scope: HTMLElement;
  range: Range;
  key: string;
  dx: number;
  dy: number;
  w0: number;
  w1: number;
  hadOut: boolean;
  outRects: MarkRect[] | null;
  inRects: MarkRect[];
  plain: Rgb;
};

export class CaptionGlide {
  private animated: boolean;
  private paintedKey: string | null = null;
  private paintedRange: Range | null = null;
  private markHost: HTMLElement | null = null;
  private markLayer: HTMLDivElement | null = null;
  private markRects: HTMLDivElement[] = [];
  private markContaining: Element | null = null;
  private tween: GlideTween | null = null;
  private colorProbe: HTMLSpanElement | null = null;

  constructor(
    private readonly doc: Document,
    animated: boolean
  ) {
    this.animated = animated;
  }

  /* The page's highlight registry, or null where the Custom Highlight API is
     missing; the caller refuses such readings before any paint. */
  private registry(): Map<string, Highlight> | null {
    const view = this.doc.defaultView;
    if (view === null) return null;
    const css = view.CSS;
    return css.highlights !== undefined ? css.highlights : null;
  }

  private highlightCtor(): typeof Highlight | null {
    const view = this.doc.defaultView;
    return view === null ? null : view.Highlight;
  }

  /* The scope must take the pill's inline custom properties, so non-HTML
     scopes get the rectangle fallback instead of a pill. */
  private htmlElement(scope: Element): HTMLElement | null {
    const view = this.doc.defaultView;
    if (view === null) return null;
    return scope instanceof view.HTMLElement ? scope : null;
  }

  private reducedMotion(): boolean {
    const view = this.doc.defaultView;
    if (view === null || view.matchMedia === undefined) return false;
    return view.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  /* Whether the pill animates toward the next word: the switch is on and the
     layer is live; reduced motion disables the glide entirely. */
  private glides(): boolean {
    return (
      this.animated &&
      this.markLayer !== null &&
      this.markLayer.hasAttribute("data-animate") &&
      !this.reducedMotion()
    );
  }

  setAnimated(animated: boolean) {
    this.animated = animated;
    this.syncAnimation();
  }

  /* Keep the live layer in step when the switch flips mid-reading. */
  private syncAnimation() {
    if (this.markLayer === null) return;
    if (this.animated) this.markLayer.setAttribute("data-animate", "");
    else this.markLayer.removeAttribute("data-animate");
  }

  /* Paint one spoken word span. The key names the word (its captured text
     offsets): a repeated frame only re-places a lost pill. */
  paint(range: Range, scope: Element, key: string) {
    if (this.registry() === null) return;
    if (key === this.paintedKey) {
      this.reposition();
      return;
    }
    const previous = this.paintedRange;
    const glider = this.markRects[0] ?? null;
    const fromLeft = markOffset(glider?.style.left);
    const fromTop = markOffset(glider?.style.top);
    /* Read the width before placeMarks overwrites the pill geometry with the
       NEW word's rects: the tween needs the OLD width as its start. */
    const fromWidth = markOffset(glider?.style.width);
    const settlesInstantly = !this.glides() || previous === null || this.markRects.length === 0;
    this.placeMarks(this.wordRects(range), this.htmlElement(scope));
    if (settlesInstantly) this.settle(range, key);
    else this.startTween(previous, range, key, { left: fromLeft, top: fromTop, width: fromWidth });
  }

  /* Re-place the pill after a page reflow. A running tween owns the geometry
     until it settles, so repositioning waits for it. */
  reposition() {
    if (this.tween !== null || this.paintedRange === null) return;
    this.placeMarks(this.wordRects(this.paintedRange), this.markHost);
  }

  /* Drop the painted word but keep the layer: the next word repaints it. */
  clearWord() {
    const registry = this.registry();
    if (registry !== null) {
      registry.delete("copyspeak-word");
      registry.delete("copyspeak-word-out");
      registry.delete("copyspeak-word-in");
    }
    this.cancelTween();
    this.clearMarks();
    this.paintedKey = null;
    this.paintedRange = null;
  }

  /* Strip everything the reading added to the page, including the layer and
     the host attribute: the stacking context must not outlive the reading. */
  release() {
    this.cancelTween();
    this.clearMarks();
    if (this.markLayer !== null) this.markLayer.remove();
    if (this.markHost !== null) {
      this.markHost.removeAttribute("data-copyspeak-mark-host");
      this.markHost.style.removeProperty("--copyspeak-word-bg");
    }
    this.colorProbe?.remove();
    this.colorProbe = null;
    this.markHost = null;
    this.markLayer = null;
    this.markContaining = null;
    this.paintedKey = null;
    this.paintedRange = null;
  }

  /* Set exactly one settled highlight name, replacing whatever was painted. */
  private settle(range: Range, key: string) {
    const registry = this.registry();
    const HighlightCtor = this.highlightCtor();
    if (registry === null || HighlightCtor === null) return;
    this.cancelTween();
    registry.delete("copyspeak-word");
    registry.delete("copyspeak-word-out");
    registry.delete("copyspeak-word-in");
    registry.set("copyspeak-word", new HighlightCtor(range));
    this.paintedKey = key;
    this.paintedRange = range;
  }

  /* Range.getClientRects is missing in test DOMs and can throw on a stale
     node, so a thrown call reads as "no layout rect". */
  private wordRects(range: Range): DOMRect[] {
    let rects: DOMRect[];
    try {
      rects = Array.from(range.getClientRects());
    } catch {
      rects = [];
    }
    return rects.filter((rect) => rect.width > 0 && rect.height > 0);
  }

  /* Paint one rounded pill per line of the spoken word, positioned inside
     the layer. It travels with the page when it scrolls, so no scroll
     listener is needed. */
  private placeMarks(rects: DOMRect[], host: HTMLElement | null) {
    const layer = host === null ? null : this.ensureLayer(host);
    if (layer === null) return;
    const view = this.doc.defaultView;
    const block = this.markContainingBlock();
    const blockRect = block === null ? null : block.getBoundingClientRect();
    /* Absolute offsets count from the containing block's padding box, or
       from the document when no ancestor is positioned. */
    let baseX = 0;
    let baseY = 0;
    if (block !== null && blockRect !== null) {
      baseX = -blockRect.left - block.clientLeft;
      baseY = -blockRect.top - block.clientTop;
    } else if (view !== null) {
      baseX = view.scrollX;
      baseY = view.scrollY;
    }
    while (this.markRects.length < rects.length) {
      const mark = this.doc.createElement("div");
      mark.className = "copyspeak-mark";
      mark.setAttribute("aria-hidden", "true");
      layer.appendChild(mark);
      this.markRects.push(mark);
    }
    while (this.markRects.length > rects.length) {
      const mark = this.markRects.pop();
      mark?.remove();
    }
    for (let index = 0; index < rects.length; index += 1) {
      const rect = rects[index];
      const mark = this.markRects[index];
      if (rect === undefined || mark === undefined) continue;
      mark.style.transform = "";
      mark.style.left = `${Math.round(rect.left + baseX - MARK_PADDING_X)}px`;
      mark.style.top = `${Math.round(rect.top + baseY - MARK_PADDING_Y)}px`;
      mark.style.width = `${Math.round(rect.width + MARK_PADDING_X * 2)}px`;
      mark.style.height = `${Math.round(rect.height + MARK_PADDING_Y * 2)}px`;
    }
  }

  private clearMarks() {
    for (const mark of this.markRects) mark.remove();
    this.markRects.length = 0;
  }

  /* The rounded pill paints inside the scope's own stacking context. A page
     re-render can drop the layer, so this re-creates it on demand. */
  private ensureLayer(host: HTMLElement): HTMLDivElement | null {
    if (this.markHost === host && this.markLayer !== null && this.markLayer.isConnected) {
      return this.markLayer;
    }
    if (this.markHost !== null) this.release();
    host.setAttribute("data-copyspeak-mark-host", "");
    const layer = this.doc.createElement("div");
    layer.className = "copyspeak-markLayer";
    layer.setAttribute("aria-hidden", "true");
    host.appendChild(layer);
    this.markHost = host;
    this.markLayer = layer;
    /* The pill replaces the rectangular highlight while the layer lives. */
    host.style.setProperty("--copyspeak-word-bg", "transparent");
    this.syncAnimation();
    return layer;
  }

  /* The positioned ancestor the marks sit in. */
  private markContainingBlock(): Element | null {
    const cached = this.markContaining;
    const host = this.markHost;
    if (cached !== null && cached.isConnected && host !== null && cached.contains(host)) {
      return cached;
    }
    let node = host === null ? null : host.parentElement;
    while (node !== null) {
      const view = this.doc.defaultView;
      if (view === null) return null;
      const position = view.getComputedStyle(node).position;
      if (position !== undefined && position !== "static") {
        this.markContaining = node;
        return node;
      }
      node = node.parentElement;
    }
    this.markContaining = null;
    return null;
  }

  /* Resolve one CSS color value to concrete rgb components through a hidden
     probe inside the scope, so inherited colors and keywords all arrive as
     numbers a tween can blend. */
  private resolveColor(value: string): Rgb | null {
    const scope = this.markHost;
    const view = this.doc.defaultView;
    if (scope === null || view === null || !scope.isConnected) return null;
    if (this.colorProbe === null) {
      const probe = this.doc.createElement("span");
      probe.setAttribute("aria-hidden", "true");
      probe.style.position = "absolute";
      probe.style.visibility = "hidden";
      this.colorProbe = probe;
    }
    const probe = this.colorProbe;
    probe.remove();
    scope.appendChild(probe);
    probe.style.color = value;
    const match = view.getComputedStyle(probe).color.match(/rgba?\(([^)]+)\)/);
    const rgba = match === null ? undefined : match[1];
    if (rgba === undefined) return null;
    const parts = rgba
      .split(/[,/\s]+/)
      .map(parseFloat)
      .filter(Number.isFinite);
    const [r, g, b] = parts;
    if (r === undefined || g === undefined || b === undefined) return null;
    return { r: r, g: g, b: b };
  }

  /* Glide the pill and blend the two words' text colors across one rAF
     clock, so the colors land exactly when the pill covers each word. Each
     blend runs inside a coverage window computed from the two word boxes:
     the outgoing word stays knocked out until the pill slides off it, and
     the incoming word stays plain until the pill reaches it, so neither word
     darkens over bare background mid-glide. */
  private startTween(
    previous: Range,
    range: Range,
    key: string,
    from: { left: number; top: number; width: number }
  ) {
    this.cancelTween();
    const view = this.doc.defaultView;
    const glider = this.markRects[0] ?? null;
    const plain = this.resolveColor("currentColor");
    const toLeft = markOffset(glider?.style.left);
    const toTop = markOffset(glider?.style.top);
    const toWidth = markOffset(glider?.style.width);
    const scope = this.markHost;
    if (
      view === null ||
      plain === null ||
      scope === null ||
      glider === null ||
      Number.isNaN(toLeft) ||
      Number.isNaN(toTop) ||
      Number.isNaN(toWidth) ||
      Number.isNaN(from.left) ||
      Number.isNaN(from.top)
    ) {
      /* Unresolvable colors or geometry: keep the instant, correct swap. */
      this.settle(range, key);
      return;
    }
    const block = this.markContainingBlock();
    const blockRect = block === null ? null : block.getBoundingClientRect();
    let baseX = 0;
    let baseY = 0;
    if (block !== null && blockRect !== null) {
      baseX = -blockRect.left - block.clientLeft;
      baseY = -blockRect.top - block.clientTop;
    } else {
      baseX = view.scrollX;
      baseY = view.scrollY;
    }
    const layerRects = (wordRange: Range): MarkRect[] =>
      this.wordRects(wordRange).map((rect) => ({
        left: rect.left + baseX,
        top: rect.top + baseY,
        width: rect.width,
        height: rect.height
      }));
    const hadOut = previous.startContainer.isConnected;
    const tween: GlideTween = {
      raf: 0,
      elapsed: 0,
      last: null,
      mark: glider,
      scope: scope,
      range: range,
      key: key,
      dx: from.left - toLeft,
      dy: from.top - toTop,
      /* The pill keeps the OLD word's width at the start of the glide and
         eases to the new width on the same clock, so the outgoing word's
         tail never loses its coverage mid-glide. */
      w0: from.width,
      w1: toWidth,
      hadOut: hadOut,
      outRects: hadOut ? layerRects(previous) : null,
      inRects: layerRects(range),
      plain: plain
    };
    const registry = this.registry();
    const HighlightCtor = this.highlightCtor();
    if (registry === null || HighlightCtor === null) {
      this.settle(range, key);
      return;
    }
    /* Paint the first frame's state before the first rAF, so the pill and
       the colors never show their settled neighbors for a frame. */
    tween.mark.style.transform = `translate(${tween.dx}px,${tween.dy}px)`;
    tween.mark.style.width = `${tween.w0}px`;
    if (hadOut) {
      scope.style.setProperty("--copyspeak-out-color", blendColor(KNOCKOUT, plain, 0));
    }
    scope.style.setProperty("--copyspeak-in-color", blendColor(plain, KNOCKOUT, 0));
    registry.delete("copyspeak-word");
    if (hadOut) {
      registry.set("copyspeak-word-out", new HighlightCtor(previous));
    }
    registry.set("copyspeak-word-in", new HighlightCtor(range));
    /* Fraction of the given word rects the pill currently covers. */
    const coverage = (rects: MarkRect[] | null, pill: MarkRect): number => {
      if (rects === null) return 0;
      const pillRight = pill.left + pill.width;
      const pillBottom = pill.top + pill.height;
      let covered = 0;
      let total = 0;
      rects.forEach((rect) => {
        const across = Math.min(pillRight, rect.left + rect.width) - Math.max(pill.left, rect.left);
        const down = Math.min(pillBottom, rect.top + rect.height) - Math.max(pill.top, rect.top);
        total += rect.width * rect.height;
        if (across > 0 && down > 0) covered += across * down;
      });
      return total === 0 ? 0 : Math.min(Math.max(covered / total, 0), 1);
    };
    const frame = () => {
      if (this.tween !== tween) return;
      if (!tween.range.startContainer.isConnected) {
        /* The page re-rendered mid-glide: drop the tween and both names.
           The next paint restores the highlight. */
        this.cancelTween();
        return;
      }
      const now = view.performance.now();
      const step = tween.last === null ? 0 : Math.min(now - tween.last, MARK_FRAME_CAP_MS);
      tween.last = now;
      tween.elapsed = Math.min(tween.elapsed + step, MARK_TRANSITION_MS);
      const t = tween.elapsed / MARK_TRANSITION_MS;
      const p = easeGlide(t);
      /* One eased progress drives the pill position, width, and both
         blends, so they cannot drift apart. */
      const pill: MarkRect = {
        left: toLeft + tween.dx * (1 - p),
        top: toTop + tween.dy * (1 - p),
        width: tween.w0 + (tween.w1 - tween.w0) * p,
        height: parseFloat(tween.mark.style.height) || 0
      };
      tween.mark.style.transform = `translate(${tween.dx * (1 - p)}px,${tween.dy * (1 - p)}px)`;
      tween.mark.style.width = `${pill.width}px`;
      if (tween.hadOut) {
        /* The outgoing word recovers its plain color as coverage drops. */
        const outBlend = 1 - coverage(tween.outRects, pill);
        tween.scope.style.setProperty(
          "--copyspeak-out-color",
          blendColor(KNOCKOUT, tween.plain, outBlend)
        );
      }
      /* The incoming word darkens only as the pill slides over it. */
      const inBlend = coverage(tween.inRects, pill);
      tween.scope.style.setProperty(
        "--copyspeak-in-color",
        blendColor(tween.plain, KNOCKOUT, inBlend)
      );
      if (t < 1) {
        tween.raf = view.requestAnimationFrame(frame);
        return;
      }
      /* Settled: swap to the single settled highlight name. */
      this.settle(tween.range, tween.key);
    };
    this.tween = tween;
    tween.raf = view.requestAnimationFrame(frame);
  }

  private cancelTween() {
    const tween = this.tween;
    if (tween === null) return;
    this.tween = null;
    const view = this.doc.defaultView;
    if (view !== null) view.cancelAnimationFrame(tween.raf);
    if (tween.mark.isConnected) tween.mark.style.transform = "";
    tween.scope.style.removeProperty("--copyspeak-out-color");
    tween.scope.style.removeProperty("--copyspeak-in-color");
    const registry = this.registry();
    if (registry !== null) {
      registry.delete("copyspeak-word-out");
      registry.delete("copyspeak-word-in");
    }
  }
}

function markOffset(value: string | undefined): number {
  if (value === undefined) return NaN;
  const parsed = parseFloat(value);
  return Number.isNaN(parsed) ? NaN : parsed;
}

function blendColor(from: Rgb, to: Rgb, t: number): string {
  const channel = (a: number, b: number) => Math.round(a + (b - a) * t);
  return `rgb(${channel(from.r, to.r)},${channel(from.g, to.g)},${channel(from.b, to.b)})`;
}

/* The single easing curve for the glide: one rAF loop uses it for the pill
   position, the width, and both color blends, so they cannot drift apart. */
function easeGlide(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
}
