// Cyberpunk effect: synthesized 56k modem handshake fragments at both ends,
// glitch stutters on the first word and bursty word onsets, and a
// bright, lightly ring-modulated voice. Stutters insert time.

import type { Effect } from "./types";
import { buildSoftClipCurve } from "./walkie-talkie";

const INTRO_DUR = 0.9;
const OUTRO_DUR = 0.6;
const SEGMENT_MIN_SEC = 2;
const SEGMENT_MAX_SEC = 4;
const BURST_SHARE = 1 / 3;
const BURST_CHANCE = 1 / 2;
const BURST_MIN_GAP_SEC = 0.4;
const SPARSE_CHANCE = 1 / 20;
const SPARSE_MIN_GAP_SEC = 1.5;

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Sample indices where speech rises after ≥80 ms of near-silence. */
export function findOnsets(data: Float32Array, sampleRate: number): number[] {
  const frame = Math.max(1, Math.round(sampleRate * 0.01));
  const frames = Math.floor(data.length / frame);
  const rms = new Float32Array(frames);
  let peak = 0;
  for (let f = 0; f < frames; f++) {
    let sum = 0;
    for (let i = f * frame; i < (f + 1) * frame; i++) sum += data[i] * data[i];
    rms[f] = Math.sqrt(sum / frame);
    peak = Math.max(peak, rms[f]);
  }
  if (peak === 0) return [];

  const hi = peak * 0.25;
  const lo = peak * 0.08;
  const minQuiet = 8;
  const onsets: number[] = [];
  let quiet = minQuiet; // buffer start counts as silence
  for (let f = 0; f < frames; f++) {
    if (rms[f] < lo) quiet++;
    else if (rms[f] > hi && quiet >= minQuiet) {
      onsets.push(f * frame);
      quiet = 0;
    } else quiet = 0;
  }
  return onsets;
}

interface Stutter {
  at: number;
  grain: number;
  repeats: number;
}

/**
 * Picks glitch points. The utterance splits into 2-4 s segments, each either
 * sparse (rare glitches, long gaps) or bursty (frequent glitches, short gaps).
 * The first onset always glitches and gets a longer, busier stutter.
 */
export function planStutters(data: Float32Array, sampleRate: number, seed: number): Stutter[] {
  const rand = mulberry32(seed);
  const stutters: Stutter[] = [];
  let segEnd = 0;
  let burst = false;
  for (const at of findOnsets(data, sampleRate)) {
    if (at >= segEnd) {
      const segSec = SEGMENT_MIN_SEC + rand() * (SEGMENT_MAX_SEC - SEGMENT_MIN_SEC);
      segEnd = at + Math.round(segSec * sampleRate);
      burst = rand() < BURST_SHARE;
    }
    const last = stutters[stutters.length - 1];
    const minGap = (burst ? BURST_MIN_GAP_SEC : SPARSE_MIN_GAP_SEC) * sampleRate;
    const chance = burst ? BURST_CHANCE : SPARSE_CHANCE;
    if (last && (at - last.at < minGap || rand() >= chance)) continue;
    const opener = !last;
    const grainSec = opener ? 0.06 + rand() * 0.04 : 0.04 + rand() * 0.03;
    const grain = Math.min(Math.round(grainSec * sampleRate), data.length - at);
    const repeats = opener ? 4 + Math.floor(rand() * 2) : 2 + Math.floor(rand() * 3);
    if (grain > 0) stutters.push({ at, grain, repeats });
  }
  return stutters;
}

/** Repeats the start of each planned word onset 2-5 extra times. */
export function insertStutters(
  channels: Float32Array[],
  sampleRate: number,
  seed: number
): Float32Array<ArrayBuffer>[] {
  const len = channels[0].length;
  const stutters = planStutters(channels[0], sampleRate, seed);

  const extra = stutters.reduce((n, s) => n + s.grain * s.repeats, 0);
  const fadeMax = Math.round(sampleRate * 0.002);
  return channels.map((data) => {
    const out = new Float32Array(len + extra);
    let read = 0;
    let write = 0;
    for (const { at, grain, repeats } of stutters) {
      out.set(data.subarray(read, at), write);
      write += at - read;
      const fade = Math.max(1, Math.min(fadeMax, Math.floor(grain / 2)));
      for (let r = 0; r < repeats; r++) {
        // Odd repeats sample-and-hold every 4 samples for digital crunch.
        const hold = r % 2 === 1 ? 4 : 1;
        for (let j = 0; j < grain; j++) {
          const env = Math.min(1, j / fade, (grain - 1 - j) / fade);
          out[write + j] = data[at + j - (j % hold)] * env;
        }
        write += grain;
      }
      read = at; // the word then plays in full
    }
    out.set(data.subarray(read), write);
    return out;
  });
}

/** Bitcrush (amplitude quantize) and hard on/off chop gated at `chopHz`. */
export function crushTail(
  data: Float32Array,
  sampleRate: number,
  step: number,
  chopHz: number,
  gain: number
): Float32Array<ArrayBuffer> {
  const out = new Float32Array(data.length);
  const half = sampleRate / chopHz / 2;
  for (let i = 0; i < data.length; i++) {
    const on = Math.floor(i / half) % 2 === 0 ? 1 : 0;
    const q = Math.round((data[i] * gain) / step) * step;
    out[i] = Math.max(-1, Math.min(1, q)) * on;
  }
  return out;
}

function envelope(ctx: BaseAudioContext, t0: number, dur: number, peak: number): GainNode {
  const g = ctx.createGain();
  g.gain.setValueAtTime(0, t0);
  g.gain.linearRampToValueAtTime(peak, t0 + 0.004);
  g.gain.setValueAtTime(peak, t0 + dur - 0.004);
  g.gain.linearRampToValueAtTime(0, t0 + dur);
  g.connect(ctx.destination);
  return g;
}

function tone(
  ctx: BaseAudioContext,
  type: OscillatorType,
  freq: number,
  t0: number,
  dur: number,
  peak: number,
  dest: AudioNode = envelope(ctx, t0, dur, peak)
): OscillatorNode {
  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, t0);
  osc.connect(dest);
  osc.start(t0);
  osc.stop(t0 + dur);
  return osc;
}

/** Frequency-shift-keyed warble, the chirpy part of a handshake. */
function fsk(ctx: BaseAudioContext, t0: number, dur: number, f1: number, f2: number, step: number) {
  const osc = tone(ctx, "square", f1, t0, dur, 0.05);
  for (let t = t0, i = 0; t < t0 + dur; t += step, i++) {
    osc.frequency.setValueAtTime(i % 2 ? f2 : f1, t);
  }
}

/** Band-passed noise, square-gated at `gateHz` like the scrambler hiss. */
function hiss(
  ctx: BaseAudioContext,
  noise: AudioBuffer,
  t0: number,
  dur: number,
  peak: number,
  center: number,
  gateHz: number
) {
  const src = ctx.createBufferSource();
  src.buffer = noise;
  src.loop = true;
  const bp = ctx.createBiquadFilter();
  bp.type = "bandpass";
  bp.frequency.value = center;
  bp.Q.value = 0.8;
  const gate = ctx.createGain();
  if (gateHz > 0) {
    // Square ±1 scaled to ±0.5 around 0.5, so the gate swings 0..1.
    gate.gain.value = 0.5;
    const depth = ctx.createGain();
    depth.gain.value = 0.5;
    depth.connect(gate.gain);
    tone(ctx, "square", gateHz, t0, dur, 0, depth);
  }
  src
    .connect(bp)
    .connect(gate)
    .connect(envelope(ctx, t0, dur, peak));
  src.start(t0);
  src.stop(t0 + dur);
}

function click(ctx: BaseAudioContext, t0: number) {
  const sr = ctx.sampleRate;
  const buf = ctx.createBuffer(1, Math.round(sr * 0.006), sr);
  const d = buf.getChannelData(0);
  for (let i = 0; i < d.length; i++) d[i] = (i % 2 ? -0.6 : 0.6) * Math.exp(-i / (sr * 0.0008));
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.connect(ctx.destination);
  src.start(t0);
}

function renderIntro(ctx: BaseAudioContext, noise: AudioBuffer) {
  // Answer tone with 15 Hz amplitude wobble.
  const am = ctx.createGain();
  am.gain.value = 0.6;
  am.connect(envelope(ctx, 0, 0.18, 0.12));
  tone(ctx, "sine", 2100, 0, 0.18, 0, am);
  const wobble = ctx.createGain();
  wobble.gain.value = 0.4;
  tone(ctx, "sine", 15, 0, 0.18, 0, wobble);
  wobble.connect(am.gain);
  click(ctx, 0.06);
  click(ctx, 0.12);

  fsk(ctx, 0.18, 0.18, 980, 1180, 0.012);

  // Falling dual-tone "bong".
  for (const [from, to] of [
    [1650, 1200],
    [2400, 1800]
  ]) {
    tone(ctx, "sine", from, 0.36, 0.12, 0.1).frequency.exponentialRampToValueAtTime(to, 0.48);
  }

  hiss(ctx, noise, 0.48, 0.36, 0.2, 1800, 37);
  hiss(ctx, noise, 0.5, 0.32, 0.08, 3400, 0);
  click(ctx, 0.86);
}

function renderOutro(ctx: BaseAudioContext, noise: AudioBuffer) {
  fsk(ctx, 0, 0.14, 1180, 1650, 0.008);
  hiss(ctx, noise, 0.1, 0.4, 0.22, 2200, 53);
  click(ctx, 0.3);
  click(ctx, 0.52);
}

/** Renders a mono tail offline so it can be post-processed sample by sample. */
async function renderTail(
  sampleRate: number,
  dur: number,
  render: (ctx: BaseAudioContext, noise: AudioBuffer) => void
): Promise<Float32Array<ArrayBuffer>> {
  const ctx = new OfflineAudioContext(1, Math.ceil(dur * sampleRate), sampleRate);
  const noise = ctx.createBuffer(1, sampleRate, sampleRate);
  const rand = mulberry32(1);
  const noiseData = noise.getChannelData(0);
  for (let i = 0; i < noiseData.length; i++) noiseData[i] = rand() * 2 - 1;
  render(ctx, noise);
  const rendered = await ctx.startRendering();
  return new Float32Array(rendered.getChannelData(0));
}

export const cyberpunk: Effect = {
  id: "cyberpunk",
  async process(input: AudioBuffer, ctx: AudioContext): Promise<AudioBuffer> {
    const sampleRate = input.sampleRate;
    const channels = input.numberOfChannels;
    const stuttered = insertStutters(
      Array.from({ length: channels }, (_, c) => input.getChannelData(c)),
      sampleRate,
      input.length
    );
    const voice = ctx.createBuffer(channels, stuttered[0].length, sampleRate);
    stuttered.forEach((data, c) => voice.copyToChannel(data, c));

    const intro = crushTail(
      await renderTail(sampleRate, INTRO_DUR, renderIntro),
      sampleRate,
      1 / 16,
      30,
      1.8
    );
    const outro = crushTail(
      await renderTail(sampleRate, OUTRO_DUR, renderOutro),
      sampleRate,
      1 / 12,
      45,
      1.6
    );

    const totalLen = Math.ceil((INTRO_DUR + voice.duration + OUTRO_DUR) * sampleRate);
    const offline = new OfflineAudioContext(channels, totalLen, sampleRate);

    const play = (data: Float32Array<ArrayBuffer>, at: number) => {
      const buf = offline.createBuffer(channels, data.length, sampleRate);
      for (let c = 0; c < channels; c++) buf.copyToChannel(data, c);
      const src = offline.createBufferSource();
      src.buffer = buf;
      src.connect(offline.destination);
      src.start(at);
    };
    play(intro, 0);
    play(outro, INTRO_DUR + voice.duration);

    const voiceSrc = offline.createBufferSource();
    voiceSrc.buffer = voice;

    const highpass = offline.createBiquadFilter();
    highpass.type = "highpass";
    highpass.frequency.value = 180;

    const presence = offline.createBiquadFilter();
    presence.type = "peaking";
    presence.frequency.value = 3000;
    presence.Q.value = 1.1;
    presence.gain.value = 5;

    const softClip = offline.createWaveShaper();
    softClip.curve = buildSoftClipCurve(0.3);
    softClip.oversample = "2x";

    const comp = offline.createDynamicsCompressor();
    comp.threshold.value = -24;
    comp.ratio.value = 3;
    comp.attack.value = 0.008;
    comp.release.value = 0.15;

    // Low-depth 55 Hz square ring-mod for a synthetic buzz.
    const ring = offline.createGain();
    ring.gain.value = 0.85;
    const ringDepth = offline.createGain();
    ringDepth.gain.value = 0.12;
    tone(offline, "square", 55, INTRO_DUR, voice.duration, 0, ringDepth);
    ringDepth.connect(ring.gain);

    voiceSrc
      .connect(highpass)
      .connect(presence)
      .connect(softClip)
      .connect(comp)
      .connect(ring)
      .connect(offline.destination);
    voiceSrc.start(INTRO_DUR);

    return offline.startRendering();
  }
};
