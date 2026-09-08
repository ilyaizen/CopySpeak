/**
 * Independent playback speed and pitch, built on SoundTouch's WSOLA stretcher.
 *
 * `speed` changes duration only (output duration = native / speed, pitch
 * untouched); `pitch` changes pitch only (duration untouched). Neither knob
 * leaks into the other, which is what plain `playbackRate` resampling cannot do.
 *
 * SoundTouch's `pitch` setter derives `transposer.rate = pitch` and
 * `stretch.tempo = 1 / pitch` - a pure pitch shift. Overriding `stretch.tempo`
 * with `speed / pitch` right after keeps that pitch shift and folds the wanted
 * duration change in: the transposer shortens by `pitch`, the stretch stage
 * then lengthens by `pitch / speed`, leaving duration = native / speed.
 */
import { SoundTouch } from "@soundtouchjs/core";

/** SoundTouch sample buffers are hard-coded to interleaved stereo frames. */
const SAMPLES_PER_FRAME = 2;

/** One channel of deinterleaved PCM, owning its buffer so it can back an AudioBuffer. */
export type PcmChannel = Float32Array<ArrayBuffer>;

/** Mono is duplicated into both frame slots; the second channel is dropped on the way out. */
function interleave(channels: PcmChannel[]): Float32Array {
  const left = channels[0];
  const right = channels[1] ?? left;
  const frames = left.length;
  const interleaved = new Float32Array(frames * SAMPLES_PER_FRAME);
  for (let frame = 0; frame < frames; frame++) {
    interleaved[frame * SAMPLES_PER_FRAME] = left[frame];
    interleaved[frame * SAMPLES_PER_FRAME + 1] = right[frame];
  }
  return interleaved;
}

function deinterleave(
  interleaved: Float32Array,
  frames: number,
  channelCount: number
): PcmChannel[] {
  const channels: PcmChannel[] = Array.from(
    { length: channelCount },
    () => new Float32Array(frames)
  );
  for (let frame = 0; frame < frames; frame++) {
    for (let c = 0; c < channelCount; c++) {
      channels[c][frame] = interleaved[frame * SAMPLES_PER_FRAME + c];
    }
  }
  return channels;
}

/**
 * Copy `parts` into a single channel set of exactly `length` frames, truncating
 * overflow and leaving any shortfall as silence.
 */
export function concatTrim(
  parts: (PcmChannel[] | null)[],
  length: number,
  channelCount: number
): PcmChannel[] {
  const channels: PcmChannel[] = Array.from(
    { length: channelCount },
    () => new Float32Array(length)
  );
  let written = 0;
  for (const part of parts) {
    if (!part || written >= length) continue;
    const take = Math.min(part[0].length, length - written);
    for (let c = 0; c < channelCount; c++) {
      channels[c].set(part[c].subarray(0, take), written);
    }
    written += take;
  }
  return channels;
}

/**
 * Stateful stretcher for sequentially pushed PCM chunks.
 *
 * Output frames trail input: WSOLA holds back up to one window, so {@link push}
 * returns `null` until enough has accumulated and {@link flush} is required at
 * the end of a fragment to avoid clipping its last word.
 */
export class TimeStretcher {
  private readonly _processor: SoundTouch;
  private readonly _channelCount: number;
  private _speed = 1;
  private _pitch = 1;
  /** Frames the current rate says should have been emitted for everything fed so far. */
  private _expectedFrames = 0;
  private _emittedFrames = 0;

  constructor(sampleRate: number, channelCount: number) {
    this._channelCount = Math.min(Math.max(channelCount, 1), 2);
    this._processor = new SoundTouch({ sampleRate, sampleBufferType: "fifo" });
    this.setRate(1, 1);
  }

  /** True when no processing is needed and chunks pass through untouched. */
  get isBypassed(): boolean {
    return this._speed === 1 && this._pitch === 1;
  }

  setRate(speed: number, pitch: number): void {
    this._speed = speed;
    this._pitch = pitch;
    this._processor.pitch = pitch;
    this._processor.stretch.tempo = speed / pitch;
  }

  /** Feed one chunk of native PCM; returns whatever output is ready, if any. */
  push(channels: PcmChannel[]): PcmChannel[] | null {
    const frames = channels[0]?.length ?? 0;
    if (frames === 0) return null;
    this._expectedFrames += frames / this._speed;
    if (this.isBypassed) {
      this._emittedFrames += frames;
      return channels;
    }
    this._processor.inputBuffer.putSamples(interleave(channels));
    this._processor.process();
    return this._drain();
  }

  /**
   * Push the trailing WSOLA window out with silence and reset. Output is capped
   * at the frame count the pushed audio is owed, so the padding never becomes
   * audible silence or drifts the caption clock.
   */
  flush(): PcmChannel[] | null {
    if (this.isBypassed) {
      this.reset();
      return null;
    }
    // ponytail: pad generously rather than model the transposer's frame ratio;
    // the owed-frames cap below discards whatever the padding over-produced.
    const padFrames = Math.max(1, this._processor.stretch.sampleReq * 4);
    this._processor.inputBuffer.putSamples(new Float32Array(padFrames * SAMPLES_PER_FRAME));
    this._processor.process();
    const drained = this._drain();
    const owed =
      Math.round(this._expectedFrames) - this._emittedFrames + (drained?.[0].length ?? 0);
    this.reset();
    if (!drained || owed <= 0) return null;
    if (owed >= drained[0].length) return drained;
    return drained.map((channel) => channel.slice(0, owed));
  }

  reset(): void {
    this._processor.clear();
    this._expectedFrames = 0;
    this._emittedFrames = 0;
  }

  private _drain(): PcmChannel[] | null {
    const frames = this._processor.outputBuffer.frameCount;
    if (frames === 0) return null;
    const interleaved = new Float32Array(frames * SAMPLES_PER_FRAME);
    this._processor.outputBuffer.extract(interleaved, 0, frames);
    this._processor.outputBuffer.receive(frames);
    this._emittedFrames += frames;
    return deinterleave(interleaved, frames, this._channelCount);
  }
}

/**
 * Whole-buffer variant for the non-streaming `<audio>` path. Output length is
 * exactly `round(buffer.length / speed)`, so caption positions stay in a clean
 * native-time relationship with the rendered audio.
 */
export function stretchBuffer(buffer: AudioBuffer, speed: number, pitch: number): AudioBuffer {
  const channelCount = Math.min(buffer.numberOfChannels, 2);
  const input: PcmChannel[] = Array.from({ length: channelCount }, (_, c) =>
    buffer.getChannelData(c)
  );
  const stretcher = new TimeStretcher(buffer.sampleRate, channelCount);
  stretcher.setRate(speed, pitch);
  const body = stretcher.push(input);
  const tail = stretcher.flush();
  const length = Math.max(1, Math.round(buffer.length / speed));
  const channels = concatTrim([body, tail], length, channelCount);
  const output = new AudioBuffer({
    length,
    numberOfChannels: channelCount,
    sampleRate: buffer.sampleRate
  });
  for (let c = 0; c < channelCount; c++) {
    output.copyToChannel(channels[c], c);
  }
  return output;
}
