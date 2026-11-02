import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { groundParallax } from "./ground-parallax";

describe("ground parallax", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("tracks the scroll container, clamps drift and cleans up", () => {
    const scroller = document.createElement("main");
    const ground = document.createElement("div");
    Object.defineProperty(scroller, "clientHeight", { value: 400 });
    const motion = new EventTarget();
    Object.defineProperty(motion, "matches", { value: false });
    vi.stubGlobal("matchMedia", () => motion);
    let frame: FrameRequestCallback | undefined;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());

    const cleanup = groundParallax(scroller, ground);
    scroller.scrollTop = 100;
    scroller.dispatchEvent(new Event("scroll"));
    frame?.(0);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("-12.0px");
    scroller.scrollTop = 10000;
    scroller.dispatchEvent(new Event("scroll"));
    frame?.(0);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("-58.0px");
    cleanup();
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("");
  });

  it("keeps the texture still when reduced motion is enabled", () => {
    const scroller = document.createElement("main");
    const ground = document.createElement("div");
    const motion = new EventTarget();
    Object.defineProperty(motion, "matches", { value: true });
    vi.stubGlobal("matchMedia", () => motion);
    const cleanup = groundParallax(scroller, ground);
    scroller.scrollTop = 100;
    scroller.dispatchEvent(new Event("scroll"));
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("");
    cleanup();
  });

  it("responds to motion preference changes and cancels a pending frame", () => {
    const scroller = document.createElement("main");
    const ground = document.createElement("div");
    Object.defineProperty(scroller, "clientHeight", { value: 400 });
    const motion = new EventTarget();
    let reduced = false;
    Object.defineProperty(motion, "matches", { get: () => reduced });
    vi.stubGlobal("matchMedia", () => motion);
    let frame: FrameRequestCallback | undefined;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    });
    const cancel = vi.fn();
    vi.stubGlobal("cancelAnimationFrame", cancel);

    scroller.scrollTop = 100;
    const cleanup = groundParallax(scroller, ground);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("-12.0px");
    reduced = true;
    motion.dispatchEvent(new Event("change"));
    frame?.(0);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("");
    scroller.dispatchEvent(new Event("scroll"));
    cleanup();
    expect(cancel).toHaveBeenCalledWith(1);
  });

  it("detaches the previous listener when the scroller changes", () => {
    const ground = document.createElement("div");
    const first = document.createElement("main");
    const second = document.createElement("main");
    Object.defineProperty(first, "clientHeight", { value: 400 });
    Object.defineProperty(second, "clientHeight", { value: 400 });
    const motion = new EventTarget();
    Object.defineProperty(motion, "matches", { value: false });
    vi.stubGlobal("matchMedia", () => motion);
    let frame: FrameRequestCallback | undefined;
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      frame = callback;
      return 1;
    });
    vi.stubGlobal("requestAnimationFrame", requestFrame);
    vi.stubGlobal("cancelAnimationFrame", vi.fn());

    const cleanupFirst = groundParallax(first, ground);
    first.scrollTop = 100;
    first.dispatchEvent(new Event("scroll"));
    frame?.(0);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("-12.0px");

    cleanupFirst();
    const cleanupSecond = groundParallax(second, ground);
    requestFrame.mockClear();
    first.dispatchEvent(new Event("scroll"));
    expect(requestFrame).not.toHaveBeenCalled();
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("0.0px");
    second.scrollTop = 200;
    second.dispatchEvent(new Event("scroll"));
    frame?.(0);
    expect(ground.style.getPropertyValue("--bg-shift")).toBe("-24.0px");
    cleanupSecond();
  });
});
