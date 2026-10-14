import { describe, expect, it } from "vite-plus/test";
import { findOnsets, insertStutters } from "./cyberpunk";

const SR = 8000;

// 0.3 s silence, 0.3 s tone, 0.3 s silence, 0.3 s tone.
function twoBursts(): Float32Array {
  const data = new Float32Array(SR * 1.2);
  for (const start of [0.3, 0.9]) {
    for (let i = start * SR; i < (start + 0.3) * SR; i++) data[i] = Math.sin(i * 0.3) * 0.5;
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

  it("leaves silence unchanged", () => {
    const [out] = insertStutters([new Float32Array(SR)], SR, 1);
    expect(out.length).toBe(SR);
  });
});
