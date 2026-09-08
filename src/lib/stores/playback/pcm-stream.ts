/**
 * PCM stream scheduler for streaming synthesis playback.
 *
 * Consumes raw PCM chunks forwarded by the backend as `audio-stream-chunk`
 * Tauri events and schedules them as sequential AudioBufferSourceNodes on the
 * shared AudioContext so speech starts while synthesis is still running
 * (~250ms prebuffer) and continues back-to-back without clicks or gaps.
 */

/** Payload of one `audio-stream-chunk` event (matches AudioStreamChunkEvent). */
export interface StreamChunkPayload {
  /** Base64-encoded raw PCM data (empty on the terminal end-of-stream event) */
  audio_base64: string;
  sample_rate: number;
  channels: number;
  bits_per_sample: number;
  /** Zero-based index of the fragment this chunk belongs to */
  fragment_index: number;
  /** Total number of fragments in this synthesis run */
  fragment_total: number;
  /** True only on the terminal zero-byte end-of-stream event */
  is_final: boolean;
  /** Caption metadata sent with the first chunk and at each fragment's end. */
  text?: string;
  fragment_duration_ms?: number;
}

interface ScheduledPosition {
  fragmentIndex: number;
  offset: number;
  duration: number;
  consumed: number;
  clock: number;
  rate: number;
}

export interface PcmStreamSchedulerOptions {
  ctx: AudioContext;
  destination: AudioNode;
  /** Invoked once when the whole streamed queue has finished playing. */
  onComplete: () => void;
}

/** Audio buffered before the first source node is scheduled. */
const PREBUFFER_SECONDS = 0.25;
/** Small safety offset so the first sample is not scheduled in the past. */
const START_DELAY_SECONDS = 0.03;
/**
 * Fallback queue-completion grace period after a fragment's terminal marker,
 * used when no explicit queue-complete signal arrives (single-shot speak_now
 * path emits no pagination:complete).
 */
const IDLE_COMPLETE_MS = 2000;

/** Decode standard base64 into bytes (mirrors the browser atob path). */
function base64ToBytes(base64: string, prefix: Uint8Array | null): Uint8Array {
  const binary = atob(base64);
  const offset = prefix?.length ?? 0;
  const bytes = new Uint8Array(offset + binary.length);
  if (prefix) bytes.set(prefix);
  for (let i = 0; i < binary.length; i++) {
    bytes[offset + i] = binary.charCodeAt(i);
  }
  return bytes;
}

/**
 * Convert little-endian 16-bit PCM samples to deinterleaved Float32 channels
 * normalized to [-1, 1]. Trailing bytes that do not form a full frame are
 * dropped.
 */
export function pcm16LeToFloat32Channels(
  bytes: Uint8Array,
  channels: number
): Float32Array<ArrayBuffer>[] {
  const bytesPerFrame = 2 * channels;
  const frames = Math.floor(bytes.length / bytesPerFrame);
  const result: Float32Array<ArrayBuffer>[] = [];
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let c = 0; c < channels; c++) {
    result.push(new Float32Array(frames));
  }
  for (let frame = 0; frame < frames; frame++) {
    for (let c = 0; c < channels; c++) {
      const offset = frame * bytesPerFrame + c * 2;
      result[c][frame] = view.getInt16(offset, true) / 32768;
    }
  }
  return result;
}

/**
 * Schedules streamed PCM chunks as gap-free sequential AudioBufferSourceNodes.
 *
 * Chunks accumulate until ~250ms is buffered, then playback starts; every
 * later chunk is scheduled at a running nextStartTime cursor that self-heals
 * to currentTime after an underrun. Volume routes through a GainNode; speed
 * and pitch map to source.playbackRate (same audible effect as the batch
 * path's element rate plus resampled pitch).
 */
export class PcmStreamScheduler {
  private readonly _ctx: AudioContext;
  private readonly _destination: AudioNode;
  private readonly _onComplete: () => void;

  private _gain: GainNode;
  private _pending: { buffer: AudioBuffer; fragmentIndex: number; offset: number }[] = [];
  private _pendingDuration = 0;
  private _activeSources = new Map<AudioBufferSourceNode, ScheduledPosition>();
  private _fragmentOffsets = new Map<number, number>();
  private _started = false;
  private _nextStartTime = 0;
  private _firstChunkAt: number | null = null;
  private _lastChunkAt: number | null = null;
  /** Transport boundaries need not coincide with a complete interleaved frame. */
  private _remainder: Uint8Array | null = null;

  private _volumePercent = 100;
  private _speed = 1.0;
  private _pitch = 1.0;

  /** No further chunks will arrive (queue-complete signal received). */
  private _finished = false;
  private _completed = false;
  private _pausedByUser = false;
  private _idleTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(options: PcmStreamSchedulerOptions) {
    this._ctx = options.ctx;
    this._destination = options.destination;
    this._onComplete = options.onComplete;
    this._gain = this._ctx.createGain();
    this._gain.gain.value = this._volumePercent / 100;
    this._gain.connect(this._destination);
  }

  /** True while this scheduler owns audible state (buffers or live sources). */
  isActive(): boolean {
    return this._activeSources.size > 0 || this._pending.length > 0;
  }

  /** Update output volume (0-100, same scale as the playback store). */
  setVolume(volumePercent: number): void {
    this._volumePercent = volumePercent;
    this._gain.gain.value = volumePercent / 100;
  }

  /**
   * Update speed and pitch. Applied immediately to already-scheduled sources;
   * the scheduling cursor compensates using the current combined rate.
   */
  setRate(speed: number, pitch: number): void {
    this._speed = speed;
    this._pitch = pitch;
    const rate = speed * pitch;
    for (const [source, position] of this._activeSources) {
      this._advancePosition(position);
      position.rate = rate;
      source.playbackRate.value = rate;
    }
  }

  /** AudioContext time freezes on pause; no progress is reported in an underrun. */
  getPlaybackPosition(): { fragmentIndex: number; positionMs: number } | null {
    for (const position of this._activeSources.values()) {
      this._advancePosition(position);
      if (this._ctx.currentTime >= position.clock && position.consumed < position.duration) {
        return {
          fragmentIndex: position.fragmentIndex,
          positionMs: (position.offset + position.consumed) * 1000
        };
      }
    }
    return null;
  }

  private _advancePosition(position: ScheduledPosition): void {
    const now = this._ctx.currentTime;
    position.consumed = Math.min(
      position.duration,
      position.consumed + Math.max(0, now - position.clock) * position.rate
    );
    position.clock = Math.max(position.clock, now);
  }

  /** Feed one decoded chunk event from the backend. */
  handleChunk(payload: StreamChunkPayload): void {
    if (payload.is_final) {
      this.handleFragmentEnd();
      return;
    }
    if (payload.fragment_duration_ms !== undefined && !payload.audio_base64) {
      // Flush short intermediate fragments without declaring the whole queue final.
      if (!this._started && this._pending.length) this._startPlayback();
      else this._schedulePending();
      this._remainder = null;
      return;
    }
    if (payload.bits_per_sample !== 16) {
      console.warn("[PcmStream] unsupported bits_per_sample:", payload.bits_per_sample);
      return;
    }
    if (!payload.audio_base64) return;
    const carriedBytes = this._remainder?.length ?? 0;
    let bytes: Uint8Array;
    try {
      bytes = base64ToBytes(payload.audio_base64, this._remainder);
    } catch {
      console.warn("[PcmStream] dropping chunk with malformed base64");
      return;
    }
    if (bytes.length === 0) return;
    this._clearIdleTimer();
    if (this._firstChunkAt === null) {
      this._firstChunkAt = performance.now();
    }

    const channels = payload.channels > 0 ? Math.min(payload.channels, 2) : 1;
    const remainderLength = bytes.length % (2 * channels);
    this._remainder = remainderLength === 0 ? null : bytes.slice(bytes.length - remainderLength);
    if (import.meta.env.DEV) {
      const now = performance.now();
      console.debug("[PcmStream] chunk", {
        bytes: bytes.length - carriedBytes,
        arrivalGapMs: this._lastChunkAt === null ? null : Math.round(now - this._lastChunkAt),
        bufferedMs: Math.round(
          1000 *
            (this._pendingDuration / this._combinedRate() +
              Math.max(0, this._nextStartTime - this._ctx.currentTime))
        ),
        carriedBytes: remainderLength
      });
      this._lastChunkAt = now;
    }
    const channelData = pcm16LeToFloat32Channels(bytes, channels);
    if (channelData[0].length === 0) return;

    const buffer = this._ctx.createBuffer(channels, channelData[0].length, payload.sample_rate);
    for (let c = 0; c < channels; c++) {
      buffer.copyToChannel(channelData[c], c);
    }

    const offset = this._fragmentOffsets.get(payload.fragment_index) ?? 0;
    this._fragmentOffsets.set(payload.fragment_index, offset + buffer.duration);
    this._pending.push({ buffer, fragmentIndex: payload.fragment_index, offset });
    this._pendingDuration += buffer.duration;

    if (!this._started && this._pendingDuration >= PREBUFFER_SECONDS) {
      this._startPlayback();
    } else if (this._started) {
      this._schedulePending();
    }
  }

  /** Terminal marker for one fragment: flush whatever is still buffered. */
  handleFragmentEnd(): void {
    if (this._remainder) {
      console.warn(
        "[PcmStream] discarding incomplete PCM frame at fragment end:",
        this._remainder.length,
        "bytes"
      );
      this._remainder = null;
    }
    if (!this._started && this._pending.length > 0) {
      this._startPlayback();
    } else {
      this._schedulePending();
    }
    // Fallback completion if no explicit queue-complete signal follows
    // (covers multi-fragment synthesis gaps and the speak_now path).
    this._clearIdleTimer();
    if (!this._finished) {
      this._idleTimer = setTimeout(() => {
        this._idleTimer = null;
        if (!this._pausedByUser) {
          this.markQueueComplete();
        }
      }, IDLE_COMPLETE_MS);
    }
  }

  /**
   * Authoritative end-of-synthesis signal (backend `pagination:complete`).
   * Completion fires once all scheduled sources have drained.
   */
  markQueueComplete(): void {
    this._clearIdleTimer();
    this._finished = true;
    if (!this.isActive()) {
      this._complete();
    }
  }

  /** Pause playback by suspending the shared AudioContext. */
  pause(): void {
    this._pausedByUser = true;
    if (this._ctx.state === "running") {
      void this._ctx.suspend();
    }
  }

  /** Resume playback previously paused via {@link pause}. */
  resume(): void {
    this._pausedByUser = false;
    if (this._ctx.state === "suspended") {
      void this._ctx.resume();
    }
  }

  /** Cancel all scheduled audio and reset internal state (no completion). */
  stop(): void {
    this._clearIdleTimer();
    for (const source of this._activeSources.keys()) {
      try {
        source.stop();
      } catch {
        // Already stopped or never started - nothing to do.
      }
    }
    this._activeSources.clear();
    this._fragmentOffsets.clear();
    this._pending = [];
    this._pendingDuration = 0;
    this._started = false;
    this._nextStartTime = 0;
    this._firstChunkAt = null;
    this._lastChunkAt = null;
    this._remainder = null;
    this._pausedByUser = false;
    try {
      this._gain.disconnect();
    } catch {
      // Already disconnected.
    }
    if (this._ctx.state === "suspended") {
      void this._ctx.resume();
    }
  }

  private _startPlayback(): void {
    this._started = true;
    this._nextStartTime = this._ctx.currentTime + START_DELAY_SECONDS;
    console.debug(
      "[PcmStream] first chunk scheduled after",
      Math.round(performance.now() - (this._firstChunkAt ?? 0)),
      "ms"
    );
    this._schedulePending();
  }

  private _combinedRate(): number {
    return this._speed * this._pitch;
  }

  private _schedulePending(): void {
    const rate = this._combinedRate();
    while (this._pending.length > 0) {
      const { buffer, fragmentIndex, offset } = this._pending.shift()!;
      this._pendingDuration -= buffer.duration;
      // Underrun tolerance: continue from currentTime if the cursor fell behind.
      const at = Math.max(this._nextStartTime, this._ctx.currentTime + 0.005);
      if (import.meta.env.DEV && at > this._nextStartTime) {
        console.debug("[PcmStream] underrun", {
          gapMs: Math.round(1000 * (at - this._nextStartTime))
        });
      }
      const source = this._ctx.createBufferSource();
      source.buffer = buffer;
      source.playbackRate.value = rate;
      source.connect(this._gain);
      source.onended = () => this._handleSourceEnded(source);
      source.start(at);
      this._activeSources.set(source, {
        fragmentIndex,
        offset,
        duration: buffer.duration,
        consumed: 0,
        clock: at,
        rate
      });
      this._nextStartTime = at + buffer.duration / rate;
    }
  }

  private _handleSourceEnded(source: AudioBufferSourceNode): void {
    this._activeSources.delete(source);
    if (this._finished && !this.isActive()) {
      this._complete();
    }
  }

  private _complete(): void {
    if (this._completed) return;
    this._completed = true;
    this._onComplete();
  }

  private _clearIdleTimer(): void {
    if (this._idleTimer !== null) {
      clearTimeout(this._idleTimer);
      this._idleTimer = null;
    }
  }
}
