// Cyberpunk effect: a hard-start synth beep, a short modem chirp at the end,
// a glitch stutter on the first word, in-place crunch on random word onsets,
// a driven and lightly ring-modulated voice, and a low static bed under
// everything. Only the first-word stutter inserts time.

import type { Effect } from "./types";
import { buildSoftClipCurve } from "./walkie-talkie";

const INTRO_DUR = 0.05;
const OUTRO_DUR = 0.15;
const GLITCH_CHANCE = 0.5;
const MIN_GAP_SEC = 0.6;
// Sample-and-hold rate for in-place glitches; lower is crunchier.
const GLITCH_HOLD_HZ = 3000;

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

/**
 * Sample indices where a sound starts: a 10 ms frame at least twice the quietest
 * of the three frames before it, above 5% of peak, and 60 ms after the last start.
 * Speech rarely goes silent between syllables, so a silence gate finds almost none.
 */
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
  const onsets: number[] = [];
  let last = -Infinity;
  for (let f = 3; f < frames; f++) {
    const quietest = Math.min(rms[f - 3], rms[f - 2], rms[f - 1]);
    if (rms[f] > peak * 0.05 && rms[f] >= 2 * Math.max(quietest, 1e-6) && f - last >= 6) {
      onsets.push(f * frame);
      last = f;
    }
  }
  return onsets;
}

interface Stutter {
  at: number;
  grain: number;
  /** 0 = glitch the grain in place. */
  repeats: number;
}

/**
 * Picks glitch points: each onset glitches at random, at least MIN_GAP_SEC
 * apart. The first onset always glitches with a repeated stutter; the rest
 * glitch in place, since a repeated 40-70 ms grain reads as echo.
 */
export function planStutters(data: Float32Array, sampleRate: number, seed: number): Stutter[] {
  const rand = mulberry32(seed);
  const stutters: Stutter[] = [];
  for (const at of findOnsets(data, sampleRate)) {
    const last = stutters[stutters.length - 1];
    if (last && (at - last.at < MIN_GAP_SEC * sampleRate || rand() >= GLITCH_CHANCE)) continue;
    const opener = !last;
    const grainSec = opener ? 0.06 + rand() * 0.04 : 0.04 + rand() * 0.03;
    const grain = Math.min(Math.round(grainSec * sampleRate), data.length - at);
    const repeats = opener ? 2 + Math.floor(rand() * 2) : 0;
    if (grain > 0) stutters.push({ at, grain, repeats });
  }
  return stutters;
}

/** Repeats the opener's start 2-3 times; crunches the other onsets in place, adding no time. */
export function insertStutters(
  channels: Float32Array[],
  sampleRate: number,
  seed: number
): Float32Array<ArrayBuffer>[] {
  const len = channels[0].length;
  const stutters = planStutters(channels[0], sampleRate, seed);

  const extra = stutters.reduce((n, s) => n + s.grain * s.repeats, 0);
  const fadeMax = Math.round(sampleRate * 0.002);
  const inPlaceHold = Math.max(2, Math.round(sampleRate / GLITCH_HOLD_HZ));
  return channels.map((data) => {
    const out = new Float32Array(len + extra);
    let read = 0;
    let write = 0;
    for (const { at, grain, repeats } of stutters) {
      out.set(data.subarray(read, at), write);
      write += at - read;
      const fade = Math.max(1, Math.min(fadeMax, Math.floor(grain / 2)));
      if (repeats === 0) {
        // Crossfade into and out of the held samples so the edges don't click.
        for (let j = 0; j < grain; j++) {
          const env = Math.min(1, j / fade, (grain - 1 - j) / fade);
          const dry = data[at + j];
          out[write + j] = dry + (data[at + j - (j % inPlaceHold)] - dry) * env;
        }
        write += grain;
        read = at + grain;
        continue;
      }
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

/** Bitcrush: amplitude quantize, then clamp. No gating, so clicks and tones keep their shape. */
export function crushTail(
  data: Float32Array,
  step: number,
  gain: number
): Float32Array<ArrayBuffer> {
  const out = new Float32Array(data.length);
  for (let i = 0; i < data.length; i++) {
    const q = Math.round((data[i] * gain) / step) * step;
    out[i] = Math.max(-1, Math.min(1, q));
  }
  return out;
}

function envelope(
  ctx: BaseAudioContext,
  t0: number,
  dur: number,
  peak: number,
  fade = 0.004
): GainNode {
  const g = ctx.createGain();
  g.gain.setValueAtTime(0, t0);
  g.gain.linearRampToValueAtTime(peak, t0 + fade);
  g.gain.setValueAtTime(peak, t0 + dur - fade);
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
function fsk(
  ctx: BaseAudioContext,
  t0: number,
  dur: number,
  f1: number,
  f2: number,
  step: number,
  peak = 0.1
) {
  const osc = tone(ctx, "square", f1, t0, dur, peak);
  for (let t = t0, i = 0; t < t0 + dur; t += step, i++) {
    osc.frequency.setValueAtTime(i % 2 ? f2 : f1, t);
  }
}

function click(ctx: BaseAudioContext, t0: number) {
  // Peak 0.15 so the x3.2 outro crush stays under full scale.
  const sr = ctx.sampleRate;
  const buf = ctx.createBuffer(1, Math.round(sr * 0.006), sr);
  const d = buf.getChannelData(0);
  for (let i = 0; i < d.length; i++) d[i] = (i % 2 ? -0.15 : 0.15) * Math.exp(-i / (sr * 0.0008));
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.connect(ctx.destination);
  src.start(t0);
}

function renderIntro(ctx: BaseAudioContext) {
  // Hard-start high synth beep, 1 ms fades so it still reaches full level. 40 ms:
  // a 6 ms burst reads as a click, not a beep.
  tone(ctx, "square", 4500, 0, 0.04, 0.12, envelope(ctx, 0, 0.04, 0.12, 0.001));
}

function renderOutro(ctx: BaseAudioContext) {
  fsk(ctx, 0, 0.07, 1180, 1650, 0.008, 0.05);
  click(ctx, 0.1);
}

/** Renders a mono tail offline so it can be post-processed sample by sample. */
async function renderTail(
  sampleRate: number,
  dur: number,
  render: (ctx: BaseAudioContext) => void
): Promise<Float32Array<ArrayBuffer>> {
  const ctx = new OfflineAudioContext(1, Math.ceil(dur * sampleRate), sampleRate);
  render(ctx);
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

    const intro = crushTail(await renderTail(sampleRate, INTRO_DUR, renderIntro), 1 / 16, 3.6);
    const outro = crushTail(await renderTail(sampleRate, OUTRO_DUR, renderOutro), 1 / 12, 3.2);

    const totalLen = Math.ceil((INTRO_DUR + voice.duration + OUTRO_DUR) * sampleRate);
    const offline = new OfflineAudioContext(channels, totalLen, sampleRate);

    // Hard clamp on the final mix. An identity curve over [-1, 1] pins any
    // input outside that range to ±1, so summed layers can't exceed full scale.
    const master = offline.createWaveShaper();
    master.curve = new Float32Array([-1, 1]);
    master.connect(offline.destination);

    const play = (data: Float32Array<ArrayBuffer>, at: number) => {
      const buf = offline.createBuffer(channels, data.length, sampleRate);
      for (let c = 0; c < channels; c++) buf.copyToChannel(data, c);
      const src = offline.createBufferSource();
      src.buffer = buf;
      src.connect(master);
      src.start(at);
    };
    play(intro, 0);
    play(outro, INTRO_DUR + voice.duration);

    const voiceSrc = offline.createBufferSource();
    voiceSrc.buffer = voice;

    // Phone voice band, 300-3400 Hz: high-pass before the drive keeps lows out
    // of the distortion; the band again after the chain cuts what the drive and
    // ring-mod regenerate, with the low-pass stacked for 24 dB/oct.
    const phoneFilter = (type: BiquadFilterType, freq: number) => {
      const filter = offline.createBiquadFilter();
      filter.type = type;
      filter.frequency.value = freq;
      return filter;
    };
    const highpass = phoneFilter("highpass", 300);

    const presence = offline.createBiquadFilter();
    presence.type = "peaking";
    presence.frequency.value = 3000;
    presence.Q.value = 1.1;
    presence.gain.value = 5;

    // +6 dB pre-clip drive; the soft clip curve keeps peaks within ±1.
    const drive = offline.createGain();
    drive.gain.value = 2;

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
      .connect(drive)
      .connect(softClip)
      .connect(comp)
      .connect(ring)
      .connect(phoneFilter("highpass", 300))
      .connect(phoneFilter("lowpass", 3400))
      .connect(phoneFilter("lowpass", 3400))
      .connect(master);
    voiceSrc.start(INTRO_DUR);

    // Low static bed under the whole output, about -36 dBFS (full-scale noise
    // before the band-pass). Seeded so renders repeat.
    const staticBuf = offline.createBuffer(1, totalLen, sampleRate);
    const staticData = staticBuf.getChannelData(0);
    const staticRand = mulberry32(2);
    for (let i = 0; i < staticData.length; i++) staticData[i] = staticRand() * 2 - 1;
    const staticSrc = offline.createBufferSource();
    staticSrc.buffer = staticBuf;
    const staticBp = offline.createBiquadFilter();
    staticBp.type = "bandpass";
    staticBp.frequency.value = 2500;
    staticBp.Q.value = 0.7;
    const staticGain = offline.createGain();
    staticGain.gain.value = 10 ** (-36 / 20);
    staticSrc.connect(staticBp).connect(staticGain).connect(master);
    staticSrc.start(0);

    return offline.startRendering();
  }
};
