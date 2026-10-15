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

// 200 tone bursts, one every 0.11 s (0.06 s tone, 0.05 s silence): onsets closer than the gap.
function fastBursts(): Float32Array {
  const data = new Float32Array(SR * 30);
  for (let b = 0; b < 200; b++) {
    const start = b * 880;
    for (let i = start; i < start + 480; i++) data[i] = Math.sin(i * 0.3) * 0.5;
  }
  return data;
}

// Speech-like: 0.12 s syllables at 0.3 peak over a 0.05 floor, one every 0.2 s. Never silent.
function syllables(): Float32Array {
  const data = new Float32Array(SR * 2.2).fill(0.05);
  for (let b = 0; b < 10; b++) {
    const start = Math.round((b * 0.2 + 0.1) * SR);
    for (let i = start; i < start + 0.12 * SR; i++) data[i] += Math.sin(i * 0.3) * 0.3;
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

  it("finds syllable starts in speech that never goes silent", () => {
    expect(findOnsets(syllables(), SR)).toHaveLength(10);
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

  it("glitches about half the onsets at random", () => {
    const stutters = planStutters(longBursts(), SR, 7);
    // 200 onsets at 0.5 chance.
    expect(stutters.length).toBeGreaterThan(60);
    expect(stutters.length).toBeLessThan(140);
  });

  it("keeps glitches at least 0.6 s apart when onsets come faster", () => {
    const stutters = planStutters(fastBursts(), SR, 7);
    const gaps = stutters.slice(1).map((s, i) => (s.at - stutters[i].at) / SR);
    expect(stutters.length).toBeGreaterThan(10);
    expect(Math.min(...gaps)).toBeGreaterThanOrEqual(0.6);
  });

  it("glitches later onsets in place, adding time only for the opener", () => {
    const input = longBursts();
    const [opener, ...rest] = planStutters(input, SR, 7);
    const [out] = insertStutters([input], SR, 7);
    const shift = opener.grain * opener.repeats;
    expect(out.length).toBe(input.length + shift);
    expect(rest.every((s) => s.repeats === 0)).toBe(true);
    // The grain is altered; the audio right after it is the original, not a repeat.
    const { at, grain } = rest[0];
    expect(out.subarray(at + shift, at + shift + grain)).not.toEqual(
      input.subarray(at, at + grain)
    );
    expect(out.subarray(at + shift + grain, at + shift + grain + 100)).toEqual(
      input.subarray(at + grain, at + grain + 100)
    );
  });

  it("leaves silence unchanged", () => {
    const [out] = insertStutters([new Float32Array(SR)], SR, 1);
    expect(out.length).toBe(SR);
  });

  it("quantizes and clamps the tail without gating it", () => {
    const out = crushTail(new Float32Array(SR).fill(0.5), 1 / 16, 1);
    expect(out.every((v) => v === 0.5)).toBe(true);
    expect(crushTail(Float32Array.of(0.3, 2), 1 / 16, 1)).toEqual(Float32Array.of(0.3125, 1));
  });
});
