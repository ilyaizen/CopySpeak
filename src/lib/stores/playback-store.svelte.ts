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

import { isTauri } from "$lib/services/tauri.js";
import type { EffectId } from "$lib/types";
import { applyFadeIn, audioBufferToWavBlob, detectAudioMimeType } from "./playback/audio-utils.js";
import { AudioAnalyser } from "./playback/analyser.js";
import { PcmStreamScheduler, type StreamChunkPayload } from "./playback/pcm-stream.js";
import { getEffect } from "./playback/effects/registry.js";
import { FragmentQueue, type QueuedFragment } from "./playback/fragment-queue.js";
import { hudStore } from "./hud-store.svelte.js";
import { validCaptionAlignment, type CaptionAlignment } from "$lib/models/captions.js";
import type { HudCaptionPayload } from "$lib/types/hud.js";

class PlaybackStore {
  isPlaying = $state(false);
  isPaused = $state(false);
  isSynthesizing = $state(false);
  isLoadingAudio = $state(false);
  error = $state<string | null>(null);
  hasCachedAudio = $state(false);
  // Retained after stop/completion so the owning history row can offer Replay.
  historyReadingId = $state<string | null>(null);

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
  private _unlistenFns: Array<() => void> = [];
  private _emit: ((name: string, payload: unknown) => Promise<void>) | null = null;
  private _emitTo: ((target: string, name: string, payload: unknown) => Promise<void>) | null =
    null;
  private _stopping = false;
  private _playbackGeneration = 0;
  private _readingText = "";
  private _fragmentCaptions: CaptionAlignment | null = null;
  private _renderedPitch = 1;
  private _streamStopped = false;
  private _fragmentText = "";
  private _captionTimer: ReturnType<typeof setInterval> | null = null;
  private _lastCaption: HudCaptionPayload | null = null;
  private _streamCaptions = new Map<
    number,
    { text: string; durationMs: number; captions?: CaptionAlignment | null }
  >();

  // Modular components
  private _analyser = new AudioAnalyser();
  private _fragmentQueue: FragmentQueue;
  private _pcmScheduler: PcmStreamScheduler | null = null;

  constructor() {
    // Initialize fragment queue with handlers
    this._fragmentQueue = new FragmentQueue({
      onFragmentPlay: async (fragment: QueuedFragment) => {
        console.log(
          "[PlaybackStore] onFragmentPlay: index",
          fragment.index,
          "total",
          fragment.total
        );
        this.currentFragmentIndex = fragment.index;
        this.totalFragments = fragment.total;
        this._fragmentText = fragment.text;
        this._fragmentCaptions = validCaptionAlignment(fragment.captions)
          ? fragment.captions
          : null;
        await this.handleAudioReady(fragment.audioBase64);
      },
      onQueueComplete: () => {
        console.log("[PlaybackStore] onQueueComplete");
        this.finishPlayback();
      }
    });
  }

  setAudioElement(el: HTMLAudioElement | null) {
    this._audioEl = el;
    if (el) {
      el.onplay = () => {
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
    this._streamCaptions.clear();
  }

  private publishCaption() {
    let caption: HudCaptionPayload | null = null;
    if (this._pcmScheduler) {
      const position = this._pcmScheduler.getPlaybackPosition();
      const fragment = position ? this._streamCaptions.get(position.fragmentIndex) : null;
      if (position && fragment) {
        caption = {
          text: fragment.captions?.text ?? fragment.text,
          captions: fragment.captions,
          position_ms: position.positionMs,
          duration_ms: fragment.durationMs,
          paused: this.isPaused,
          active: true
        };
      } else if (this._lastCaption) {
        caption = { ...this._lastCaption, active: false, paused: this.isPaused };
      }
    } else if (this._audioEl && this.isPlaying) {
      const el = this._audioEl;
      const duration = Number.isFinite(el.duration)
        ? el.duration
        : (this._decodedBuffer?.duration ?? 0) / this._renderedPitch;
      caption = {
        text: this._fragmentCaptions?.text ?? (this._fragmentText || this._readingText),
        captions: this._fragmentCaptions,
        position_ms: el.currentTime * this._renderedPitch * 1000,
        duration_ms: duration * this._renderedPitch * 1000,
        paused: this.isPaused,
        active: !this.isLoadingAudio && !el.ended && !el.seeking && el.readyState >= 2
      };
    }
    if (!caption) return;
    this._lastCaption = caption;
    hudStore.handleCaption(caption);
    void this._emitTo?.("hud", "hud:caption", caption);
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
        const outputLen = Math.max(1, Math.round(this._decodedBuffer.length / pitchRatio));
        const offline = new OfflineAudioContext(
          this._decodedBuffer.numberOfChannels,
          outputLen,
          this._decodedBuffer.sampleRate
        );
        const src = offline.createBufferSource();
        src.buffer = this._decodedBuffer;
        src.playbackRate.value = pitchRatio;
        src.connect(offline.destination);
        src.start(0);
        buffer = await offline.startRendering();
      }
      if (effect) {
        buffer = await effect.process(buffer, this._audioCtx);
      }
      applyFadeIn(buffer, 10);
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
        this._emit?.("hud:audio-duration", accurateDurationMs);
      }
      const renderedPitch = this.pitch;
      const url = await this.buildPlaybackUrl(renderedPitch);
      if (generation !== this._playbackGeneration) return;
      if (!this._audioEl || !url) throw new Error("Audio player is not ready");
      this._renderedPitch = renderedPitch;
      this._audioEl.src = url;
      this._analyser.start(); // Start amplitude capture BEFORE audio plays
      await this.playAudio();
      if (generation !== this._playbackGeneration) return;
      this.hasCachedAudio = true;
    } catch (e) {
      if (generation !== this._playbackGeneration) return;
      this.handleStop();
      this.error = `Audio playback failed: ${e}`;
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
      captions: payload.captions
    });
    console.log(
      "[PlaybackStore] Queue length:",
      this._fragmentQueue.getQueueLength(),
      "isProcessing:",
      this._fragmentQueue.isProcessing()
    ); // Start processing if not already
    if (!this._fragmentQueue.isProcessing()) {
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
    this.isPlaying = false;
    this.isPaused = false;
    this.currentFragmentIndex = null;
    this.totalFragments = null;
    this._pcmScheduler = null;
    void this._emit?.("hud:stop", null);
  }

  /**
   * Handle one PCM chunk of streaming synthesis audio. Lazily creates the
   * scheduler (which prebuffers ~250ms before starting gap-free playback) and
   * feeds it the chunk.
   */
  handleStreamChunk(payload: StreamChunkPayload): void {
    if (this._streamStopped) return;
    if (!this._audioCtx) {
      this._audioCtx = new AudioContext();
    }
    const ctx = this._audioCtx;
    // Resume AudioContext if suspended (autoplay policies, or a paused stream)
    if (ctx.state === "suspended" && !this.isPaused) {
      void ctx.resume();
    }
    if (!this._pcmScheduler) {
      console.log("[PlaybackStore] creating PCM scheduler for fragment", payload.fragment_index);
      this._pcmScheduler = new PcmStreamScheduler({
        ctx,
        destination: ctx.destination,
        onComplete: () => {
          console.log("[PlaybackStore] stream playback complete");
          this.finishPlayback();
        }
      });
      this._pcmScheduler.setVolume(this.volume);
      this._pcmScheduler.setRate(this.speed, this.pitch);
      this.currentFragmentIndex = payload.fragment_index;
      this.totalFragments = payload.fragment_total;
      this.isPlaying = true;
      this.isPaused = false;
    }
    if (payload.text !== undefined) {
      this._streamCaptions.set(payload.fragment_index, {
        text: payload.text,
        durationMs: 0,
        captions: this._streamCaptions.get(payload.fragment_index)?.captions
      });
    }
    const caption = this._streamCaptions.get(payload.fragment_index);
    if (caption && validCaptionAlignment(payload.captions)) caption.captions = payload.captions;
    if (caption && payload.fragment_duration_ms !== undefined) {
      caption.durationMs = payload.fragment_duration_ms;
    }
    this._pcmScheduler.handleChunk(payload);
    if (this._pcmScheduler) this.startCaptionClock();
  }

  async playAudio() {
    if (!this._audioEl) {
      throw new Error("Audio player is not ready");
    }
    this._audioEl.volume = this.volume / 100;
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
    const renderedPitch = this.pitch;
    const url = await this.buildPlaybackUrl(renderedPitch);
    if (url) {
      this._renderedPitch = renderedPitch;
      this._audioEl.src = url;
      this._audioEl.currentTime = 0;
      try {
        await this.playAudio();
      } catch (e) {
        this.handleStop();
        this.error = `Audio playback failed: ${e}`;
      }
    }
  }

  handleStop() {
    this._streamStopped = true;
    this.stopCaptionClock();
    this._playbackGeneration += 1;
    this.isLoadingAudio = false;
    this._analyser.stop();
    this._stopping = true;

    // Clear the fragment queue and any active PCM stream
    this._fragmentQueue.clear();
    this._pcmScheduler?.stop();
    this._pcmScheduler = null;
    this.currentFragmentIndex = null;
    this.totalFragments = null;

    if (this._audioEl) {
      this._audioEl.pause();
      this._audioEl.currentTime = 0;
    }
    this.isPlaying = false;
    this.isPaused = false;
    void this._emit?.("hud:stop", null);
    setTimeout(() => {
      this._stopping = false;
    }, 0);
  }

  handleTogglePause() {
    if (this._pcmScheduler) {
      if (this.isPaused) {
        this._pcmScheduler.resume();
        this.isPaused = false;
      } else {
        this._pcmScheduler.pause();
        this.isPaused = true;
      }
      return;
    }
    if (!this._audioEl) return;
    if (this._audioEl.paused) {
      this._audioEl.play().catch(() => {});
      this.isPaused = false;
    } else {
      this._audioEl.pause();
      this.isPaused = true;
    }
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
      this._audioEl.playbackRate = speed;
    }
    // Keep the streaming scheduler in sync while it owns playback
    if (this._pcmScheduler?.isActive()) {
      this._pcmScheduler.setVolume(volume);
      this._pcmScheduler.setRate(speed, pitch);
    }
    // Sync pitch and speed to HUD store for progress bar timing
    hudStore.setPitch(pitch);
    hudStore.setSpeed(speed);
  }

  async setupListeners(): Promise<void> {
    if (!isTauri) return;
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
          index: 1,
          total: 1,
          text: ""
        });
        if (!this._fragmentQueue.isProcessing()) {
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
      // scheduler completes once all scheduled sources have drained.
      const unPaginationComplete = await listen("pagination:complete", () => {
        this._pcmScheduler?.markQueueComplete();
      });

      console.log("[PlaybackStore] All listeners registered");

      const unPlaybackStop = await listen("playback-stop", () => {
        this.handleStop();
      });

      const unTogglePause = await listen("playback-toggle-pause", () => {
        this.handleTogglePause();
      });

      const unSynthesis = await listen<boolean>("synthesis-state-change", (e) => {
        if (e.payload) {
          this.historyReadingId = null;
          this._streamStopped = false;
        }
        this.isSynthesizing = e.payload;
      });

      const unAbort = await listen("synthesis-aborted", () => {
        // Handle abort event from backend - clear queue and stop
        this.handleStop();
      });

      this._unlistenFns = [
        ...captionListeners,
        unAudioReady,
        unFragmentReady,
        unStreamChunk,
        unPaginationComplete,
        unPlaybackStop,
        unTogglePause,
        unSynthesis,
        unAbort
      ];
    } catch (e) {
      console.error("Failed to setup playback listeners:", e);
    }
  }

  teardownListeners() {
    this.handleStop();
    this._analyser.stop();
    this._analyser.destroy();
    this._pcmScheduler?.stop();
    this._pcmScheduler = null;
    for (const fn of this._unlistenFns) fn();
    this._unlistenFns = [];
    if (this._cachedPitchUrl) {
      URL.revokeObjectURL(this._cachedPitchUrl.url);
      this._cachedPitchUrl = null;
    }
    if (this._audioCtx) {
      this._audioCtx.close();
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
