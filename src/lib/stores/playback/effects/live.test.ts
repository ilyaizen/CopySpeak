import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import type { EffectId } from "#lib/types";
import { createLiveEffect } from "./live";

/** An AudioParam the fake graph can connect into. */
interface FakeParam {
  value: number;
}

interface FakeNode {
  kind: string;
  connections: Array<FakeNode | FakeParam>;
  connect(target: FakeNode | FakeParam): void;
  disconnect(): void;
  gain?: { value: number };
  type?: string;
  frequency?: { value: number };
  Q?: { value: number };
  curve?: Float32Array | null;
  oversample?: string;
  threshold?: { value: number };
  knee?: { value: number };
  ratio?: { value: number };
  attack?: { value: number };
  release?: { value: number };
}

interface FakeSource {
  start: () => void;
  stop: () => void;
}

/** Stub the Web Audio API with a recorded node graph; samples are optional. */
function createHarness(withSamples = false) {
  const nodes: FakeNode[] = [];
  const oscillators: FakeSource[] = [];
  const sources: FakeSource[] = [];
  const node = (kind: string): FakeNode => {
    const created: FakeNode = {
      kind,
      connections: [],
      connect(target) {
        created.connections.push(target);
      },
      disconnect() {
        created.connections.length = 0;
      }
    };
    nodes.push(created);
    return created;
  };
  const param = () => ({ value: 0 });
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      currentTime = 0;
      destination = node("destination");
      decodeAudioData = vi.fn(async () => ({ duration: 0.1 }));
      createGain = () => Object.assign(node("gain"), { gain: param() });
      createBiquadFilter = () =>
        Object.assign(node("biquad"), {
          type: "lowpass",
          frequency: param(),
          Q: param(),
          gain: param()
        });
      ["createWaveShaper"] = () =>
        Object.assign(node("waveshaper"), { curve: null, oversample: "none" });
      createDynamicsCompressor = () =>
        Object.assign(node("compressor"), {
          threshold: param(),
          knee: param(),
          ratio: param(),
          attack: param(),
          release: param()
        });
      createOscillator = () => {
        const start = vi.fn();
        const stop = vi.fn();
        const oscillator = Object.assign(node("oscillator"), {
          type: "sine",
          frequency: param(),
          start,
          stop
        });
        oscillators.push({ start, stop });
        return oscillator;
      };
      createBufferSource = () => {
        const start = vi.fn();
        const stop = vi.fn();
        const source = Object.assign(node("source"), {
          buffer: null,
          loop: false,
          start,
          stop,
          onended: null
        });
        sources.push({ start, stop });
        return source;
      };
      resume = vi.fn();
      suspend = vi.fn();
      close = vi.fn();
    }
  );
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      withSamples
        ? { ok: true, arrayBuffer: async () => new ArrayBuffer(0) }
        : { ok: false, arrayBuffer: async () => new ArrayBuffer(0) }
    )
  );
  const ctx = new AudioContext();
  return { ctx, nodes, oscillators, sources };
}

/** Follow recorded connections from a node; used to prove the graph reaches the output. */
function reachableNodes(nodes: FakeNode[], start: FakeNode): FakeNode[] {
  const visited: FakeNode[] = [];
  const queue = [start];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) break;
    visited.push(current);
    for (const target of current.connections) {
      const next = nodes.find((candidate) => candidate === target);
      if (next && !visited.includes(next)) queue.push(next);
    }
  }
  return visited;
}

const BATCH_ONLY: EffectId[] = ["none", "game_boy", "cyberpunk"];

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("createLiveEffect", () => {
  it.each(BATCH_ONLY)("returns no live chain for %s", (effectId) => {
    const { ctx, nodes } = createHarness();
    expect(createLiveEffect(ctx, effectId)).toBeNull();
    // Only the stub destination exists; nothing was built.
    expect(nodes).toHaveLength(1);
  });

  it("ports the walkie-talkie graph from the input into the destination", () => {
    const { ctx, nodes } = createHarness();
    const effect = createLiveEffect(ctx, "walkie_talkie");
    if (effect === null) throw new Error("walkie_talkie must have a live chain");

    const biquads = nodes.filter((node) => node.kind === "biquad");
    expect(
      biquads.map((filter) => [filter.type, filter.frequency?.value, filter.Q?.value])
    ).toEqual([
      ["highpass", 420, 0.6],
      ["lowpass", 2600, 0.8],
      ["peaking", 1200, 1.4],
      ["notch", 1800, 5]
    ]);
    expect(biquads[2].gain?.value).toBe(4);

    const saturator = nodes.find((node) => node.kind === "waveshaper");
    expect(saturator?.oversample).toBe("2x");
    expect(saturator?.curve?.length).toBe(4096);

    const compressor = nodes.find((node) => node.kind === "compressor");
    expect([
      compressor?.threshold?.value,
      compressor?.knee?.value,
      compressor?.ratio?.value,
      compressor?.attack?.value,
      compressor?.release?.value
    ]).toEqual([-28, 14, 2.2, 0.012, 0.18]);

    const radioGain = nodes.find((node) => node.kind === "gain" && node.gain?.value === 0.86);
    const destination = nodes.find((node) => node.kind === "destination");
    if (radioGain === undefined || destination === undefined) {
      throw new Error("the live chain must end in a radio gain feeding the destination");
    }
    expect(radioGain.connections).toContain(destination);

    // input -> highpass -> lowpass -> peaking -> notch -> shaper -> comp -> radioGain -> destination
    const input = nodes.find((node) => node.kind === "gain" && node.connections[0] === biquads[0]);
    if (input === undefined) throw new Error("the input gain must feed the radio EQ");
    const renderedInput: object = effect.input;
    expect(renderedInput).toBe(input);
    expect(reachableNodes(nodes, input)).toContain(destination);
  });

  it("starts, finishes and disposes safely while the samples are not loaded", async () => {
    const { ctx, oscillators, sources } = createHarness();
    const effect = createLiveEffect(ctx, "walkie_talkie");
    if (effect === null) throw new Error("walkie_talkie must have a live chain");

    effect.start();
    effect.finish();
    effect.dispose();
    effect.dispose();

    expect(oscillators[0].start).toHaveBeenCalledOnce();
    expect(oscillators[0].stop).toHaveBeenCalledOnce();
    // The sample load settles after dispose and must not schedule anything.
    await vi.waitFor(() => expect(sources).toHaveLength(0));
  });

  it("plays the head click and static bed once late samples arrive", async () => {
    const { ctx, sources, oscillators } = createHarness(true);
    const effect = createLiveEffect(ctx, "walkie_talkie");
    if (effect === null) throw new Error("walkie_talkie must have a live chain");

    // start() beats the fetch/decode round trip; the samples schedule on arrival.
    effect.start();
    await vi.waitFor(() => expect(sources).toHaveLength(2));
    expect(sources[0].start).toHaveBeenCalledOnce();
    expect(sources[1].start).toHaveBeenCalledOnce();

    // finish() stops the static bed, adds the tail click and self-disposes later.
    vi.useFakeTimers();
    effect.finish();
    expect(sources).toHaveLength(3);
    expect(sources[1].stop).toHaveBeenCalledOnce();
    expect(oscillators[0].stop).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(oscillators[0].stop).toHaveBeenCalledOnce();
  });
});
