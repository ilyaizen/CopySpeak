/**
 * Animated word captions — KISS version using mix-blend-mode: difference.
 * A single foreground pill with difference blend mode inverts text naturally.
 * No color probing, no background layer, no coverage math.
 */

import type { ActionReturn } from "svelte/action";
import type { CaptionWord } from "#lib/models/captions.js";

export const MARK_TRANSITION_MS = 120;
const MARK_FRAME_CAP_MS = 40;
const MARK_PADDING_X = 4;
const MARK_PADDING_Y = 2;
const MARK_RADIUS_PX = 4;

type GlideTween = {
  raf: number;
  elapsed: number;
  last: number | null;
  mark: HTMLDivElement;
  view: Window;
  target: HTMLElement;
  dx: number;
  dy: number;
  w0: number;
  w1: number;
};

export class CaptionGlider {
  private animated: boolean;
  private paintedEl: HTMLElement | null = null;
  private mark: HTMLDivElement | null = null;
  private tween: GlideTween | null = null;

  constructor(
    private readonly host: HTMLElement,
    animated: boolean
  ) {
    this.animated = animated;
  }

  setAnimated(animated: boolean) {
    this.animated = animated;
  }

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
    const fromLeft = markOffset(this.mark?.style.left);
    const fromTop = markOffset(this.mark?.style.top);
    const fromWidth = markOffset(this.mark?.style.width);
    const settlesInstantly = !this.glides() || previous === null || this.mark === null;
    this.ensureMark(el);
    if (settlesInstantly) this.settle(el);
    else this.startTween(el, { left: fromLeft, top: fromTop, width: fromWidth });
  }

  reposition() {
    if (this.tween !== null || this.paintedEl === null) return;
    this.ensureMark(this.paintedEl);
  }

  clear() {
    this.cancelTween();
    this.removeMark();
    this.paintedEl = null;
  }

  private hold() {
    if (this.tween !== null) return;
    const held = this.paintedEl;
    if (held === null) return;
    this.reposition();
  }

  release() {
    this.cancelTween();
    this.removeMark();
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

  private glides(): boolean {
    const view = this.view();
    return (
      this.animated &&
      this.mark !== null &&
      view !== null &&
      view.requestAnimationFrame !== undefined &&
      !this.reducedMotion()
    );
  }

  private settle(el: HTMLElement) {
    this.cancelTween();
    this.paintedEl = el;
  }

  private rectsOf(el: HTMLElement): DOMRect[] {
    let rects: DOMRect[];
    try {
      rects = Array.from(el.getClientRects());
    } catch {
      rects = [];
    }
    return rects.filter((rect) => rect.width > 0 && rect.height > 0);
  }

  private ensureMark(el: HTMLElement) {
    const rects = this.rectsOf(el);
    if (rects.length === 0) {
      this.removeMark();
      return;
    }
    if (this.mark !== null && this.mark.isConnected) {
      /* Update existing mark to new word's position (target position for tween). */
      const [baseX, baseY] = this.hostOffsets();
      const rect = rects[0];
      this.mark.style.left = `${Math.round(rect.left + baseX - MARK_PADDING_X)}px`;
      this.mark.style.top = `${Math.round(rect.top + baseY - MARK_PADDING_Y)}px`;
      this.mark.style.width = `${Math.round(rect.width + MARK_PADDING_X * 2)}px`;
      this.mark.style.height = `${Math.round(rect.height + MARK_PADDING_Y * 2)}px`;
      return;
    }
    this.removeMark();
    const mark = this.host.ownerDocument.createElement("div");
    mark.className = "caption-glide-mark";
    mark.setAttribute("aria-hidden", "true");
    mark.style.position = "absolute";
    mark.style.borderRadius = `${MARK_RADIUS_PX}px`;
    /* A white fill under difference inverts everything under the pill, so the
       spoken word stays readable on light and dark surfaces alike. */
    mark.style.background = "#fff";
    mark.style.mixBlendMode = "difference";
    mark.style.pointerEvents = "none";
    mark.style.boxSizing = "border-box";
    this.host.appendChild(mark);
    this.mark = mark;
    const [baseX, baseY] = this.hostOffsets();
    const rect = rects[0];
    mark.style.left = `${Math.round(rect.left + baseX - MARK_PADDING_X)}px`;
    mark.style.top = `${Math.round(rect.top + baseY - MARK_PADDING_Y)}px`;
    mark.style.width = `${Math.round(rect.width + MARK_PADDING_X * 2)}px`;
    mark.style.height = `${Math.round(rect.height + MARK_PADDING_Y * 2)}px`;
  }

  private removeMark() {
    this.mark?.remove();
    this.mark = null;
  }

  private hostOffsets(): [number, number] {
    const hostRect = this.host.getBoundingClientRect();
    return [
      -hostRect.left - this.host.clientLeft + this.host.scrollLeft,
      -hostRect.top - this.host.clientTop + this.host.scrollTop
    ];
  }

  private startTween(target: HTMLElement, from: { left: number; top: number; width: number }) {
    this.cancelTween();
    this.paintedEl = target;
    const view = this.view();
    const mark = this.mark;
    const toLeft = markOffset(mark?.style.left);
    const toTop = markOffset(mark?.style.top);
    const toWidth = markOffset(mark?.style.width);
    if (
      view === null ||
      view.requestAnimationFrame === undefined ||
      mark === null ||
      Number.isNaN(toLeft) ||
      Number.isNaN(toTop) ||
      Number.isNaN(toWidth) ||
      Number.isNaN(from.left) ||
      Number.isNaN(from.top) ||
      Number.isNaN(from.width)
    ) {
      this.settle(target);
      return;
    }
    const dx = from.left - toLeft;
    const dy = from.top - toTop;
    const w0 = from.width;
    const w1 = toWidth;
    const tween: GlideTween = {
      raf: 0,
      elapsed: 0,
      last: null,
      mark,
      view,
      target,
      dx,
      dy,
      w0,
      w1
    };
    mark.style.transform = `translate(${dx}px,${dy}px)`;
    mark.style.width = `${w0}px`;
    const frame = () => {
      if (this.tween !== tween) return;
      if (!tween.target.isConnected) {
        this.cancelTween();
        return;
      }
      const now = view.performance.now();
      const step = tween.last === null ? 0 : Math.min(now - tween.last, MARK_FRAME_CAP_MS);
      tween.last = now;
      tween.elapsed = Math.min(tween.elapsed + step, MARK_TRANSITION_MS);
      const t = tween.elapsed / MARK_TRANSITION_MS;
      const p = easeGlide(t);
      mark.style.transform = `translate(${dx * (1 - p)}px,${dy * (1 - p)}px)`;
      mark.style.width = `${w0 + (w1 - w0) * p}px`;
      if (t < 1) {
        tween.raf = view.requestAnimationFrame(frame);
        return;
      }
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
  }
}

function markOffset(value: string | undefined): number {
  if (value === undefined) return NaN;
  const parsed = parseFloat(value);
  return Number.isNaN(parsed) ? NaN : parsed;
}

/** CSS cubic-bezier(x1, y1, x2, y2) as an easing function over [0, 1]. */
function cubicBezier(x1: number, y1: number, x2: number, y2: number): (x: number) => number {
  const axis = (a1: number, a2: number, t: number) =>
    ((1 - 3 * a2 + 3 * a1) * t + (3 * a2 - 6 * a1)) * t * t + 3 * a1 * t;
  return (x) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let low = 0;
    let high = 1;
    let t = x;
    for (let i = 0; i < 20; i += 1) {
      t = (low + high) / 2;
      if (axis(x1, x2, t) < x) low = t;
      else high = t;
    }
    return axis(y1, y2, t);
  };
}

/** Extra swift easing in: most of the travel happens early, then it settles. */
const easeGlide = cubicBezier(0, 0.5, 0, 1);

export interface CaptionGlideParams {
  words: CaptionWord[];
  activeIndex: number;
  animated: boolean;
  resetKey?: string | null;
}

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
