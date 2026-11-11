/**
 * Global Playback Store
 *
 * Single source of truth for audio playback state across all routes.
 * The GlobalPlayer component mounts the <audio> element and calls
 * setAudioElement() + setupListeners() on mount.
 *
 * Supports streaming pagination playback: fragments are queued and played
 * sequentially as they arrive from the backend.
 */

import { isTauri } from "#lib/services/tauri.js";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "svelte-sonner";
import type { AppConfig, EffectId } from "#lib/types";
import { effectiveEffect } from "#lib/effects-gate";
import { applyFadeIn, audioBufferToWavBlob, detectAudioMimeType } from "./playback/audio-utils.js";
import { AudioAnalyser, type EmitPayload } from "./playback/analyser.js";
import { PcmStreamScheduler, type StreamChunkPayload } from "./playback/pcm-stream.js";
import { stretchBuffer } from "./playback/time-stretch.js";
import { getEffect } from "./playback/effects/registry.js";
import { createLiveEffect, type LiveEffect } from "./playback/effects/live.js";
import { FragmentQueue, type QueuedFragment } from "./playback/fragment-queue.js";
import { hudStore } from "./hud-store.svelte.js";
import { validCaptionAlignment, type CaptionAlignment } from "#lib/models/captions.js";
import type { HudCaptionPayload } from "#lib/types/hud.js";

/** Wall seconds the skip buttons and arrow keys move by. */
export const SKIP_SECONDS = 5;

/** Key for held chunks with no reading identity (older build, helper scopes). */
const ANONYMOUS_STREAM_READING = 0;

/** Most readings whose streamed audio may wait for the speakers at once. */
const MAX_HELD_STREAM_RUNS = 4;

/** Most base64 characters of streamed audio that may wait for the speakers. */
const MAX_HELD_STREAM_BYTES = 32 * 1024 * 1024;

/** Most dropped, refused or failed reading ids remembered to reject late chunks. */
const MAX_IGNORED_STREAM_READINGS = 64;

/** One reading's streamed chunks, held while another reading owns the speakers. */
interface HeldStreamRun {
  /** Chunks in arrival order, including the terminal zero-byte end marker. */
  chunks: StreamChunkPayload[];
  /** Whether this reading's `pagination:complete` arrived. */
  complete: boolean;
  /** Sum of `audio_base64.length` over `chunks`, for `MAX_HELD_STREAM_BYTES`. */
  bytes: number;
}

/** Payload of `pagination:complete` (matches `PaginationEvent`). */
interface PaginationCompletePayload {
  reading_id?: number;
}

/** Payload of `playback-stream-failed` (matches `PlaybackStreamFailedEvent`). */
interface StreamFailedPayload {
  session_id: string | null;
  reading_id?: number;
}

/** Payload of `reading-failed` (matches the Rust failure event). */
interface ReadingFailedPayload {
  message: string;
  reading_id?: number;
}

/**
 * The one owner of the speakers' streamed audio (M5): the active scheduler with
 * its reading and browser session, plus every other reading's held run. All
 * ownership checks route through `accept`, `complete`, `fail` and `release`, so
 * a new feed path cannot mix two readings without passing a check.
 */
class StreamOwner {
  private _scheduler: PcmStreamScheduler | null = null;
  private _readingId: number | null = null;
  private _sessionId: string | null = null;
  private readonly _holds = new Map<number, HeldStreamRun>();
  /** Total `audio_base64.length` over every held run. */
  private _heldBytes = 0;
  /**
   * Identified readings that were dropped, refused or failed. A chunk emitted
   * just before the Rust cancel can reach the webview after the drop event; it
   * must not start or extend that reading again. The anonymous key is never
   * recorded: the stop gate drops id-less chunks while stopped, and the old
   * build keeps its fallback. Bounded because reading ids are never reused.
   */
  private readonly _ignored = new Set<number>();
  /** Set after the first id-less chunk that cannot feed the identified scheduler. */
  private _anonymousWarned = false;

  get scheduler(): PcmStreamScheduler | null {
    return this._scheduler;
  }

  get readingId(): number | null {
    return this._readingId;
  }

  get sessionId(): string | null {
    return this._sessionId;
  }

  /** True while a streamed scheduler owns the speakers. */
  ownsSpeakers(): boolean {
    return this._scheduler !== null;
  }

  /** True when `readingId` names the reading audible through the scheduler. */
  isAudible(readingId: number | null): boolean {
    return this._scheduler !== null && readingId !== null && readingId === this._readingId;
  }

  /** True when a held run waits for the speakers for that reading. */
  hasHeld(readingId: number | null): boolean {
    return this._holds.has(readingId ?? ANONYMOUS_STREAM_READING);
  }

  /** True when any reading waits for the speakers. */
  hasHeldRuns(): boolean {
    return this._holds.size > 0;
  }

  /**
   * Remember an identified reading so a late chunk cannot restart it or hold it
   * again. The anonymous key is never recorded. The set keeps the newest ids and
   * drops the oldest, which is safe because a reading id is never reused.
   */
  ignore(readingId: number | null): void {
    if (readingId === null || readingId === ANONYMOUS_STREAM_READING) return;
    this._ignored.delete(readingId);
    this._ignored.add(readingId);
    while (this._ignored.size > MAX_IGNORED_STREAM_READINGS) {
      const oldest = this._ignored.values().next().value;
      if (oldest === undefined) break;
      this._ignored.delete(oldest);
    }
  }

  /** Record the scheduler that just took the speakers for a reading. */
  start(scheduler: PcmStreamScheduler, readingId: number | null, sessionId: string | null): void {
    this._scheduler = scheduler;
    this._readingId = readingId;
    this._sessionId = sessionId;
    this._anonymousWarned = false;
  }

  /** Stop the scheduler and forget which reading owned the speakers. */
  release(): number | null {
    const readingId = this._readingId;
    this._scheduler?.stop();
    this._scheduler = null;
    this._readingId = null;
    this._sessionId = null;
    return readingId;
  }

  /**
   * Route one arriving chunk. Returns true when the store may feed it to the
   * scheduler now; false when it waits in a held run, or a cap dropped it.
   * `batchOwnsSpeakers` is the store's cross-engine hold: streamed PCM waits
   * while the batch fragment path plays.
   */
  accept(chunk: StreamChunkPayload, readingId: number | null, batchOwnsSpeakers: boolean): boolean {
    // A late chunk of a dropped, refused or failed reading must neither start a
    // scheduler nor wait in a held run. Anonymous chunks are exempt on purpose.
    if (readingId !== null && this._ignored.has(readingId)) return false;
    if (this._scheduler !== null) {
      // Only the scheduler's own reading may feed it. An id-less chunk cannot
      // join an identified stream (M2), so hold it under the anonymous key.
      if (readingId === this._readingId) return true;
      if (readingId === null) this._warnAnonymous();
      this._hold(chunk, readingId);
      return false;
    }
    if (batchOwnsSpeakers) {
      if (readingId === null) this._warnAnonymous();
      this._hold(chunk, readingId);
      return false;
    }
    return true;
  }

  /**
   * Handle `pagination:complete`. An id-less completion ends only an anonymous
   * scheduler (M2); an identified one completes its own scheduler or marks its
   * held run. Unknown ids are ignored.
   */
  complete(readingId: number | null): void {
    if (readingId === null) {
      if (this._scheduler !== null && this._readingId === null) {
        this._scheduler.markQueueComplete();
      }
      return;
    }
    if (this._scheduler !== null && readingId === this._readingId) {
      this._scheduler.markQueueComplete();
      return;
    }
    const held = this._holds.get(readingId);
    if (held) held.complete = true;
  }

  /**
   * Handle a failed streamed reading. Returns true when the failure belongs to
   * the audible reading, so the store stops only that reading (B3). A waiting
   * reading's failure drops its held run; an id-less failure keeps today's
   * behavior (session match on the owner, clear every hold when idle).
   */
  fail(readingId: number | null, sessionId: string | null): boolean {
    if (this._scheduler !== null) {
      const owns =
        readingId !== null ? readingId === this._readingId : sessionId === this._sessionId;
      if (owns) return true;
      if (readingId !== null) this.drop(readingId);
      return false;
    }
    if (readingId === null) this.clearHolds();
    else this.drop(readingId);
    return false;
  }

  /** Forget one waiting reading's held run (scoped stop, attributed failure). */
  drop(readingId: number | null): void {
    // A late chunk emitted before the cancel can land after this drop: remember
    // the reading so it cannot start a scheduler or a fresh held run.
    this.ignore(readingId);
    const key = readingId ?? ANONYMOUS_STREAM_READING;
    const run = this._holds.get(key);
    if (run === undefined) return;
    this._holds.delete(key);
    this._heldBytes = Math.max(0, this._heldBytes - run.bytes);
    console.log(
      "[PlaybackStore] dropping held stream reading",
      readingId,
      "chunks:",
      run.chunks.length
    );
  }

  /** Forget every held run; every path that cuts all playback uses it. */
  clearHolds(): void {
    this._holds.clear();
    this._heldBytes = 0;
    this._ignored.clear();
  }

  /**
   * Take the waiting run with the lowest reading id that can replay (M7).
   * Reading ids are allocated in start order, so the lowest id is the oldest
   * reading. The anonymous key (0) is a sentinel, so an anonymous run drains
   * before identified ids when both wait. A run without a terminal chunk or
   * `pagination:complete` is dropped: replaying a partial run would play and
   * never complete, and would hold the speakers forever (M1). The run is
   * deleted before the caller replays it.
   */
  takeNext(): { readingId: number; run: HeldStreamRun } | null {
    while (this._holds.size > 0) {
      let chosen: number | null = null;
      for (const key of this._holds.keys()) {
        if (chosen === null || key < chosen) chosen = key;
      }
      if (chosen === null) return null;
      const run = this._holds.get(chosen);
      if (run === undefined) return null;
      this._holds.delete(chosen);
      this._heldBytes = Math.max(0, this._heldBytes - run.bytes);
      // The terminal chunk stays in the payload; do not keep a duplicate flag.
      const terminal = run.chunks.some((chunk) => chunk.is_final);
      if (terminal || run.complete) return { readingId: chosen, run };
      console.warn(
        "[PlaybackStore] dropping held stream reading",
        chosen === ANONYMOUS_STREAM_READING ? null : chosen,
        "with no terminal chunk or pagination:complete"
      );
    }
    return null;
  }

  /** Hold one chunk under its reading, enforcing both caps (B2). */
  private _hold(chunk: StreamChunkPayload, readingId: number | null): void {
    // The called-from `accept` checks this too; keep it here so every hold path
    // rejects a late chunk of a dropped, refused or failed reading.
    if (readingId !== null && this._ignored.has(readingId)) return;
    const key = readingId ?? ANONYMOUS_STREAM_READING;
    const bytes = chunk.audio_base64.length;
    let run = this._holds.get(key);
    if (run === undefined) {
      run = { chunks: [], complete: false, bytes: 0 };
      this._holds.set(key, run);
    }
    // A reading whose own run breaks the per-reading cap can never fit.
    if (run.bytes + bytes > MAX_HELD_STREAM_BYTES) {
      this._refuse(key, run);
      return;
    }
    // Make room by dropping the oldest waiting run, never this one. Repeat until
    // both caps hold; refuse the arriving reading when nothing else is left.
    while (
      this._holds.size > MAX_HELD_STREAM_RUNS ||
      this._heldBytes + bytes > MAX_HELD_STREAM_BYTES
    ) {
      const victim = this._oldestKeyExcept(key);
      if (victim === null) {
        this._refuse(key, run);
        return;
      }
      this._dropOldest(victim);
    }
    run.chunks.push(chunk);
    run.bytes += bytes;
    this._heldBytes += bytes;
  }

  /** Refuse one reading: drop its run and ignore its later chunks. */
  private _refuse(key: number, run: HeldStreamRun): void {
    this._holds.delete(key);
    this._heldBytes = Math.max(0, this._heldBytes - run.bytes);
    this.ignore(key === ANONYMOUS_STREAM_READING ? null : key);
    console.warn(
      "[PlaybackStore] refusing held stream reading",
      key === ANONYMOUS_STREAM_READING ? null : key,
      "over the held-audio cap"
    );
  }

  /** Drop one held run the caps demand, with a log line naming the reason. */
  private _dropOldest(key: number): void {
    const run = this._holds.get(key);
    if (run === undefined) return;
    this._holds.delete(key);
    this._heldBytes = Math.max(0, this._heldBytes - run.bytes);
    console.warn(
      "[PlaybackStore] dropping oldest held stream reading",
      key === ANONYMOUS_STREAM_READING ? null : key,
      "to keep the held-buffer cap"
    );
  }

  /** Lowest held reading id other than `current`, or null when none is left. */
  private _oldestKeyExcept(current: number): number | null {
    let oldest: number | null = null;
    for (const key of this._holds.keys()) {
      if (key !== current && (oldest === null || key < oldest)) oldest = key;
    }
    return oldest;
  }

  /** Warn once per streamed reading that an id-less chunk waits separately. */
  private _warnAnonymous(): void {
    if (this._anonymousWarned) return;
    this._anonymousWarned = true;
    console.warn(
      "[PlaybackStore] holding an id-less stream chunk apart from the identified scheduler"
    );
  }
}

class PlaybackStore {
  isPlaying = $state(false);
  isPaused = $state(false);
  isSynthesizing = $state(false);
  isLoadingAudio = $state(false);
  error = $state<string | null>(null);
  hasCachedAudio = $state(false);
  // Retained after stop/completion so the owning history row can offer Replay.
  historyReadingId = $state<string | null>(null);
  // Audible fragment's text, caption timings and clock; null when nothing plays.
  caption = $state.raw<HudCaptionPayload | null>(null);

  // Pagination state for HUD display
  currentFragmentIndex = $state<number | null>(null);
  totalFragments = $state<number | null>(null);

  // Synced from config by whoever has it loaded (synthesize-page or global-player)
  pitch = $state(1.0);
  volume = $state(100);
  speed = $state(1.0);
  activeEffect = $state<EffectId>("none");

  private _audioEl: HTMLAudioElement | null = null;
  private _audioCtx: AudioContext | null = null;
  private _decodedBuffer: AudioBuffer | null = null;
  private _originalBytes: ArrayBuffer | null = null;
  private _cachedPitchUrl: { ratio: number; effectId: EffectId; url: string } | null = null;
  // The registration in flight or done. Teardown takes it before it resolves, so a
  // teardown that lands mid-setup (HMR remounting GlobalPlayer) still unlistens
  // what that setup registers, and the next setup registers afresh.
  private _listeners: Promise<Array<() => void>> | null = null;
  private _emit: ((name: string, payload: EmitPayload) => Promise<void>) | null = null;
  private _emitTo: ((target: string, name: string, payload: EmitPayload) => Promise<void>) | null =
    null;
  private _stopping = false;
  private _playbackGeneration = 0;
  private _readingText = "";
  /** Failure message for the current reading; retained until the next starts. */
  private _readingFailure: string | null = null;
  private _fragmentCaptions: CaptionAlignment | null = null;
  private _streamStopped = false;
  /** Reading the last stop stopped; late chunks of this id stay dropped (B1). */
  private _stoppedReadingId: number | null = null;
  private _fragmentText = "";
  private _captionTimer: ReturnType<typeof setInterval> | null = null;
  private _lastCaption: HudCaptionPayload | null = null;
  // Wall-clock time of the last caption report for the control server's
  // `GET /status`; the caption clock ticks every 25 ms.
  private _lastCaptionReport = 0;
  private _streamCaptions = new Map<
    number,
    {
      text: string;
      durationMs: number;
      captions?: CaptionAlignment | null;
      sessionId?: string | null;
    }
  >();

  // Modular components
  private _analyser = new AudioAnalyser();
  private _fragmentQueue: FragmentQueue;
  // Cross-engine queue and stream-vs-stream queue: a reading's audio waits while
  // another reading owns the speakers. Streamed chunks buffer in that reading's
  // own held run inside the owner, so two readings never share one scheduler;
  // batch fragments already wait in `_fragmentQueue` until the stream drains.
  // `_drainHeldAudio` hands the speakers over, so at most one engine plays.
  private _owner = new StreamOwner();
  // Live walkie-talkie chain the streamed PCM plays through; null for a reading
  // with no effect, or with an effect Rust keeps on the batch path.
  private _liveEffect: LiveEffect | null = null;

  // Browser companion: while a page follows this audio (a browser-initiated
  // reading or a mirrored double-copy/hotkey speak), the audible-clock position
  // is reported to the Rust bridge, which projects the current word into the
  // page selection (see src-tauri/src/browser_bridge.rs). The binding is the
  // session of the clip now audible (set as each clip activates), never the
  // last session announced: a queued speak's announcement lands while an
  // earlier clip still plays, and its completion must not fire on that drain.
  private _browserSessionId: string | null = null;
  // Stream fragment entry last announced as audible (identity, not index: the
  // same index recurs across speaks).
  private _announcedStream: object | null = null;

  // `playback-started` fires once per run, when audio first becomes audible; the
  // Rust side ducks other apps on it and restores on `playback-finished`.
  private _startedEmitted = false;

  constructor() {
    // Initialize fragment queue with handlers
    this._fragmentQueue = new FragmentQueue({
      onFragmentPlay: (fragment: QueuedFragment) => this._activateFragment(fragment),
      onQueueComplete: () => {
        console.log("[PlaybackStore] onQueueComplete");
        this.finishPlayback();
      }
    });
  }

  /** Point the audible pipeline at a fragment: caption state, decode, render, play. */
  private _activateFragment(fragment: QueuedFragment): Promise<void> {
    console.log("[PlaybackStore] activating fragment:", fragment.index, "of", fragment.total);
    this.currentFragmentIndex = fragment.index;
    this.totalFragments = fragment.total;
    this._fragmentText = fragment.text;
    this._fragmentCaptions = validCaptionAlignment(fragment.captions) ? fragment.captions : null;
    this._browserSessionId = fragment.sessionId ?? null;
    void this._emit?.("playback-fragment-audible", { index: fragment.index, text: fragment.text });
    return this.handleAudioReady(fragment.audioBase64);
  }

  setAudioElement(el: HTMLAudioElement | null) {
    this._audioEl = el;
    if (el) {
      el.onplay = () => {
        // The element path is the batch path: no streamed reading owns this run.
        this.emitPlaybackStarted(null);
        this.isPlaying = true;
        this.isPaused = false;
        this.startCaptionClock();
      };
      el.onpause = () => {
        if (this._stopping) return;
        this.isPaused = !el.ended;
        this.isPlaying = !el.ended;
      };
      el.onended = () => {
        this._fragmentQueue.handleFragmentEnded();
      };
    }
  }

  private startCaptionClock() {
    if (this._captionTimer === null) {
      this._captionTimer = setInterval(() => this.publishCaption(), 25);
    }
    this.publishCaption();
  }

  private stopCaptionClock() {
    if (this._captionTimer !== null) clearInterval(this._captionTimer);
    this._captionTimer = null;
    this._lastCaption = null;
    this.caption = null;
    this._streamCaptions.clear();
    // One inactive report keeps the status route fresh for its one-second
    // window after a stop, so a poll does not show the last live snapshot.
    void invoke("report_playback_caption", { caption: { active: false } }).catch(() => {});
  }

  private publishCaption() {
    let caption: HudCaptionPayload | null = null;
    const scheduler = this._owner.scheduler;
    if (scheduler) {
      const position = scheduler.getPlaybackPosition();
      const fragment = position ? this._streamCaptions.get(position.fragmentIndex) : null;
      if (position && fragment) {
        if (fragment !== this._announcedStream) {
          this._announcedStream = fragment;
          this._browserSessionId = fragment.sessionId ?? null;
          void this._emit?.("playback-fragment-audible", {
            index: position.fragmentIndex,
            text: fragment.text
          });
        }
        caption = {
          text: fragment.captions?.text ?? fragment.text,
          captions: fragment.captions,
          position_ms: position.positionMs,
          duration_ms: fragment.durationMs,
          paused: this.isPaused,
          active: true,
          fragment_index: position.fragmentIndex
        };
      } else if (this._lastCaption) {
        // A suspended context reports no position while the playhead stalls
        // between sources, so a paused reading would look finished: keep the last
        // frozen position and stay active, or GET /status cannot tell the two apart.
        caption = { ...this._lastCaption, active: this.isPaused, paused: this.isPaused };
      }
    } else if (this._audioEl && this.isPlaying) {
      const el = this._audioEl;
      const duration = Number.isFinite(el.duration)
        ? el.duration
        : (this._decodedBuffer?.duration ?? 0);
      // The element plays through the AudioContext (analyser), and its media clock
      // leads the speakers by the context's output latency, in wall seconds.
      const latency = Math.max(0, this._audioCtx?.outputLatency || 0) * el.playbackRate;
      caption = {
        text: this._fragmentCaptions?.text ?? (this._fragmentText || this._readingText),
        captions: this._fragmentCaptions,
        position_ms: Math.max(0, el.currentTime - latency) * 1000,
        duration_ms: duration * 1000,
        paused: this.isPaused,
        active: !this.isLoadingAudio && !el.ended && !el.seeking && el.readyState >= 2,
        fragment_index: this.currentFragmentIndex ?? undefined
      };
    }
    if (!caption) return;
    // Report to the control server at most every ~100 ms: the caption clock
    // ticks at 25 ms and every report crosses the Tauri boundary. Best-effort:
    // an older build without the command ignores it.
    const now = Date.now();
    if (now - this._lastCaptionReport >= 100) {
      this._lastCaptionReport = now;
      void invoke("report_playback_caption", { caption }).catch(() => {});
    }
    this._lastCaption = caption;
    this.caption = caption;
    hudStore.handleCaption(caption);
    void this._emitTo?.("hud", "hud:caption", caption);
    // Report the audible clock to the browser bridge. Best-effort: the bridge
    // may not exist (older app build) or no reading may be active.
    if (this._browserSessionId !== null) {
      const fragmentIndex = scheduler
        ? (scheduler.getPlaybackPosition()?.fragmentIndex ?? this.currentFragmentIndex ?? 0)
        : (this.currentFragmentIndex ?? 0);
      void invoke("browser_reading_progress", {
        sessionId: this._browserSessionId,
        fragmentIndex,
        positionMs: Math.round(caption.position_ms),
        paused: caption.paused
      }).catch(() => {});
    }
  }

  async buildPlaybackUrl(pitchRatio: number): Promise<string> {
    const generation = this._playbackGeneration;
    const effectId = this.activeEffect;
    if (
      this._cachedPitchUrl &&
      this._cachedPitchUrl.ratio === pitchRatio &&
      this._cachedPitchUrl.effectId === effectId
    ) {
      return this._cachedPitchUrl.url;
    }
    if (this._cachedPitchUrl) {
      URL.revokeObjectURL(this._cachedPitchUrl.url);
      this._cachedPitchUrl = null;
    }
    const effect = getEffect(effectId);
    let blob: Blob;
    if (pitchRatio === 1.0 && !effect && this._originalBytes) {
      const mimeType = detectAudioMimeType(this._originalBytes);
      blob = new Blob([this._originalBytes], { type: mimeType });
    } else if (this._decodedBuffer && this._audioCtx) {
      let buffer: AudioBuffer;
      if (pitchRatio === 1.0) {
        // Copy to avoid mutating cached decoded buffer when applying fade-in
        buffer = new AudioBuffer({
          length: this._decodedBuffer.length,
          numberOfChannels: this._decodedBuffer.numberOfChannels,
          sampleRate: this._decodedBuffer.sampleRate
        });
        for (let c = 0; c < this._decodedBuffer.numberOfChannels; c++) {
          buffer.copyToChannel(this._decodedBuffer.getChannelData(c), c);
        }
      } else {
        // Pitch shift only - duration stays native so caption positions map 1:1.
        buffer = stretchBuffer(this._decodedBuffer, 1, pitchRatio);
      }
      // Fade the speech before the effect: an effect may open with its own onset.
      applyFadeIn(buffer, 10);
      if (effect) {
        buffer = await effect.process(buffer, this._audioCtx);
      }
      blob = audioBufferToWavBlob(buffer);
    } else {
      return "";
    }
    if (generation !== this._playbackGeneration) return "";
    const url = URL.createObjectURL(blob);
    this._cachedPitchUrl = { ratio: pitchRatio, effectId, url };
    return url;
  }

  async handleAudioReady(base64: string): Promise<void> {
    const generation = this._playbackGeneration;
    this.isLoadingAudio = true;
    this.error = null;
    this.hasCachedAudio = false;
    try {
      console.log("[PlaybackStore] handleAudioReady called, base64 length:", base64.length);
      const binary = atob(base64);
      const arrayBuffer = new ArrayBuffer(binary.length);
      const bytes = new Uint8Array(arrayBuffer);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }

      this._originalBytes = arrayBuffer.slice(0);
      if (this._cachedPitchUrl) {
        URL.revokeObjectURL(this._cachedPitchUrl.url);
        this._cachedPitchUrl = null;
      }

      if (!this._audioCtx) {
        this._audioCtx = new AudioContext();
      }

      // Resume AudioContext if suspended (required on clean Windows 11 / strict autoplay policies)
      if (this._audioCtx.state === "suspended") {
        await this._audioCtx.resume();
      }
      if (generation !== this._playbackGeneration) return;

      // Wire AnalyserNode once per audio element (guard prevents double-wiring)
      if (this._audioEl && !this._analyser.getAnalyser()) {
        this._analyser.setup(this._audioEl, this._audioCtx, {
          emitTo: this._emitTo
        });
      }

      const decoded = await this._audioCtx.decodeAudioData(arrayBuffer.slice(0));
      if (generation !== this._playbackGeneration) return;
      this._decodedBuffer = decoded;
      if (this._decodedBuffer) {
        const accurateDurationMs = Math.round(this._decodedBuffer.duration * 1000);
        hudStore.setAccurateDurationMs(accurateDurationMs);
        // Emit to HUD window for cross-window state sync
        void this._emit?.("hud:audio-duration", accurateDurationMs);
      }
      const url = await this.buildPlaybackUrl(this.pitch);
      if (generation !== this._playbackGeneration) return;
      if (!this._audioEl || !url) throw new Error("Audio player is not ready");
      this._audioEl.src = url;
      this._analyser.start(); // Start amplitude capture BEFORE audio plays
      await this.playAudio();
      if (generation !== this._playbackGeneration) return;
      this.hasCachedAudio = true;
    } catch (e) {
      if (generation !== this._playbackGeneration) return;
      this.handleStop();
      this.error = `Audio playback failed: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      if (generation === this._playbackGeneration) this.isLoadingAudio = false;
    }
  }

  /**
   * Handle incoming audio fragment from backend.
   * Queues the fragment and starts processing if not already.
   */
  async handleFragmentReady(payload: {
    audio_base64: string;
    fragment_index: number;
    fragment_total: number;
    is_final: boolean;
    text: string;
    captions?: CaptionAlignment | null;
    session_id?: string | null;
  }): Promise<void> {
    console.log(
      "[PlaybackStore] handleFragmentReady: index",
      payload.fragment_index,
      "total",
      payload.fragment_total,
      "is_final",
      payload.is_final
    );
    // Add to queue
    this._fragmentQueue.enqueue({
      audioBase64: payload.audio_base64,
      index: payload.fragment_index,
      total: payload.fragment_total,
      text: payload.text,
      captions: payload.captions,
      sessionId: payload.session_id ?? null
    });
    console.log(
      "[PlaybackStore] Queue length:",
      this._fragmentQueue.getQueueLength(),
      "isProcessing:",
      this._fragmentQueue.isProcessing()
    ); // Start processing if not already
    // Cross-engine queue: while a streamed reading owns the speakers, these
    // fragments wait in the queue and start when the stream drains.
    if (!this._fragmentQueue.isProcessing() && !this._streamOwnsSpeakers()) {
      await this._fragmentQueue.startProcessing();
    }
  }

  /**
   * Shared queue-completion path for batch fragments and streamed PCM chunks:
   * stops the analyser, resets playback state, hides the HUD.
   */
  private finishPlayback(): void {
    this.stopCaptionClock();
    this._analyser.stop();
    // Natural end: the live chain plays its tail click and disposes itself.
    this._finishLiveEffect();
    this.isPlaying = false;
    this.isPaused = false;
    this.currentFragmentIndex = null;
    this.totalFragments = null;
    // Capture the reading that owned the stream before ownership clears: Rust
    // releases the speaker owner for that reading on `playback-finished` (B1).
    const readingId = this._owner.release();
    // A reading that hit reading-failed ends in "error"; a normal one in
    // "completed". The bridge keeps a delivered session up for this report.
    this.reportBrowserFinished(this._readingFailure === null ? "completed" : "error");
    void this._emit?.("hud:stop", null);
    this.emitPlaybackFinished(readingId);
    // The speakers are free: start whatever waited for this engine to drain.
    this._drainHeldAudio();
  }

  /** True while the batch fragment path owns (or is taking) the speakers. */
  private _batchOwnsSpeakers(): boolean {
    return this._fragmentQueue.isProcessing() || this.isLoadingAudio;
  }

  /** True while the streamed scheduler owns the speakers. */
  private _streamOwnsSpeakers(): boolean {
    return this._owner.ownsSpeakers();
  }

  /**
   * Hand the speakers to the reading that waited while the other engine played:
   * the oldest buffered stream starts, else the queued batch fragments. At most
   * one reading plays at a time, so a new reading never doubles the audio.
   * Streamed runs keep priority over queued batch fragments because streamed
   * PCM cannot be regenerated, while a batch fragment can replay (M7).
   */
  private _drainHeldAudio(): void {
    if (this._streamStopped) {
      this._owner.clearHolds();
      return;
    }
    // Delete the run before replaying: the first chunk starts the scheduler this
    // reading now owns, and the stream's later chunks feed it live.
    const next = this._owner.takeNext();
    if (next !== null) {
      console.log(
        "[PlaybackStore] draining held stream reading",
        next.readingId === ANONYMOUS_STREAM_READING ? null : next.readingId,
        "chunks:",
        next.run.chunks.length,
        "final:",
        next.run.chunks.some((chunk) => chunk.is_final)
      );
      for (const chunk of next.run.chunks) this.handleStreamChunk(chunk);
      // The terminal chunk completes the scheduler on its own; applying the
      // explicit completion covers a reading that outran its end marker.
      if (next.run.complete) this._owner.scheduler?.markQueueComplete();
      return;
    }
    if (!this._fragmentQueue.isProcessing() && this._fragmentQueue.getQueueLength() > 0) {
      void this._fragmentQueue.startProcessing();
    }
  }

  /** The reading id on a playback event; null when absent (helper scope, old build). */
  private _eventReadingId(readingId: number | null | undefined): number | null {
    return readingId === undefined || readingId === null || readingId === 0 ? null : readingId;
  }

  /**
   * The backend finished synthesizing a reading. An id-less completion ends only
   * the anonymous stream (M2); an identified one completes its own scheduler or
   * marks its held run (see `StreamOwner.complete`).
   */
  private handlePaginationComplete(payload: PaginationCompletePayload | null): void {
    this._owner.complete(this._eventReadingId(payload?.reading_id));
  }

  /** Hand a naturally finished live chain its tail and let it self-dispose. */
  private _finishLiveEffect(): void {
    const effect = this._liveEffect;
    if (!effect) return;
    this._liveEffect = null;
    effect.finish();
  }

  /** Cut the live chain at once (user stop, teardown); no tail click. */
  private _disposeLiveEffect(): void {
    const effect = this._liveEffect;
    if (!effect) return;
    this._liveEffect = null;
    effect.dispose();
  }

  /** Tell the bridge the followed audio ended, so the page clears its highlight. */
  private reportBrowserFinished(status: "completed" | "cancelled" | "error"): void {
    const sessionId = this._browserSessionId;
    if (sessionId === null) return;
    this._browserSessionId = null;
    void invoke("browser_reading_finished", { sessionId, status }).catch(() => {});
  }

  /**
   * Handle one PCM chunk of streaming synthesis audio. Lazily creates the
   * scheduler (which prebuffers ~250ms before starting gap-free playback) and
   * feeds it the chunk. A chunk of another reading waits in that reading's own
   * held run: only the reading that owns the scheduler may feed it (M2, M5).
   */
  handleStreamChunk(payload: StreamChunkPayload): void {
    const readingId = this._eventReadingId(payload.reading_id);
    if (this._streamStopped) {
      // Late chunks cannot restart a stopped reading (B1). An id-less chunk
      // stays dropped; a chunk of the stopped reading stays dropped; a chunk of
      // any other reading heals the gate, replacing Rust's old StreamRearm.
      if (readingId === null || readingId === this._stoppedReadingId) return;
      this._streamStopped = false;
      this._stoppedReadingId = null;
    }
    // Stream-vs-stream, anonymous separation and the cross-engine hold all live
    // in the owner; only a chunk it accepts reaches the scheduler.
    if (!this._owner.accept(payload, readingId, this._batchOwnsSpeakers())) return;
    this._playStreamChunk(payload, readingId);
  }

  /** Start the scheduler for a chunk it accepted, and feed the chunk. */
  private _playStreamChunk(payload: StreamChunkPayload, readingId: number | null): void {
    if (!this._audioCtx) {
      this._audioCtx = new AudioContext();
    }
    const ctx = this._audioCtx;
    // Resume AudioContext if suspended (autoplay policies, or a paused stream)
    if (ctx.state === "suspended" && !this.isPaused) {
      void ctx.resume();
    }
    let scheduler = this._owner.scheduler;
    if (scheduler === null) {
      console.log("[PlaybackStore] creating PCM scheduler for fragment", payload.fragment_index);
      // The lag fix: walkie_talkie streams through a live node graph instead of
      // rendering whole fragments. Rust keeps the other effects on the batch
      // path, so a null chain plays straight into the output.
      const live = createLiveEffect(ctx, this.activeEffect);
      if (live) {
        this._liveEffect = live;
        live.start();
      }
      scheduler = new PcmStreamScheduler({
        ctx,
        destination: live?.input ?? ctx.destination,
        // The scheduler rejects a chunk of any other reading (M5).
        readingId,
        onComplete: () => {
          console.log("[PlaybackStore] stream playback complete");
          this.finishPlayback();
        }
      });
      scheduler.setVolume(this.volume);
      scheduler.setRate(this.speed, this.pitch);
      this._owner.start(scheduler, readingId, payload.session_id ?? null);
      this.currentFragmentIndex = payload.fragment_index;
      this.totalFragments = payload.fragment_total;
      this.isPlaying = true;
      this.isPaused = false;
      // Only the reading that became audible claims the Rust speaker owner (B1).
      this.emitPlaybackStarted(readingId);
    }
    if (payload.text !== undefined) {
      this._streamCaptions.set(payload.fragment_index, {
        text: payload.text,
        durationMs: 0,
        captions: this._streamCaptions.get(payload.fragment_index)?.captions,
        sessionId: payload.session_id ?? null
      });
    }
    const caption = this._streamCaptions.get(payload.fragment_index);
    if (caption && validCaptionAlignment(payload.captions)) caption.captions = payload.captions;
    if (caption && payload.fragment_duration_ms !== undefined) {
      caption.durationMs = payload.fragment_duration_ms;
    }
    scheduler.handleChunk(payload);
    this.startCaptionClock();
  }

  /**
   * A streamed reading failed on the backend. Only when that reading owns the
   * stream on the speakers does the store stop it, and then only that reading:
   * the waiting runs and the batch queue stay (B3). Another reading's failure
   * leaves the audible one playing, and its own end still emits
   * playback-finished. When nothing is playing, emit playback-finished without a
   * stop: handleStop would also gate off the chunks of a reading that is still
   * synthesizing, and clear its queue.
   */
  handleStreamFailed(failed: StreamFailedPayload): void {
    const readingId = this._eventReadingId(failed.reading_id);
    if (this._owner.fail(readingId, failed.session_id ?? null)) {
      this._stopFailedOwner();
      return;
    }
    if (this._owner.ownsSpeakers()) return;
    // A streamed reading that failed before it could start must not start when
    // the batch queue drains; the owner dropped its hold (or all of them for an
    // id-less failure) above. Only an idle queue may be released here: an active
    // one belongs to another reading, whose failure this event does not describe.
    if (this.isPlaying || this.isLoadingAudio || this._fragmentQueue.isProcessing()) return;
    this._fragmentQueue.markTerminal();
    this.emitPlaybackFinished(null);
  }

  /**
   * B3: stop only the failed streamed reading. The scheduler and its audio stop,
   * the browser finish reports "error", and the waiting runs plus the batch
   * queue stay. Never handleStop here: it drops every held run and sets the
   * stop gate, which would silence the readings that wait.
   */
  private _stopFailedOwner(): void {
    const readingId = this._owner.release();
    // Record the failed reading before the drain: a chunk emitted just before
    // its cancel must not start a new scheduler for it.
    this._owner.ignore(readingId);
    this.stopCaptionClock();
    this._analyser.stop();
    this._disposeLiveEffect();
    this.isPlaying = false;
    this.isPaused = false;
    this.currentFragmentIndex = null;
    this.totalFragments = null;
    this.reportBrowserFinished("error");
    void this._emit?.("hud:stop", null);
    this.emitPlaybackFinished(readingId);
    // The speakers are free: hand them to the next waiting reading.
    this._drainHeldAudio();
  }

  async playAudio() {
    if (!this._audioEl) {
      throw new Error("Audio player is not ready");
    }
    this._audioEl.volume = this.volume / 100;
    // Time-stretch rather than resample, so speed never shifts pitch.
    this._audioEl.preservesPitch = true;
    this._audioEl.playbackRate = this.speed;
    console.log(
      "[PlaybackStore] playAudio: volume",
      this.volume,
      "speed",
      this.speed,
      "src",
      this._audioEl.src?.substring(0, 50)
    );
    await this._audioEl.play();
  }

  async handleReplay(): Promise<void> {
    this.historyReadingId = null;
    if (!this._audioEl) return;
    if (this._fragmentQueue.canReplay()) {
      await this._fragmentQueue.replay();
      return;
    }
    const url = await this.buildPlaybackUrl(this.pitch);
    if (url) {
      this._audioEl.src = url;
      this._audioEl.currentTime = 0;
      try {
        await this.playAudio();
      } catch (e) {
        this.handleStop();
        this.error = `Audio playback failed: ${e instanceof Error ? e.message : String(e)}`;
      }
    }
  }

  handleStop() {
    // Remember which reading this stop stopped before ownership clears (B1): its
    // late chunks stay dropped, a different reading's chunk heals the gate.
    this._stoppedReadingId = this._owner.readingId;
    this._streamStopped = true;
    this.stopCaptionClock();
    this._playbackGeneration += 1;
    this.isLoadingAudio = false;
    this._analyser.stop();
    this._stopping = true;
    // A reading waiting for the other engine is cancelled with this stop.
    this._owner.clearHolds();

    // Clear the fragment queue and any active PCM stream
    this._fragmentQueue.clear();
    const readingId = this._owner.release();
    // A user stop cuts the live chain immediately; only natural completion gets
    // the tail click and delayed dispose.
    this._disposeLiveEffect();
    this.currentFragmentIndex = null;
    this.totalFragments = null;

    if (this._audioEl) {
      this._audioEl.pause();
      this._audioEl.currentTime = 0;
    }
    this.isPlaying = false;
    this.isPaused = false;
    this.reportBrowserFinished("cancelled");
    void this._emit?.("hud:stop", null);
    this.emitPlaybackFinished(readingId);
    setTimeout(() => {
      this._stopping = false;
    }, 0);
  }

  /**
   * Terminal-state signal for Rust waiters (control-server --wait): a
   * blocking /speak resolves when this fires, for natural ends and user
   * stops alike. The payload names the reading that became audible, so Rust
   * claims the speaker owner for it; null covers the batch path (B1).
   */
  private emitPlaybackStarted(readingId: number | null): void {
    if (this._startedEmitted) return;
    this._startedEmitted = true;
    void this._emit?.("playback-started", { reading_id: readingId });
  }

  /** Reply with the reading captured before state cleared, or null (B1). */
  private emitPlaybackFinished(readingId: number | null): void {
    this._startedEmitted = false;
    void this._emit?.("playback-finished", { reading_id: readingId });
  }

  /**
   * Rewind by wall seconds. Stays entirely in the webview: the PCM scheduler
   * rebuilds its source timeline, the audio element seeks its media clock -
   * no Tauri round-trip, so the action is instant on both paths.
   */
  skipBackward(seconds: number = SKIP_SECONDS): void {
    const scheduler = this._owner.scheduler;
    if (scheduler) {
      scheduler.skipBackward(seconds);
      return;
    }
    const el = this._audioEl;
    if (!el || !this.isPlaying) return;
    const delta = seconds * (el.playbackRate || 1);
    if (el.currentTime - delta >= 0 || !Number.isFinite(el.duration)) {
      el.currentTime = Math.max(0, el.currentTime - delta);
      return;
    }
    // Cross the fragment boundary: replay the previous fragment from its tail.
    const deficit = delta - el.currentTime;
    const previous = this._fragmentQueue.previousFragment();
    if (!previous) {
      el.currentTime = 0;
      return;
    }
    void this._playPreviousFragment(previous, deficit);
  }

  /**
   * Fast-forward by wall seconds. Hitting the element's duration advances to
   * the next queued fragment naturally via its ended handler.
   */
  skipForward(seconds: number = SKIP_SECONDS): void {
    const scheduler = this._owner.scheduler;
    if (scheduler) {
      scheduler.skipForward(seconds);
      return;
    }
    const el = this._audioEl;
    if (!el || !this.isPlaying) return;
    const delta = seconds * (el.playbackRate || 1);
    if (Number.isFinite(el.duration)) {
      el.currentTime = Math.min(el.currentTime + delta, el.duration);
    } else {
      el.currentTime += delta;
    }
  }

  /**
   * Seek relative to the current position, as the control server sends it:
   * positive `deltaMs` fast-forwards, negative rewinds, clamped at the timeline
   * ends by the skip methods.
   */
  seekBy(deltaMs: number): void {
    if (!Number.isFinite(deltaMs) || deltaMs === 0) return;
    const seconds = Math.abs(deltaMs) / 1000;
    if (deltaMs > 0) this.skipForward(seconds);
    else this.skipBackward(seconds);
  }

  /**
   * Rewind across a fragment boundary: replay a retained earlier fragment and
   * seek into its tail by `deficit` wall seconds. The interrupted fragment
   * then auto-advances afterwards from its start (small boundary overlap,
   * accepted); a deficit larger than the retained fragment clamps to its start.
   */
  private async _playPreviousFragment(fragment: QueuedFragment, deficit: number): Promise<void> {
    await this._activateFragment(fragment);
    const el = this._audioEl;
    if (!el || deficit <= 0) return;
    const seek = Number.isFinite(el.duration) ? Math.max(0, el.duration - deficit) : 0;
    if (seek > 0) el.currentTime = seek;
  }

  /**
   * Pause the audible reading. Idempotent: a second pause is a no-op, and an
   * idle store stays idle instead of play()ing stale audio.
   */
  pausePlayback(): void {
    if (this.isPaused) return;
    if (!this.isPlaying) return;
    const scheduler = this._owner.scheduler;
    if (scheduler) {
      scheduler.pause();
      this.isPaused = true;
      return;
    }
    if (!this._audioEl) return;
    this._audioEl.pause();
    this.isPaused = true;
  }

  /** Resume a paused reading. Idempotent, and a no-op when nothing is paused. */
  resumePlayback(): void {
    if (!this.isPaused) return;
    const scheduler = this._owner.scheduler;
    if (scheduler) {
      scheduler.resume();
      this.isPaused = false;
      return;
    }
    if (!this._audioEl) return;
    this._audioEl.play().catch(() => {});
    this.isPaused = false;
  }

  handleTogglePause() {
    if (this.isPaused) this.resumePlayback();
    else this.pausePlayback();
  }

  // Keep volume/speed in sync with config (called by synthesize-page via $effect)
  syncPlaybackConfig(volume: number, speed: number, pitch: number, effect: EffectId = "none") {
    this.volume = volume;
    this.speed = speed;
    this.pitch = pitch;
    if (this.activeEffect !== effect) {
      this.activeEffect = effect;
      if (this._cachedPitchUrl) {
        URL.revokeObjectURL(this._cachedPitchUrl.url);
        this._cachedPitchUrl = null;
      }
    }
    if (this._audioEl) {
      this._audioEl.volume = volume / 100;
      this._audioEl.preservesPitch = true;
      this._audioEl.playbackRate = speed;
    }
    // Keep the streaming scheduler in sync while it owns playback
    const scheduler = this._owner.scheduler;
    if (scheduler?.isActive()) {
      scheduler.setVolume(volume);
      scheduler.setRate(speed, pitch);
    }
    // Sync pitch and speed to HUD store for progress bar timing
    hudStore.setPitch(pitch);
    hudStore.setSpeed(speed);
  }

  async setupListeners(): Promise<void> {
    if (!isTauri) return;
    this._listeners ??= this._registerListeners();
    await this._listeners;
  }

  private async _registerListeners(): Promise<Array<() => void>> {
    console.log("[PlaybackStore] Setting up listeners...");
    try {
      const { listen, emit, emitTo } = await import("@tauri-apps/api/event");
      this._emit = emit;
      this._emitTo = emitTo;
      // Legacy audio-ready carries only bytes. HUD metadata supplies its text;
      // queued/streamed fragments carry their own text and take precedence.
      const captionListeners = await Promise.all(
        ["hud:start", "hud:playback_start", "hud:synthesizing"].map((name) =>
          listen<{ text: string | null }>(name, (event) => {
            this._readingText = event.payload.text ?? "";
          })
        )
      );
      console.log("[PlaybackStore] event API loaded");

      // Note: AnalyserNode is set up in handleAudioReady once we have an AudioContext
      // _audioCtx is null here until audio is first decoded

      // Legacy single audio-ready event (for non-paginated playback)
      const unAudioReady = await listen<string>("audio-ready", async (e) => {
        console.log("[PlaybackStore] audio-ready received");
        this._fragmentQueue.enqueue({
          audioBase64: e.payload,
          index: 0,
          total: 1,
          text: ""
        });
        if (!this._fragmentQueue.isProcessing() && !this._streamOwnsSpeakers()) {
          await this._fragmentQueue.startProcessing();
        }
      });

      // New streaming fragment-ready event
      const unFragmentReady = await listen<{
        audio_base64: string;
        fragment_index: number;
        fragment_total: number;
        is_final: boolean;
        text: string;
        captions?: CaptionAlignment | null;
        session_id?: string | null;
      }>("audio-fragment-ready", async (e) => {
        console.log(
          "[PlaybackStore] audio-fragment-ready received, index:",
          e.payload.fragment_index,
          "total:",
          e.payload.fragment_total
        );
        await this.handleFragmentReady(e.payload);
      });

      // Streaming PCM chunks from streaming-capable backends (ElevenLabs)
      const unStreamChunk = await listen<StreamChunkPayload>("audio-stream-chunk", (e) => {
        this.handleStreamChunk(e.payload);
      });

      // Authoritative end-of-synthesis signal for the streamed queue; the
      // scheduler completes once all scheduled sources have drained. The
      // payload names the reading, so a later reading's completion cannot end
      // the reading that still plays.
      const unPaginationComplete = await listen<PaginationCompletePayload | null>(
        "pagination:complete",
        (e) => {
          this.handlePaginationComplete(e.payload);
        }
      );

      console.log("[PlaybackStore] All listeners registered");

      const unPlaybackStop = await listen("playback-stop", () => {
        this.handleStop();
      });

      // A streamed reading failed on the backend; see handleStreamFailed.
      const unStreamFailed = await listen<StreamFailedPayload>("playback-stream-failed", (e) => {
        this.handleStreamFailed(e.payload);
      });

      const unTogglePause = await listen("playback-toggle-pause", () => {
        this.handleTogglePause();
      });

      // Control-server transport routes: pause/resume are idempotent commands,
      // seek carries the relative step in milliseconds.
      const unPause = await listen("playback-pause", () => {
        this.pausePlayback();
      });
      const unResume = await listen("playback-resume", () => {
        this.resumePlayback();
      });
      const unSeek = await listen<{ delta_ms: number }>("playback-seek", (event) => {
        this.seekBy(event.payload.delta_ms);
      });

      // Page controls (browser companion pill) relayed by the Rust bridge.
      const unSkipBackward = await listen("playback-skip-backward", () => {
        this.skipBackward();
      });
      const unSkipForward = await listen("playback-skip-forward", () => {
        this.skipForward();
      });

      // Browser companion: the Rust bridge says when a followed session is
      // cancelled or replaced, so we stop reporting it. An end notice for an
      // already-replaced session leaves its successor alone. Sessions bind
      // only when their own clip becomes audible (see _activateFragment).
      const unBrowserReading = await listen<{ active: boolean; session_id: string }>(
        "browser-reading",
        (e) => {
          if (!e.payload.active && e.payload.session_id === this._browserSessionId) {
            this._browserSessionId = null;
            // The bridge cancelled or replaced this reading; a queue waiting
            // for its next fragment must not wait forever.
            this._fragmentQueue.markTerminal();
          }
        }
      );

      // A scoped stop of a waiting reading: discard only its held run, so the
      // audible reading keeps playing (B1).
      const unReadingDropped = await listen<{ reading_id: number }>(
        "playback-reading-dropped",
        (event) => {
          this._owner.drop(this._eventReadingId(event.payload.reading_id));
        }
      );

      // A section failed permanently after bounded retries; the reading stops
      // there and the already-buffered audio drains. Retained until the next
      // reading starts so the terminal report says "error", not "completed".
      const unReadingFailed = await listen<ReadingFailedPayload>("reading-failed", (event) => {
        const { message } = event.payload;
        const readingId = this._eventReadingId(event.payload.reading_id);
        // A waiting reading's failure is attributed (B4): drop only its held
        // run and report the failure, without touching the audible reading's
        // failure state, its error, or the batch queue. An id-less failure (an
        // older build) and the audible reading keep today's behavior; a batch
        // reading's id cannot be matched here, so it also keeps it.
        if (
          readingId !== null &&
          !this._owner.isAudible(readingId) &&
          this._owner.hasHeld(readingId)
        ) {
          this._owner.drop(readingId);
          toast.error(message, { duration: 15000 });
          return;
        }
        this._readingFailure = message;
        this.error = message;
        // The reading stops at the failed section: release a queue waiting for
        // a further fragment so playback-finished fires and /speak --wait
        // returns instead of hanging to its cap.
        this._fragmentQueue.markTerminal();
        toast.error(message, { duration: 15000 });
      });

      // Global truncation notice: the Play page keeps its inline banner, but a
      // capped clipboard must not stay silent when another page is open.
      const unTruncated = await listen<{
        original_length: number;
        truncated_length: number;
        max_length: number;
      }>("text-truncated", (event) => {
        toast.warning(
          `Copied text was shortened from ${event.payload.original_length} to ${event.payload.truncated_length} characters by the reading limit (${event.payload.max_length}).`,
          { duration: 15000 }
        );
      });

      // Effects/speed/pitch chosen outside the Play page (Effects page, profile
      // switch) only ever reach the backend config; re-sync here so the next
      // reading uses them instead of the values this store held at startup.
      const unConfigChanged = await listen("config-changed", async () => {
        try {
          const config = await invoke<AppConfig>("get_config");
          const { volume } = config.playback;
          const profile = config.tts.profiles.find((p) => p.id === config.tts.active_profile_id);
          const effect = effectiveEffect(config);
          this.syncPlaybackConfig(volume, profile?.speed ?? 1.0, profile?.pitch ?? 1.0, effect);
        } catch (e) {
          console.error("[PlaybackStore] Failed to re-sync after config-changed:", e);
        }
      });

      const unSynthesis = await listen<boolean>("synthesis-state-change", (e) => {
        if (e.payload) {
          this.historyReadingId = null;
          this._streamStopped = false;
          this._stoppedReadingId = null;
          // A new reading owns the failure state from here on.
          this._readingFailure = null;
          this.error = null;
          // Drop fragments a finished earlier reading retained for Replay only
          // when no reading can still use them (M9): nothing plays or loads, the
          // queue is idle and holds no fragment, and no streamed reading waits.
          // `!isProcessing()` alone is unsafe: a held batch reading is not
          // processing by design, and resetting would drop its queued fragments.
          if (
            !this.isPlaying &&
            !this.isLoadingAudio &&
            !this._fragmentQueue.isProcessing() &&
            this._fragmentQueue.getQueueLength() === 0 &&
            !this._owner.hasHeldRuns()
          ) {
            this._fragmentQueue.reset();
          }
        }
        this.isSynthesizing = e.payload;
      });

      const unAbort = await listen("synthesis-aborted", () => {
        // Handle abort event from backend - clear queue and stop
        this.handleStop();
      });

      return [
        ...captionListeners,
        unAudioReady,
        unFragmentReady,
        unStreamChunk,
        unPaginationComplete,
        unPlaybackStop,
        unStreamFailed,
        unTogglePause,
        unPause,
        unResume,
        unSeek,
        unSkipBackward,
        unSkipForward,
        unBrowserReading,
        unReadingDropped,
        unReadingFailed,
        unTruncated,
        unConfigChanged,
        unSynthesis,
        unAbort
      ];
    } catch (e) {
      console.error("Failed to setup playback listeners:", e);
      return [];
    }
  }

  teardownListeners() {
    this.handleStop();
    this._analyser.stop();
    this._analyser.destroy();
    this._owner.release();
    const pending = this._listeners;
    this._listeners = null;
    void pending?.then((fns) => {
      for (const fn of fns) fn();
    });
    if (this._cachedPitchUrl) {
      URL.revokeObjectURL(this._cachedPitchUrl.url);
      this._cachedPitchUrl = null;
    }
    if (this._audioCtx) {
      void this._audioCtx.close();
      this._audioCtx = null;
    }
    this._emit = null;
    this._emitTo = null;
    this._decodedBuffer = null;
    this._originalBytes = null;
    this.hasCachedAudio = false;
    this._fragmentQueue.clear();
    this.currentFragmentIndex = null;
    this.totalFragments = null;
  }
}

export const playbackStore = new PlaybackStore();
