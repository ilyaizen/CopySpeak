/**
 * Animated word captions for the app's own caption surfaces (Play page, HUD):
 * a rounded pill behind the spoken word that glides from word to word while
 * the word text recolors in lockstep. Ported from the browser companion
 * (browser-extension/src/glide.ts); both port the DSH plugin's glide, so all
 * three share the 120 ms easeInOutQuad tween, the first-rendered-frame clock
 * anchor, the per-frame step cap, and the coverage-window color blends.
 *
 * Differences from the companion version:
 * - The app owns its word spans, so paint() takes the active span element;
 *   there is no Range capture and no CSS Custom Highlight API.
 * - Text colors come from theme tokens (oklch). Chromium serializes computed
 *   tokens space-preserving as oklch(...) (probed on WebView2 157), so the
 *   color probe parses rgb(), rgba(), color(srgb ...) and oklch() forms.
 * - The blend targets are explicit: the outgoing word settles to the spoken
 *   color (muted foreground), the incoming word starts from the plain
 *   foreground. The pill itself keeps var(--primary) through CSS.
 *
 * The captionGlide Svelte action wires a caption container to one glider.
 * The container needs position:relative + isolation (the surfaces set both),
 * so the layer's negative z-index paints above the card background but
 * behind the text. Word spans carry data-word-offset; their active class
 * supplies the knockout color while background: var(--caption-word-bg,
 * <primary>) defers to the pill while the layer is live.
 */

import type { ActionReturn } from "svelte/action";
import type { CaptionWord } from "#lib/models/captions.js";

export const MARK_TRANSITION_MS = 120;
/* One rendered frame may advance the glide clock by at most this much: the
   clock anchors at the first rendered frame and caps each frame's step, so
   late frames stretch the glide, never skip it. */
const MARK_FRAME_CAP_MS = 40;
const MARK_PADDING_X = 2;
const MARK_PADDING_Y = 1;
const MARK_RADIUS_PX = 4;

type MarkRect = { left: number; top: number; width: number; height: number };
type Rgb = { r: number; g: number; b: number };

/** CSS color strings, resolved through a probe inside the caption container. */
export interface GlideColors {
  /** Color of the word text while the pill covers it (the knockout color). */
  knockout: string;
  /** Settled color of the word the pill leaves (already spoken). */
  spoken: string;
  /** Settled color of a word before the pill reaches it. */
  plain: string;
}

export const CAPTION_GLIDE_COLORS: GlideColors = {
  knockout: "var(--primary-foreground)",
  spoken: "var(--muted-foreground)",
  plain: "var(--foreground)"
};

type GlideTween = {
  raf: number;
  /* Played glide milliseconds. Anchored at the first rendered frame. */
  elapsed: number;
  last: number | null;
  mark: HTMLDivElement;
  view: Window;
  target: HTMLElement;
  dx: number;
  dy: number;
  w0: number;
  w1: number;
  hadOut: boolean;
  outEl: HTMLElement | null;
  outRects: MarkRect[] | null;
  inEl: HTMLElement;
  inRects: MarkRect[];
  knockout: Rgb;
  spoken: Rgb;
  plain: Rgb;
};

export class CaptionGlider {
  private animated: boolean;
  private colors: GlideColors;
  private paintedEl: HTMLElement | null = null;
  private layer: HTMLDivElement | null = null;
  private marks: HTMLDivElement[] = [];
  private tween: GlideTween | null = null;
  private probe: HTMLSpanElement | null = null;

  constructor(
    private readonly host: HTMLElement,
    animated: boolean,
    colors?: GlideColors
  ) {
    this.animated = animated;
    this.colors = colors ?? CAPTION_GLIDE_COLORS;
  }

  setAnimated(animated: boolean) {
    this.animated = animated;
  }

  /* Paint the spoken word. A new element glides from the painted one; the
     same element only re-places a moved pill. A null element (the clock sits
     between word timings, or the reading is paused) holds the last paint so
     the next word still glides from it. */
  paint(el: HTMLElement | null) {
    if (el === null) {
      this.hold();
      return;
    }
    if (el === this.paintedEl) {
      this.reposition();
      return;
    }
    const previous = this.paintedEl;
    const glider = this.marks[0] ?? null;
    const fromLeft = markOffset(glider?.style.left);
    const fromTop = markOffset(glider?.style.top);
    /* Read the width before placeMarks overwrites the pill geometry with the
       NEW word's rects: the tween needs the OLD width as its start. */
    const fromWidth = markOffset(glider?.style.width);
    const settlesInstantly = !this.glides() || previous === null || this.marks.length === 0;
    this.placeMarks(this.rectsOf(el));
    if (settlesInstantly) this.settle(el);
    else this.startTween(previous, el, { left: fromLeft, top: fromTop, width: fromWidth });
  }

  /* Re-place the pill after a reflow. A running tween owns the geometry
     until it settles, so repositioning waits for it. */
  reposition() {
    if (this.tween !== null || this.paintedEl === null) return;
    this.placeMarks(this.rectsOf(this.paintedEl));
  }

  /* Drop the paint but keep the layer: the next word repaints it. */
  clear() {
    this.cancelTween();
    if (this.paintedEl !== null) this.paintedEl.style.color = "";
    this.clearMarks();
    this.paintedEl = null;
  }

  /* No active word: hold the last paint instead of tearing it down. The 25 ms
     caption clock lands between word timings on every inter-word gap, and a
     teardown there makes every following word settle instantly (the reported
     no-glide bug). The held word keeps the knockout color over the pill, the
     companion's paused-frame look; a new reading clears via resetKey. */
  private hold() {
    if (this.tween !== null) return;
    const held = this.paintedEl;
    if (held === null) return;
    this.reposition();
    const knockout = this.resolveColor(this.colors.knockout);
    if (knockout !== null && held.isConnected) held.style.color = rgbString(knockout);
  }

  /* Remove everything the glide added to the host. */
  release() {
    this.cancelTween();
    this.clearMarks();
    if (this.layer !== null) this.layer.remove();
    this.host.style.removeProperty("--caption-word-bg");
    if (this.paintedEl !== null) this.paintedEl.style.color = "";
    this.probe?.remove();
    this.probe = null;
    this.layer = null;
    this.paintedEl = null;
  }

  private view(): Window | null {
    return this.host.ownerDocument.defaultView;
  }

  private reducedMotion(): boolean {
    const view = this.view();
    if (view === null || view.matchMedia === undefined) return false;
    return view.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  /* Whether the pill animates toward the next word: the setting is on, the
     layer is live, rAF exists, and motion is not reduced. */
  private glides(): boolean {
    const view = this.view();
    return (
      this.animated &&
      this.layer !== null &&
      view !== null &&
      view.requestAnimationFrame !== undefined &&
      !this.reducedMotion()
    );
  }

  private settle(el: HTMLElement) {
    this.cancelTween();
    /* A held word may carry an inline knockout color; the word leaving the
       painted slot hands its color back to the classes. */
    if (this.paintedEl !== null && this.paintedEl !== el) this.paintedEl.style.color = "";
    this.paintedEl = el;
  } /* Line boxes of the spoken word. A thrown or empty read reads as "no
     layout rect", which settles the paint instead of gliding. */
  private rectsOf(el: HTMLElement): DOMRect[] {
    let rects: DOMRect[];
    try {
      rects = Array.from(el.getClientRects());
    } catch {
      rects = [];
    }
    return rects.filter((rect) => rect.width > 0 && rect.height > 0);
  }

  /* Paint one rounded pill per line of the spoken word, positioned inside
     the host. The layer scrolls with the host's content, so the marks travel
     with the text and no scroll listener is needed. */
  private placeMarks(rects: DOMRect[]) {
    const layer = this.ensureLayer();
    if (layer === null) return;
    const [baseX, baseY] = this.hostOffsets();
    while (this.marks.length < rects.length) {
      const mark = this.host.ownerDocument.createElement("div");
      mark.className = "caption-glide-mark";
      mark.setAttribute("aria-hidden", "true");
      mark.style.position = "absolute";
      mark.style.borderRadius = `${MARK_RADIUS_PX}px`;
      mark.style.backgroundColor = "var(--primary)";
      mark.style.pointerEvents = "none";
      layer.appendChild(mark);
      this.marks.push(mark);
    }
    while (this.marks.length > rects.length) {
      const mark = this.marks.pop();
      mark?.remove();
    }
    for (let index = 0; index < rects.length; index += 1) {
      const rect = rects[index];
      const mark = this.marks[index];
      if (rect === undefined || mark === undefined) continue;
      mark.style.transform = "";
      mark.style.left = `${Math.round(rect.left + baseX - MARK_PADDING_X)}px`;
      mark.style.top = `${Math.round(rect.top + baseY - MARK_PADDING_Y)}px`;
      mark.style.width = `${Math.round(rect.width + MARK_PADDING_X * 2)}px`;
      mark.style.height = `${Math.round(rect.height + MARK_PADDING_Y * 2)}px`;
    }
  }

  private clearMarks() {
    for (const mark of this.marks) mark.remove();
    this.marks.length = 0;
  }

  /* The pill layer inside the host's own stacking context. A re-render can
     drop the layer, so this re-creates it on demand. */
  private ensureLayer(): HTMLDivElement | null {
    if (this.layer !== null && this.layer.isConnected) return this.layer;
    this.clearMarks();
    const layer = this.host.ownerDocument.createElement("div");
    layer.className = "caption-glide-layer";
    layer.setAttribute("aria-hidden", "true");
    layer.style.position = "absolute";
    layer.style.top = "0";
    layer.style.left = "0";
    layer.style.width = "0";
    layer.style.height = "0";
    layer.style.zIndex = "-1";
    layer.style.pointerEvents = "none";
    this.host.appendChild(layer);
    this.layer = layer;
    /* The pill replaces the active word's own background while the layer
       lives; the value falls back to the theme primary without the layer. */
    this.host.style.setProperty("--caption-word-bg", "transparent");
    return layer;
  }

  /* Host-relative content coordinates of a viewport rect: the layer sits at
     the host's padding box and scrolls with its content, so the host's own
     scroll offset must be added on top of the client-box offset. */
  private hostOffsets(): [number, number] {
    const hostRect = this.host.getBoundingClientRect();
    return [
      -hostRect.left - this.host.clientLeft + this.host.scrollLeft,
      -hostRect.top - this.host.clientTop + this.host.scrollTop
    ];
  }

  /* Resolve one CSS color value to concrete rgb components through a hidden
     probe inside the host, so inherited colors, keywords and theme tokens
     all arrive as numbers a tween can blend. */
  private resolveColor(value: string): Rgb | null {
    const view = this.view();
    if (view === null || !this.host.isConnected) return null;
    if (this.probe === null) {
      const probe = this.host.ownerDocument.createElement("span");
      probe.setAttribute("aria-hidden", "true");
      probe.style.position = "absolute";
      probe.style.visibility = "hidden";
      this.probe = probe;
    }
    const probe = this.probe;
    probe.remove();
    this.host.appendChild(probe);
    probe.style.color = value;
    return parseColor(view.getComputedStyle(probe).color);
  }

  /* Glide the pill and blend the two words' text colors across one rAF
     clock, so the colors land exactly when the pill covers each word. Each
     blend runs inside a coverage window computed from the two word boxes:
     the outgoing word stays knocked out until the pill slides off it, and
     the incoming word stays plain until the pill reaches it, so neither word
     darkens over bare background mid-glide. */
  private startTween(
    previous: HTMLElement,
    target: HTMLElement,
    from: { left: number; top: number; width: number }
  ) {
    this.cancelTween();
    /* The target owns the painted slot from tween start: repeated frames for
       the same word re-place instead of restarting the glide. */
    this.paintedEl = target;
    const view = this.view();
    const glider = this.marks[0] ?? null;
    const knockout = this.resolveColor(this.colors.knockout);
    const spoken = this.resolveColor(this.colors.spoken);
    const plain = this.resolveColor(this.colors.plain);
    const toLeft = markOffset(glider?.style.left);
    const toTop = markOffset(glider?.style.top);
    const toWidth = markOffset(glider?.style.width);
    if (
      view === null ||
      view.requestAnimationFrame === undefined ||
      knockout === null ||
      spoken === null ||
      plain === null ||
      glider === null ||
      Number.isNaN(toLeft) ||
      Number.isNaN(toTop) ||
      Number.isNaN(toWidth) ||
      Number.isNaN(from.left) ||
      Number.isNaN(from.top) ||
      Number.isNaN(from.width)
    ) {
      /* Unresolvable colors or geometry: keep the instant, correct swap. */
      this.settle(target);
      return;
    }
    const hadOut = previous.isConnected;
    const [baseX, baseY] = this.hostOffsets();
    const layerRects = (el: HTMLElement): MarkRect[] =>
      this.rectsOf(el).map((rect) => ({
        left: rect.left + baseX,
        top: rect.top + baseY,
        width: rect.width,
        height: rect.height
      }));
    const tween: GlideTween = {
      raf: 0,
      elapsed: 0,
      last: null,
      mark: glider,
      view: view,
      target: target,
      dx: from.left - toLeft,
      dy: from.top - toTop,
      /* The pill keeps the OLD word's width at the start of the glide and
         eases to the new width on the same clock, so the outgoing word's
         tail never loses its coverage mid-glide. */
      w0: from.width,
      w1: toWidth,
      hadOut: hadOut,
      outEl: hadOut ? previous : null,
      outRects: hadOut ? layerRects(previous) : null,
      inEl: target,
      inRects: layerRects(target),
      knockout: knockout,
      spoken: spoken,
      plain: plain
    };
    /* Paint the first frame's state before the first rAF, so the pill and
       the colors never show their settled neighbors for a frame. */
    tween.mark.style.transform = `translate(${tween.dx}px,${tween.dy}px)`;
    tween.mark.style.width = `${tween.w0}px`;
    if (tween.outEl !== null) {
      tween.outEl.style.color = blendColor(knockout, spoken, 0);
    }
    tween.inEl.style.color = blendColor(plain, knockout, 0);
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
      if (!tween.target.isConnected) {
        /* The surface re-rendered mid-glide: drop the tween and the glide
           colors. The next paint restores the word. */
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
      if (tween.hadOut && tween.outEl !== null) {
        /* The outgoing word recovers its spoken color as coverage drops. */
        const outBlend = 1 - coverage(tween.outRects, pill);
        tween.outEl.style.color = blendColor(knockout, tween.spoken, outBlend);
      }
      /* The incoming word darkens only as the pill slides over it. */
      const inBlend = coverage(tween.inRects, pill);
      tween.inEl.style.color = blendColor(tween.plain, knockout, inBlend);
      if (t < 1) {
        tween.raf = view.requestAnimationFrame(frame);
        return;
      }
      /* Settled: the classes render the word; the pill stays over it. */
      this.settle(tween.target);
    };
    this.tween = tween;
    tween.raf = view.requestAnimationFrame(frame);
  }

  private cancelTween() {
    const tween = this.tween;
    if (tween === null) return;
    this.tween = null;
    if (tween.view.cancelAnimationFrame !== undefined) {
      tween.view.cancelAnimationFrame(tween.raf);
    }
    if (tween.mark.isConnected) tween.mark.style.transform = "";
    if (tween.outEl !== null) tween.outEl.style.color = "";
    tween.inEl.style.color = "";
  }
}

function markOffset(value: string | undefined): number {
  if (value === undefined) return NaN;
  const parsed = parseFloat(value);
  return Number.isNaN(parsed) ? NaN : parsed;
}

/* Chromium serializes computed theme tokens space-preserving: oklch tokens
   come back as oklch(...) (probed on WebView2/Edge 157), so they convert to
   sRGB here. The legacy rgb()/rgba()/color(srgb ...) forms stay for tests
   and explicit colors. */
function parseColor(value: string): Rgb | null {
  const rgb = value.match(/rgba?\(([^)]+)\)/);
  if (rgb !== null) {
    const parts = rgb[1]
      .split(/[,/\s]+/)
      .map(parseFloat)
      .filter(Number.isFinite);
    const [r, g, b] = parts;
    if (r === undefined || g === undefined || b === undefined) return null;
    return { r: r, g: g, b: b };
  }
  const srgb = value.match(/color\(\s*srgb\s+([^)]+)\)/);
  if (srgb !== null) {
    const parts = srgb[1]
      .split(/[\s/]+/)
      .map(parseFloat)
      .filter(Number.isFinite);
    const [r, g, b] = parts;
    if (r === undefined || g === undefined || b === undefined) return null;
    const channel = (unit: number) => Math.round(unit * 255);
    return { r: channel(r), g: channel(g), b: channel(b) };
  }
  const oklch = value.match(/oklch\(([^)]+)\)/);
  if (oklch !== null) return oklchToRgb(oklch[1]);
  return null;
}

/* One oklch() to sRGB conversion (CSS Color 4): lightness, chroma and hue
   accept numbers, percentages and a deg-suffixed hue; alpha is dropped like
   the rgb() branch does because the theme tokens are opaque. Out-of-gamut
   channels clamp to the sRGB cube, so the tween always blends numbers. */
function oklchToRgb(args: string): Rgb | null {
  const color = args.split("/")[0] ?? "";
  const numbers = color.match(/-?(?:\d+\.?\d*|\.\d+)(?:%|deg)?/g);
  if (numbers === null) return null;
  const channel = (token: string): number => {
    const number = parseFloat(token);
    return Number.isNaN(number) ? NaN : token.endsWith("%") ? number / 100 : number;
  };
  const [l, chroma, hue] = numbers.map(channel);
  if (l === undefined || chroma === undefined || hue === undefined) return null;
  const rad = (hue * Math.PI) / 180;
  const a = chroma * Math.cos(rad);
  const b = chroma * Math.sin(rad);
  const encode = (linear: number): number => {
    const c = Math.min(Math.max(linear, 0), 1);
    return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
  };
  const l3 = Math.pow(l + 0.3963377774 * a + 0.2158037573 * b, 3);
  const m3 = Math.pow(l - 0.1055613458 * a - 0.0638541728 * b, 3);
  const s3 = Math.pow(l - 0.0894841775 * a - 1.291485548 * b, 3);
  return {
    r: Math.round(encode(4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3) * 255),
    g: Math.round(encode(-1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3) * 255),
    b: Math.round(encode(-0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3) * 255)
  };
}

function rgbString(color: Rgb): string {
  return `rgb(${color.r},${color.g},${color.b})`;
}

function blendColor(from: Rgb, to: Rgb, t: number): string {
  const channel = (a: number, b: number) => Math.round(a + (b - a) * t);
  return rgbString({
    r: channel(from.r, to.r),
    g: channel(from.g, to.g),
    b: channel(from.b, to.b)
  });
}

/* The single easing curve for the glide: one rAF loop uses it for the pill
   position, the width, and both color blends, so they cannot drift apart. */
function easeGlide(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
}

export interface CaptionGlideParams {
  /** The caption words of the audible fragment. */
  words: CaptionWord[];
  /** Index of the spoken word in words; -1 holds the last paint. */
  activeIndex: number;
  /** Saved glide preference; reduced motion still jumps. */
  animated: boolean;
  /** Identity of the spoken text. A change (new reading or fragment) clears
     the held paint before the next word paints. */
  resetKey?: string | null;
}

/**
 * Attach one CaptionGlider to a caption container. The spans must carry
 * data-word-offset; the container must be positioned and isolated.
 */
export function captionGlide(
  host: HTMLElement,
  params: CaptionGlideParams
): ActionReturn<CaptionGlideParams> {
  const view = host.ownerDocument.defaultView;
  const glider = view === null ? null : new CaptionGlider(host, params.animated);
  let resetKey: string | null | undefined = params.resetKey;
  const update = (next: CaptionGlideParams) => {
    if (glider === null) return;
    if (next.resetKey !== resetKey) {
      resetKey = next.resetKey;
      glider.clear();
    }
    glider.setAnimated(next.animated);
    const word = next.activeIndex >= 0 ? next.words[next.activeIndex] : undefined;
    const el =
      word === undefined
        ? null
        : host.querySelector<HTMLElement>(`[data-word-offset="${word.offset}"]`);
    glider.paint(el);
  };
  update(params);
  return {
    update,
    destroy() {
      glider?.release();
    }
  };
}
