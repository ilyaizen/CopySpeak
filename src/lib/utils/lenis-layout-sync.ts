// Lenis caches its scroll limit. It re-measures only when its observed boxes resize.
// The app content box has a fixed height (`h-full`), so a collapsing section changes
// scrollHeight without resizing any observed box. Lenis then clamps smooth scrolls to a
// stale limit. This module re-measures after DOM, animation, image and font activity,
// and stops once the layout stays still for SETTLE_FRAMES frames.

const SETTLE_FRAMES = 2;

/** The part of a Lenis instance this module reads and refreshes. */
export interface LenisLayout {
  readonly limit: number;
  readonly dimensions: { resize(): void };
}

/** Keeps `lenis.limit` equal to the live scroll range of `wrapper`. Returns the cleanup. */
export function syncLenisToLayout(lenis: LenisLayout, wrapper: HTMLElement, content: HTMLElement) {
  let frame = 0;
  let quietFrames = 0;
  let activity = false;

  const refreshIfStale = (): boolean => {
    if (wrapper.scrollHeight - wrapper.clientHeight === lenis.limit) return false;
    // Unlike `lenis.resize()`, this keeps the in-flight scroll target intact.
    lenis.dimensions.resize();
    return true;
  };

  // Finite CSS/Web Animations only. Infinite spinners would keep the loop alive forever.
  const animating = () =>
    content
      .getAnimations?.({ subtree: true })
      .some((a) => a.effect?.getComputedTiming().endTime !== Infinity) ?? false;

  const tick = () => {
    frame = 0;
    const changed = refreshIfStale();
    quietFrames = changed || activity || animating() ? 0 : quietFrames + 1;
    activity = false;
    if (quietFrames < SETTLE_FRAMES) frame = requestAnimationFrame(tick);
  };

  const wake = () => {
    activity = true;
    if (!frame) frame = requestAnimationFrame(tick);
  };

  const mutations = new MutationObserver(wake);
  mutations.observe(content, {
    childList: true,
    subtree: true,
    attributes: true,
    characterData: true
  });

  // Viewport and text reflow: the wrapper box resizes, which Lenis already observes,
  // but Lenis debounces that by 250 ms. Re-measure on the next frame instead.
  const resizes = new ResizeObserver(wake);
  resizes.observe(wrapper);

  // Images and fonts change height without any DOM mutation.
  // `load` does not bubble, so listen in the capture phase.
  wrapper.addEventListener("load", wake, true);
  document.fonts?.addEventListener("loadingdone", wake);

  wake();

  return () => {
    mutations.disconnect();
    resizes.disconnect();
    wrapper.removeEventListener("load", wake, true);
    document.fonts?.removeEventListener("loadingdone", wake);
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  };
}
