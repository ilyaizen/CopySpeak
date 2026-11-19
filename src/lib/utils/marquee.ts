// Infinite drag-and-hover marquee action for the provider rows (Settings About and the
// landing page). Pass the viewport element; direction is +1 or -1. Reduced motion stops the
// loop and leaves the row scrollable. The loop pauses while the viewport is offscreen.
export function marquee(viewport: HTMLDivElement, direction: number) {
  const track = viewport.querySelector(".track");
  const row = viewport.querySelector(".provider-list");
  if (!(track instanceof HTMLElement) || !(row instanceof HTMLElement)) {
    throw new Error("Marquee track missing");
  }
  const trackElement = track;
  const rowElement = row;
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  let offset = 0;
  let speed = 0;
  let last = 0;
  let hover = false;
  let focus = false;
  let pointerId: number | null = null;
  let startX = 0;
  let previousX = 0;
  let moved = false;
  let frame = 0;
  let visible = true;
  let width = 0;

  // The row width only changes on resize, so it is cached instead of read per frame.
  function measure() {
    width = rowElement.getBoundingClientRect().width;
  }
  function updatePosition() {
    if (width > 0) offset = ((offset % width) + width) % width;
    trackElement.style.transform = `translateX(${-offset}px)`;
  }
  // Offscreen viewports (the landing providers sit below the fold) must not run a rAF loop.
  function start() {
    if (frame || !visible || motion.matches) return;
    last = 0;
    frame = requestAnimationFrame(animate);
  }
  function stop() {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  }
  function animate(time: number) {
    const elapsed = last ? Math.min(time - last, 100) : 0;
    const target = hover || focus || pointerId !== null ? 0 : direction * 0.03;
    speed += (target - speed) * (1 - Math.exp(-elapsed / 250));
    if (elapsed) {
      offset += speed * elapsed;
      updatePosition();
    }
    last = time;
    frame = requestAnimationFrame(animate);
  }
  function syncMotion() {
    if (motion.matches) {
      stop();
      pointerId = null;
      offset = 0;
      speed = 0;
      last = 0;
      updatePosition();
    } else {
      viewport.scrollLeft = 0;
      measure();
      start();
    }
  }
  const io =
    "IntersectionObserver" in window
      ? new IntersectionObserver((entries) => {
          visible = entries.some((entry) => entry.isIntersecting);
          if (visible) {
            measure();
            start();
          } else {
            stop();
          }
        })
      : null;
  function pointerDown(event: PointerEvent) {
    if (motion.matches || !event.isPrimary || event.button !== 0) return;
    pointerId = event.pointerId;
    startX = previousX = event.clientX;
    moved = false;
  }
  function pointerMove(event: PointerEvent) {
    if (event.pointerId !== pointerId) return;
    if (!moved && Math.abs(event.clientX - startX) >= 5) {
      moved = true;
      viewport.setPointerCapture(event.pointerId);
      previousX = startX;
    }
    if (moved) {
      offset -= event.clientX - previousX;
      updatePosition();
    }
    previousX = event.clientX;
  }
  function pointerEnd(event: PointerEvent) {
    if (event.pointerId === pointerId) pointerId = null;
  }
  function focusIn(event: FocusEvent) {
    focus = true;
    if (motion.matches || !(event.target instanceof HTMLAnchorElement)) return;
    viewport.scrollLeft = 0;
    const bounds = viewport.getBoundingClientRect();
    const link = event.target.getBoundingClientRect();
    if (link.left < bounds.left + 12) offset += link.left - bounds.left - 12;
    if (link.right > bounds.right - 12) offset += link.right - bounds.right + 12;
    updatePosition();
  }
  function focusOut(event: FocusEvent) {
    if (!(event.relatedTarget instanceof Node) || !viewport.contains(event.relatedTarget))
      focus = false;
  }
  const enter = () => (hover = true);
  const leave = () => (hover = false);
  viewport.addEventListener("pointerenter", enter);
  viewport.addEventListener("pointerleave", leave);
  viewport.addEventListener("pointerdown", pointerDown);
  viewport.addEventListener("pointermove", pointerMove);
  viewport.addEventListener("pointerup", pointerEnd);
  viewport.addEventListener("pointercancel", pointerEnd);
  viewport.addEventListener("lostpointercapture", pointerEnd);
  viewport.addEventListener("focusin", focusIn);
  viewport.addEventListener("focusout", focusOut);
  window.addEventListener("resize", measure);
  io?.observe(viewport);
  motion.addEventListener("change", syncMotion);
  syncMotion();
  return {
    destroy() {
      stop();
      io?.disconnect();
      window.removeEventListener("resize", measure);
      motion.removeEventListener("change", syncMotion);
      viewport.removeEventListener("pointerenter", enter);
      viewport.removeEventListener("pointerleave", leave);
      viewport.removeEventListener("pointerdown", pointerDown);
      viewport.removeEventListener("pointermove", pointerMove);
      viewport.removeEventListener("pointerup", pointerEnd);
      viewport.removeEventListener("pointercancel", pointerEnd);
      viewport.removeEventListener("lostpointercapture", pointerEnd);
      viewport.removeEventListener("focusin", focusIn);
      viewport.removeEventListener("focusout", focusOut);
    }
  };
}
