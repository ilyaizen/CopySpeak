// Shared by the landing and app content; the background stays still for reduced motion.
export function groundParallax(scroller: HTMLElement, ground: HTMLElement) {
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  let frame = 0;

  const update = () => {
    frame = 0;
    if (motion.matches) {
      ground.style.removeProperty("--bg-shift");
      return;
    }
    const shift = Math.max(0, Math.min(scroller.scrollTop * 0.12, scroller.clientHeight * 0.145));
    ground.style.setProperty("--bg-shift", `${(-shift).toFixed(1)}px`);
  };
  const queue = () => {
    if (!frame) frame = requestAnimationFrame(update);
  };
  scroller.addEventListener("scroll", queue, { passive: true });
  window.addEventListener("resize", queue);
  motion.addEventListener("change", queue);
  update();

  return () => {
    scroller.removeEventListener("scroll", queue);
    window.removeEventListener("resize", queue);
    motion.removeEventListener("change", queue);
    if (frame) cancelAnimationFrame(frame);
    ground.style.removeProperty("--bg-shift");
  };
}
