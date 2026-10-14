import { describe, expect, it } from "vite-plus/test";
import { crushTail, findOnsets, insertStutters, planStutters } from "./cyberpunk";

const SR = 8000;

// 0.3 s silence, 0.3 s tone, 0.3 s silence, 0.3 s tone.
function twoBursts(): Float32Array {
  const data = new Float32Array(SR * 1.2);
  for (const start of [0.3, 0.9]) {
    for (let i = start * SR; i < (start + 0.3) * SR; i++) data[i] = Math.sin(i * 0.3) * 0.5;
  }
  return data;
}

// 200 tone bursts, one every 0.6 s (0.3 s tone, 0.3 s silence).
function longBursts(): Float32Array {
  const data = new Float32Array(SR * 120);
  for (let b = 0; b < 200; b++) {
    const start = Math.round(b * 0.6 * SR);
    for (let i = start; i < start + 0.3 * SR; i++) data[i] = Math.sin(i * 0.3) * 0.5;
  }
  return data;
}

describe("cyberpunk effect", () => {
  it("finds onsets at burst starts", () => {
    const onsets = findOnsets(twoBursts(), SR);
    expect(onsets).toHaveLength(2);
    expect(Math.abs(onsets[0] - 0.3 * SR)).toBeLessThanOrEqual(SR * 0.01);
    expect(Math.abs(onsets[1] - 0.9 * SR)).toBeLessThanOrEqual(SR * 0.01);
  });

  it("stutters the first onset deterministically and keeps the original intact", () => {
    const input = twoBursts();
    const [out] = insertStutters([input], SR, 42);
    const [again] = insertStutters([input], SR, 42);
    expect(out.length).toBeGreaterThan(input.length);
    expect(again).toEqual(out);
    // Audio before the first onset is untouched; the tail matches the input tail.
    const onset = findOnsets(input, SR)[0];
    expect(out.subarray(0, onset)).toEqual(input.subarray(0, onset));
    expect(out.subarray(out.length - 100)).toEqual(input.subarray(input.length - 100));
    // Inserted grains are audible.
    expect(Math.max(...out.subarray(onset, onset + 400).map(Math.abs))).toBeGreaterThan(0.1);
  });

  it("always glitches the opening onset", () => {
    const stutters = planStutters(twoBursts(), SR, 42);
    expect(stutters[0].at).toBe(findOnsets(twoBursts(), SR)[0]);
  });

  it("mixes sparse and bursty glitch spacing", () => {
    const stutters = planStutters(longBursts(), SR, 7);
    const gaps = stutters.slice(1).map((s, i) => (s.at - stutters[i].at) / SR);
    // Burst segments glitch back to back; sparse segments wait well past 1.5 s.
    expect(Math.min(...gaps)).toBeGreaterThanOrEqual(0.4);
    expect(gaps.some((g) => g < 1)).toBe(true);
    expect(gaps.some((g) => g > 1.5)).toBe(true);
  });

  it("leaves silence unchanged", () => {
    const [out] = insertStutters([new Float32Array(SR)], SR, 1);
    expect(out.length).toBe(SR);
  });

  it("quantizes and chops the tail", () => {
    // period 16 samples at 500 Hz chop: first 8 on, next 8 off.
    const out = crushTail(new Float32Array(16).fill(0.5), SR, 1 / 16, 500, 1);
    expect(Array.from(out.subarray(0, 8))).toEqual(Array(8).fill(0.5));
    expect(Array.from(out.subarray(8, 16))).toEqual(Array(8).fill(0));
  });
});
