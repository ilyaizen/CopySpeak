import { describe, expect, it } from "vite-plus/test";
import { concatTrim, TimeStretcher } from "./time-stretch";

const SAMPLE_RATE = 24000;

function sine(frames: number, hz: number): Float32Array<ArrayBuffer> {
  const samples = new Float32Array(frames);
  for (let i = 0; i < frames; i++) {
    samples[i] = Math.sin((2 * Math.PI * hz * i) / SAMPLE_RATE);
  }
  return samples;
}

/** Zero crossings per second, a cheap stand-in for perceived pitch on a sine. */
function crossingsPerSecond(samples: Float32Array): number {
  let crossings = 0;
  for (let i = 1; i < samples.length; i++) {
    if (samples[i - 1] < 0 !== samples[i] < 0) crossings++;
  }
  return (crossings * SAMPLE_RATE) / samples.length;
}

/** Push a whole signal through in chunks, flush, and return the joined output. */
function render(
  input: Float32Array<ArrayBuffer>,
  speed: number,
  pitch: number,
  chunkFrames = 4800
): Float32Array {
  const stretcher = new TimeStretcher(SAMPLE_RATE, 1);
  stretcher.setRate(speed, pitch);
  const parts: Float32Array[] = [];
  for (let offset = 0; offset < input.length; offset += chunkFrames) {
    const output = stretcher.push([input.slice(offset, offset + chunkFrames)]);
    if (output) parts.push(output[0]);
  }
  const tail = stretcher.flush();
  if (tail) parts.push(tail[0]);
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const joined = new Float32Array(total);
  let written = 0;
  for (const part of parts) {
    joined.set(part, written);
    written += part.length;
  }
  return joined;
}

/**
 * Voiced worst case for WSOLA matching: gliding f0 with vibrato and 15
 * harmonics, so any splice lands mid-waveform instead of on a zero crossing.
 */
function voiced(frames: number): Float32Array<ArrayBuffer> {
  const out = new Float32Array(frames);
  let phase = 0;
  for (let i = 0; i < frames; i++) {
    const t = i / SAMPLE_RATE;
    const f0 = 145 + 35 * Math.sin(2 * Math.PI * 0.21 * t) + 4 * Math.sin(2 * Math.PI * 6 * t);
    phase += (2 * Math.PI * f0) / SAMPLE_RATE;
    let volume = 0;
    for (let h = 1; h <= 15; h++) volume += Math.sin(phase * h) / h;
    out[i] = 0.3 * volume;
  }
  return out;
}

/** Largest per-sample step, restricted to (or, with `exclude`, away from) a window around each seam. */
function maxDeltaNearSeams(
  samples: Float32Array,
  seams: number[],
  window: number,
  exclude = false
): number {
  let max = 0;
  for (let i = 1; i < samples.length; i++) {
    if (seams.some((seam) => Math.abs(i - seam) <= window) === exclude) continue;
    max = Math.max(max, Math.abs(samples[i] - samples[i - 1]));
  }
  return max;
}

describe("TimeStretcher", () => {
  it("passes audio through untouched at speed 1 and pitch 1", () => {
    const input = sine(SAMPLE_RATE, 440);
    const stretcher = new TimeStretcher(SAMPLE_RATE, 1);
    expect(stretcher.isBypassed).toBe(true);
    expect(stretcher.push([input])?.[0]).toBe(input);
    expect(stretcher.flush()).toBeNull();
  });

  // Pins the `pitch` setter + `stretch.tempo` override: SoundTouch derives
  // tempo = 1 / pitch from the setter, and we replace it with speed / pitch.
  it.each([0.5, 1.5, 2])("changes duration by speed %s without moving pitch", (speed) => {
    const input = sine(2 * SAMPLE_RATE, 440);
    const output = render(input, speed, 1);
    expect(output.length / (input.length / speed)).toBeCloseTo(1, 1);
    expect(crossingsPerSecond(output)).toBeCloseTo(880, -2);
  });

  it.each([0.75, 1.35])("changes pitch by %s without moving duration", (pitch) => {
    const input = sine(2 * SAMPLE_RATE, 440);
    const output = render(input, 1, pitch);
    expect(output.length / input.length).toBeCloseTo(1, 1);
    expect(crossingsPerSecond(output)).toBeCloseTo(880 * pitch, -2);
  });

  it("applies speed and pitch independently when combined", () => {
    const input = sine(2 * SAMPLE_RATE, 440);
    const output = render(input, 2, 1.35);
    expect(output.length / (input.length / 2)).toBeCloseTo(1, 1);
    expect(crossingsPerSecond(output)).toBeCloseTo(880 * 1.35, -2);
  });

  it("keeps stereo channels aligned", () => {
    const stretcher = new TimeStretcher(SAMPLE_RATE, 2);
    stretcher.setRate(1.5, 1);
    const left = sine(SAMPLE_RATE, 440);
    const right = sine(SAMPLE_RATE, 220);
    const output = stretcher.push([left, right]);
    expect(output).not.toBeNull();
    expect(output![0].length).toBe(output![1].length);
    expect(crossingsPerSecond(output![0])).toBeCloseTo(880, -2);
    expect(crossingsPerSecond(output![1])).toBeCloseTo(440, -2);
  });

  it("drops nothing but silence when flushing a fragment tail", () => {
    const input = sine(SAMPLE_RATE, 440);
    const stretcher = new TimeStretcher(SAMPLE_RATE, 1);
    stretcher.setRate(1.25, 1);
    const body = stretcher.push([input])?.[0].length ?? 0;
    const tail = stretcher.flush()?.[0].length ?? 0;
    // Flush pads with silence but is capped at the frames the input is owed.
    expect(body + tail).toBeLessThanOrEqual(Math.round(input.length / 1.25));
    expect(body + tail).toBeGreaterThan(input.length / 1.25 - 0.05 * SAMPLE_RATE);
  });

  // Flush resets SoundTouch between fragments; without the fades the next
  // fragment's first sample lands at full amplitude in one step while the
  // flushed tail splices mid-waveform. Both showed up as per-sample steps well
  // above the signal's natural slew, audible as scattered ticks.
  it.each([
    [1.35, 1.15],
    [1.25, 1.0]
  ])(
    "keeps fragment seams as smooth as the surrounding audio (speed %s, pitch %s)",
    (speed, pitch) => {
      const input = voiced(6 * SAMPLE_RATE);
      const fragmentFrames = 2 * SAMPLE_RATE;
      const chunkFrames = 2048;
      const stretcher = new TimeStretcher(SAMPLE_RATE, 1);
      stretcher.setRate(speed, pitch);
      const parts: Float32Array[] = [];
      const seams: number[] = [];
      let total = 0;
      for (let start = 0; start < input.length; start += fragmentFrames) {
        const end = Math.min(start + fragmentFrames, input.length);
        for (let offset = start; offset < end; offset += chunkFrames) {
          const output = stretcher.push([input.slice(offset, Math.min(offset + chunkFrames, end))]);
          if (output) {
            parts.push(output[0]);
            total += output[0].length;
          }
        }
        const tail = stretcher.flush();
        if (tail) {
          parts.push(tail[0]);
          total += tail[0].length;
        }
        if (end < input.length) seams.push(total);
      }
      const joined = new Float32Array(total);
      let written = 0;
      for (const part of parts) {
        joined.set(part, written);
        written += part.length;
      }

      // Reference is the same stretched audio away from the seams - the signal's
      // natural per-sample slew. 15 ms covers the flushed-tail splice and the
      // next fragment's post-seam onset.
      const window = Math.round(0.015 * SAMPLE_RATE);
      const nearSeams = maxDeltaNearSeams(joined, seams, window);
      const elsewhere = maxDeltaNearSeams(joined, seams, window, true);
      expect(seams.length).toBe(2);
      expect(nearSeams).toBeLessThanOrEqual(elsewhere);
    }
  );
});

describe("concatTrim", () => {
  it("truncates overflow and leaves shortfall silent", () => {
    const parts = [[Float32Array.of(1, 2, 3)], null, [Float32Array.of(4, 5)]];
    expect(Array.from(concatTrim(parts, 4, 1)[0])).toEqual([1, 2, 3, 4]);
    expect(Array.from(concatTrim(parts, 7, 1)[0])).toEqual([1, 2, 3, 4, 5, 0, 0]);
  });
});
