import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { syncLenisToLayout } from "../lenis-layout-sync";

// Frames run only when the test calls flush(). Mutations resolve on the next macrotask.
let frames: Map<number, FrameRequestCallback>;
let nextId: number;

function flush() {
  const pending = [...frames.values()];
  frames.clear();
  for (const callback of pending) callback(0);
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function setup() {
  const wrapper = document.createElement("main");
  const content = document.createElement("div");
  wrapper.append(content);
  let scrollHeight = 1000;
  let clientHeight = 400;
  Object.defineProperty(wrapper, "scrollHeight", { get: () => scrollHeight });
  Object.defineProperty(wrapper, "clientHeight", { get: () => clientHeight });

  // Mirrors Lenis: `limit` is a cached value that only `dimensions.resize()` refreshes.
  const lenis = {
    limit: scrollHeight - clientHeight,
    dimensions: {
      resize: vi.fn(() => {
        lenis.limit = wrapper.scrollHeight - wrapper.clientHeight;
      })
    }
  };

  return {
    wrapper,
    content,
    lenis,
    setScrollHeight: (value: number) => (scrollHeight = value)
  };
}

describe("syncLenisToLayout", () => {
  beforeEach(() => {
    frames = new Map();
    nextId = 1;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      const id = nextId++;
      frames.set(id, callback);
      return id;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
    // jsdom has no ResizeObserver. The wrapper's own resize is not under test here.
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      }
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("refreshes a stale limit after a section grows, then stops once settled", async () => {
    const { wrapper, content, lenis, setScrollHeight } = setup();
    const cleanup = syncLenisToLayout(lenis, wrapper, content);

    setScrollHeight(1400);
    content.append(document.createElement("section"));
    await settle();
    flush();

    expect(lenis.dimensions.resize).toHaveBeenCalledTimes(1);
    expect(lenis.limit).toBe(1000);

    // Two quiet frames after the change end the loop.
    flush();
    flush();
    expect(frames.size).toBe(0);
    cleanup();
  });

  it("does not re-measure when the limit already matches the layout", async () => {
    const { wrapper, content, lenis } = setup();
    const cleanup = syncLenisToLayout(lenis, wrapper, content);

    flush();
    flush();
    flush();
    expect(lenis.dimensions.resize).not.toHaveBeenCalled();
    cleanup();
  });

  it("keeps pumping while a finite animation runs, and stops when it ends", () => {
    const { wrapper, content, lenis } = setup();
    let running = true;
    Object.defineProperty(content, "getAnimations", {
      value: () => (running ? [{ effect: { getComputedTiming: () => ({ endTime: 300 }) } }] : [])
    });
    const cleanup = syncLenisToLayout(lenis, wrapper, content);

    for (let i = 0; i < 5; i++) flush();
    expect(frames.size).toBe(1);

    running = false;
    flush();
    flush();
    expect(frames.size).toBe(0);
    cleanup();
  });

  it("ignores infinite animations so spinners do not keep the loop alive", () => {
    const { wrapper, content, lenis } = setup();
    Object.defineProperty(content, "getAnimations", {
      value: () => [{ effect: { getComputedTiming: () => ({ endTime: Infinity }) } }]
    });
    const cleanup = syncLenisToLayout(lenis, wrapper, content);

    // The attach-time measurement counts as activity, then SETTLE_FRAMES quiet frames end the loop.
    flush();
    flush();
    flush();
    expect(frames.size).toBe(0);
    cleanup();
  });

  it("cancels the pending frame and stops observing on cleanup", async () => {
    const { wrapper, content, lenis } = setup();
    const cleanup = syncLenisToLayout(lenis, wrapper, content);
    expect(frames.size).toBe(1);

    cleanup();
    expect(frames.size).toBe(0);

    content.append(document.createElement("div"));
    await settle();
    expect(frames.size).toBe(0);
  });
});
