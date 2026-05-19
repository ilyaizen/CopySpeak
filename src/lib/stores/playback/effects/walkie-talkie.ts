// Walkie-talkie effect: narrow radio EQ, subtle saturation, light AM wobble,
// with PTT clicks at both ends and static under the speech.

import type { Effect } from "./types";

const CLICK_URL = "/sounds/walkie-click-start.wav";
const STATIC_URL = "/sounds/walkie-static.wav";

interface WalkieSamples {
  click: AudioBuffer | null;
  static: AudioBuffer | null;
}

let cachedSamples: Promise<WalkieSamples> | null = null;

async function loadSample(ctx: AudioContext, url: string): Promise<AudioBuffer | null> {
  try {
    const res = await fetch(url);
    if (!res.ok) return null;
    const bytes = await res.arrayBuffer();
    return await ctx.decodeAudioData(bytes);
  } catch (e) {
    console.warn(`[walkie-talkie] Failed to load sample ${url}:`, e);
    return null;
  }
}

async function loadSamples(ctx: AudioContext): Promise<WalkieSamples> {
  if (!cachedSamples) {
    cachedSamples = Promise.all([loadSample(ctx, CLICK_URL), loadSample(ctx, STATIC_URL)]).then(
      ([click, staticNoise]) => ({ click, static: staticNoise })
    );
  }
  return cachedSamples;
}

function buildSoftClipCurve(amount = 0.18): Float32Array<ArrayBuffer> {
  const n = 4096;
  const curve = new Float32Array(new ArrayBuffer(n * Float32Array.BYTES_PER_ELEMENT));
  const k = amount * 8;
  for (let i = 0; i < n; i++) {
    const x = (i * 2) / n - 1;
    curve[i] = Math.tanh((1 + k) * x) / Math.tanh(1 + k);
  }
  return curve;
}

export const walkieTalkie: Effect = {
  id: "walkie_talkie",
  async process(input: AudioBuffer, ctx: AudioContext): Promise<AudioBuffer> {
    const samples = await loadSamples(ctx);
    const headDur = samples.click?.duration ?? 0;
    const tailDur = samples.click?.duration ?? 0;
    const totalDur = headDur + input.duration + tailDur;
    const sampleRate = input.sampleRate;
    const channels = input.numberOfChannels;
    const totalLen = Math.ceil(totalDur * sampleRate);

    const offline = new OfflineAudioContext(channels, totalLen, sampleRate);

    if (samples.click) {
      const headSrc = offline.createBufferSource();
      headSrc.buffer = samples.click;
      headSrc.connect(offline.destination);
      headSrc.start(0);
    }

    const voiceSrc = offline.createBufferSource();
    voiceSrc.buffer = input;

    const highpass = offline.createBiquadFilter();
    highpass.type = "highpass";
    highpass.frequency.value = 420;
    highpass.Q.value = 0.6;

    const lowpass = offline.createBiquadFilter();
    lowpass.type = "lowpass";
    lowpass.frequency.value = 2600;
    lowpass.Q.value = 0.8;

    const presence = offline.createBiquadFilter();
    presence.type = "peaking";
    presence.frequency.value = 1200;
    presence.Q.value = 1.4;
    presence.gain.value = 4;

    const notch = offline.createBiquadFilter();
    notch.type = "notch";
    notch.frequency.value = 1800;
    notch.Q.value = 5;

    const shaper = offline.createWaveShaper();
    shaper.curve = buildSoftClipCurve();
    shaper.oversample = "2x";

    const comp = offline.createDynamicsCompressor();
    comp.threshold.value = -28;
    comp.knee.value = 14;
    comp.ratio.value = 2.2;
    comp.attack.value = 0.012;
    comp.release.value = 0.18;

    const wobble = offline.createOscillator();
    const wobbleDepth = offline.createGain();
    const radioGain = offline.createGain();
    wobble.type = "sine";
    wobble.frequency.value = 18;
    wobbleDepth.gain.value = 0.035;
    radioGain.gain.value = 0.86;
    wobble.connect(wobbleDepth);
    wobbleDepth.connect(radioGain.gain);

    voiceSrc.connect(highpass);
    highpass.connect(lowpass);
    lowpass.connect(presence);
    presence.connect(notch);
    notch.connect(shaper);
    shaper.connect(comp);
    comp.connect(radioGain);
    radioGain.connect(offline.destination);
    wobble.start(headDur);
    voiceSrc.start(headDur);

    if (samples.static) {
      const staticSrc = offline.createBufferSource();
      const staticGain = offline.createGain();
      staticSrc.buffer = samples.static;
      staticSrc.loop = true;
      staticGain.gain.value = 0.055;
      staticSrc.connect(staticGain);
      staticGain.connect(offline.destination);
      staticSrc.start(headDur, 0, input.duration);
    }

    if (samples.click) {
      const tailSrc = offline.createBufferSource();
      tailSrc.buffer = samples.click;
      tailSrc.connect(offline.destination);
      tailSrc.start(headDur + input.duration);
    }

    return offline.startRendering();
  }
};
