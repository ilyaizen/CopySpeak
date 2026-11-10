// Live walkie-talkie chain for streamed PCM playback.
//
// walkie-talkie.ts renders the effect over a whole decoded buffer. Streaming
// playback schedules raw PCM chunks while synthesis is still running, so it has
// no buffer to render. This module builds the same radio character as a
// persistent node graph: the stream scheduler connects its output to `input`,
// and the EQ, saturation, compression and AM wobble run live as audio streams
// through.
//
// Rust keeps game_boy and cyberpunk on the batch path, so `createLiveEffect`
// returns null for them and the scheduler plays straight into its destination.

import type { EffectId } from "#lib/types";
import { buildSoftClipCurve } from "./walkie-talkie";

const CLICK_URL = "/sounds/walkie-click-start.wav";
const STATIC_URL = "/sounds/walkie-static.wav";

/** Level of the looping static bed under the speech (same as the batch effect). */
const STATIC_LEVEL = 0.055;
/** How long the tail click may ring out before the graph is torn down. */
const TAIL_DISPOSE_MS = 1000;

export interface LiveEffect {
  /** The node streamed PCM plays into. */
  readonly input: AudioNode;
  /** Begin the effect: radio wobble, head click and the static bed. */
  start(): void;
  /** Natural end of the reading: tail click, then a delayed self-dispose. */
  finish(): void;
  /** Immediate teardown (user stop, teardown). Idempotent. */
  dispose(): void;
}

interface WalkieSamples {
  click: AudioBuffer | null;
  static: AudioBuffer | null;
}

// The batch effect and the live chain share one decode per AudioContext: both
// read the same bundled WAVs, so a reading that crosses paths must not fetch
// them twice.
let cachedCtx: AudioContext | null = null;
let cachedSamples: Promise<WalkieSamples> | null = null;

async function loadSample(ctx: AudioContext, url: string): Promise<AudioBuffer | null> {
  try {
    const res = await fetch(url);
    if (!res.ok) return null;
    const bytes = await res.arrayBuffer();
    return await ctx.decodeAudioData(bytes);
  } catch (e) {
    console.warn(`[live-walkie] Failed to load sample ${url}:`, e);
    return null;
  }
}

function loadSamples(ctx: AudioContext): Promise<WalkieSamples> {
  if (cachedCtx !== ctx || !cachedSamples) {
    cachedCtx = ctx;
    cachedSamples = Promise.all([loadSample(ctx, CLICK_URL), loadSample(ctx, STATIC_URL)]).then(
      ([click, staticNoise]) => ({ click, static: staticNoise })
    );
  }
  return cachedSamples;
}

/** One live reading's radio chain; the store owns the instance and its lifecycle. */
class WalkieTalkieLiveEffect implements LiveEffect {
  readonly input: GainNode;

  private readonly _ctx: AudioContext;
  private readonly _radioGain: GainNode;
  private readonly _wobble: OscillatorNode;
  private readonly _nodes: AudioNode[];
  private readonly _sources: AudioBufferSourceNode[] = [];
  private _staticSource: AudioBufferSourceNode | null = null;
  private _samples: WalkieSamples | null = null;
  private _started = false;
  private _finished = false;
  private _disposed = false;
  private _disposeTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(ctx: AudioContext) {
    this._ctx = ctx;
    this.input = ctx.createGain();

    const highpass = ctx.createBiquadFilter();
    highpass.type = "highpass";
    highpass.frequency.value = 420;
    highpass.Q.value = 0.6;

    const lowpass = ctx.createBiquadFilter();
    lowpass.type = "lowpass";
    lowpass.frequency.value = 2600;
    lowpass.Q.value = 0.8;

    const presence = ctx.createBiquadFilter();
    presence.type = "peaking";
    presence.frequency.value = 1200;
    presence.Q.value = 1.4;
    presence.gain.value = 4;

    const notch = ctx.createBiquadFilter();
    notch.type = "notch";
    notch.frequency.value = 1800;
    notch.Q.value = 5;

    const softClip = ctx.createWaveShaper();
    softClip.curve = buildSoftClipCurve();
    softClip.oversample = "2x";

    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -28;
    comp.knee.value = 14;
    comp.ratio.value = 2.2;
    comp.attack.value = 0.012;
    comp.release.value = 0.18;

    this._radioGain = ctx.createGain();
    this._radioGain.gain.value = 0.86;

    const wobbleDepth = ctx.createGain();
    wobbleDepth.gain.value = 0.035;
    this._wobble = ctx.createOscillator();
    this._wobble.type = "sine";
    this._wobble.frequency.value = 18;
    this._wobble.connect(wobbleDepth);
    wobbleDepth.connect(this._radioGain.gain);

    this.input.connect(highpass);
    highpass.connect(lowpass);
    lowpass.connect(presence);
    presence.connect(notch);
    notch.connect(softClip);
    softClip.connect(comp);
    comp.connect(this._radioGain);
    this._radioGain.connect(ctx.destination);

    this._nodes = [
      this.input,
      highpass,
      lowpass,
      presence,
      notch,
      softClip,
      comp,
      this._radioGain,
      wobbleDepth,
      this._wobble
    ];

    // Samples load while the first PCM chunks arrive. A sample that lands after
    // start() plays late instead of being dropped; one that lands after finish()
    // still plays the tail click while the graph is alive.
    void loadSamples(ctx).then((samples) => {
      if (this._disposed) return;
      this._samples = samples;
      if (this._finished) this._playClick();
      else if (this._started) this._startSamples();
    });
  }

  start(): void {
    if (this._started || this._finished || this._disposed) return;
    this._started = true;
    this._wobble.start();
    this._startSamples();
  }

  finish(): void {
    if (this._finished || this._disposed) return;
    this._finished = true;
    this._stopStatic();
    this._playClick();
    this._disposeTimer = setTimeout(() => this.dispose(), TAIL_DISPOSE_MS);
  }

  dispose(): void {
    if (this._disposed) return;
    this._disposed = true;
    if (this._disposeTimer !== null) {
      clearTimeout(this._disposeTimer);
      this._disposeTimer = null;
    }
    try {
      this._wobble.stop();
    } catch {
      // Never started, or the context already stopped it.
    }
    for (const source of this._sources) this._stopSource(source);
    this._sources.length = 0;
    this._staticSource = null;
    for (const node of this._nodes) {
      try {
        node.disconnect();
      } catch {
        // Already disconnected.
      }
    }
  }

  /** Head click and static bed, once the decoded samples are available. */
  private _startSamples(): void {
    if (!this._samples || this._finished) return;
    this._playClick();
    this._startStatic();
  }

  /** Play the PTT click at the chain's output (head or tail). */
  private _playClick(): void {
    const buffer = this._samples?.click;
    if (!buffer || this._disposed) return;
    const source = this._ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(this._ctx.destination);
    source.start();
    this._sources.push(source);
  }

  /** Start the looping static bed under the speech. */
  private _startStatic(): void {
    const buffer = this._samples?.static;
    if (!buffer || this._disposed || this._staticSource) return;
    const source = this._ctx.createBufferSource();
    const gain = this._ctx.createGain();
    source.buffer = buffer;
    source.loop = true;
    gain.gain.value = STATIC_LEVEL;
    source.connect(gain);
    gain.connect(this._ctx.destination);
    source.start();
    this._staticSource = source;
    this._sources.push(source);
  }

  private _stopStatic(): void {
    if (!this._staticSource) return;
    this._stopSource(this._staticSource);
    this._staticSource = null;
  }

  private _stopSource(source: AudioBufferSourceNode): void {
    try {
      source.stop();
    } catch {
      // Already stopped.
    }
  }
}

/**
 * The live chain for an effect id, or null when the effect has no streaming
 * implementation (none, or a batch-only effect Rust keeps off the stream path).
 */
export function createLiveEffect(ctx: AudioContext, effectId: EffectId): LiveEffect | null {
  if (effectId !== "walkie_talkie") return null;
  return new WalkieTalkieLiveEffect(ctx);
}
