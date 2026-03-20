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
import { audioBufferToWavBlob, detectAudioMimeType } from "./playback/audio-utils.js";
import { AudioAnalyser } from "./playback/analyser.js";
import { FragmentQueue, type QueuedFragment } from "./playback/fragment-queue.js";

class PlaybackStore {
  isPlaying = $state(false);
  isPaused = $state(false);
  isSynthesizing = $state(false);
  error = $state<string | null>(null);
  hasCachedAudio = $state(false);

  // Pagination state for HUD display
  currentFragmentIndex = $state<number | null>(null);
  totalFragments = $state<number | null>(null);

  // Synced from config by whoever has it loaded (synthesize-page or global-player)
  pitch = $state(1.0);
  volume = $state(100);
  speed = $state(1.0);

  private _audioEl: HTMLAudioElement | null = null;
  private _audioCtx: AudioContext | null = null;
  private _decodedBuffer: AudioBuffer | null = null;
  private _originalBytes: ArrayBuffer | null = null;
  private _cachedPitchUrl: { ratio: number; url: string } | null = null;
  private _unlistenFns: Array<() => void> = [];
  private _emit: ((name: string, payload: unknown) => Promise<void>) | null = null;
  private _emitTo: ((target: string, name: string, payload: unknown) => Promise<void>) | null = null;
  private _stopping = false;

  // Modular components
  private _analyser = new AudioAnalyser();
  private _fragmentQueue: FragmentQueue;

  constructor() {
    // Initialize fragment queue with handlers
    this._fragmentQueue = new FragmentQueue({
      onFragmentPlay: async (fragment: QueuedFragment) => {
        console.log("[PlaybackStore] onFragmentPlay: index", fragment.index, "total", fragment.total);
        this.currentFragmentIndex = fragment.index;
        this.totalFragments = fragment.total;
        await this.handleAudioReady(fragment.audioBase64);
      },
      onQueueComplete: () => {
        console.log("[PlaybackStore] onQueueComplete");
        this._analyser.stop();
        this.isPlaying = false;
        this.isPaused = false;
        this.currentFragmentIndex = null;
        this.totalFragments = null;
        void this._emit?.("hud:stop", null);
      },
    });
  }

  setAudioElement(el: HTMLAudioElement | null) {
    this._audioEl = el;
    if (el) {
      el.onplay = () => {
        this.isPlaying = true;
        this.isPaused = false;
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

  async buildPlaybackUrl(pitchRatio: number): Promise<string> {
    if (this._cachedPitchUrl && this._cachedPitchUrl.ratio === pitchRatio) {
      return this._cachedPitchUrl.url;
    }
    if (this._cachedPitchUrl) {
      URL.revokeObjectURL(this._cachedPitchUrl.url);
      this._cachedPitchUrl = null;
    }
    let blob: Blob;
    if (pitchRatio === 1.0 && this._originalBytes) {
      const mimeType = detectAudioMimeType(this._originalBytes);
      blob = new Blob([this._originalBytes], { type: mimeType });
    } else if (this._decodedBuffer && this._audioCtx) {
      const outputLen = Math.max(
        1,
        Math.round(this._decodedBuffer.length / pitchRatio),
      );
      const offline = new OfflineAudioContext(
        this._decodedBuffer.numberOfChannels,
        outputLen,
        this._decodedBuffer.sampleRate,
      );
      const src = offline.createBufferSource();
      src.buffer = this._decodedBuffer;
      src.playbackRate.value = pitchRatio;
      src.connect(offline.destination);
      src.start(0);
      const rendered = await offline.startRendering();
      blob = audioBufferToWavBlob(rendered);
    } else {
      return "";
    }
    const url = URL.createObjectURL(blob);
    this._cachedPitchUrl = { ratio: pitchRatio, url };
    return url;
  }

  async handleAudioReady(base64: string): Promise<void> {
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

    // Wire AnalyserNode once per audio element (guard prevents double-wiring)
    if (this._audioEl && this._audioCtx && !this._analyser.getAnalyser()) {
      this._analyser.setup(this._audioEl, this._audioCtx, {
        emitTo: this._emitTo,
      });
    }

    try {
      this._decodedBuffer = await this._audioCtx.decodeAudioData(
        arrayBuffer.slice(0),
      );
      const url = await this.buildPlaybackUrl(this.pitch);
      if (this._audioEl && url) {
        this._audioEl.src = url;
        this._analyser.start(); // Start amplitude capture BEFORE audio plays
        this.playAudio();
      }
      this.hasCachedAudio = true;
    } catch (e) {
      this.error = `Audio decode error: ${e}`;
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
  }): Promise<void> {
    console.log("[PlaybackStore] handleFragmentReady: index", payload.fragment_index, "total", payload.fragment_total, "is_final", payload.is_final);
    // Add to queue
    this._fragmentQueue.enqueue({
      audioBase64: payload.audio_base64,
      index: payload.fragment_index,
      total: payload.fragment_total,
      text: payload.text,
    });
    console.log("[PlaybackStore] Queue length:", this._fragmentQueue.getQueueLength(), "isProcessing:", this._fragmentQueue.isProcessing());// Start processing if not already
    if (!this._fragmentQueue.isProcessing()) {
      await this._fragmentQueue.startProcessing();
    }
  }

  playAudio() {
    if (!this._audioEl) {
      console.error("[PlaybackStore] playAudio: no audio element");
      return;
    }
    this._audioEl.volume = this.volume / 100;
    this._audioEl.playbackRate = this.speed;
    console.log("[PlaybackStore] playAudio: volume", this.volume, "speed", this.speed, "src", this._audioEl.src?.substring(0, 50));
    this._audioEl.play().catch((err) => {
      console.error("[PlaybackStore] play() failed:", err);
    });
  }

  async handleReplay(): Promise<void> {
    if (!this._audioEl) return;
    const url = await this.buildPlaybackUrl(this.pitch);
    if (url) {
      this._audioEl.src = url;
      this._audioEl.currentTime = 0;
      this.playAudio();
    }
  }

  handleStop() {
    this._analyser.stop();
    this._stopping = true;

    // Clear the fragment queue
    this._fragmentQueue.clear();
    this.currentFragmentIndex = null;
    this.totalFragments = null;

    if (this._audioEl) {
      this._audioEl.pause();
      this._audioEl.currentTime = 0;
    }
    this.isPlaying = false;
    this.isPaused = false;
    void this._emit?.("hud:stop", null);
    setTimeout(() => { this._stopping = false; }, 0);
  }

  handleTogglePause() {
    if (!this._audioEl) return;
    if (this._audioEl.paused) {
      this._audioEl.play().catch(() => { });
      this.isPaused = false;
    } else {
      this._audioEl.pause();
      this.isPaused = true;
    }
  }

  // Keep volume/speed in sync with config (called by synthesize-page via $effect)
  syncPlaybackConfig(volume: number, speed: number, pitch: number) {
    this.volume = volume;
    this.speed = speed;
    this.pitch = pitch;
    if (this._audioEl) {
      this._audioEl.volume = volume / 100;
      this._audioEl.playbackRate = speed;
    }
  }

  async setupListeners(): Promise<void> {
    if (!isTauri) return;
    console.log("[PlaybackStore] Setting up listeners...");
    try {
      const { listen, emit, emitTo } = await import("@tauri-apps/api/event");
      this._emit = emit;
      this._emitTo = emitTo;
      console.log("[PlaybackStore] event API loaded");

      // Note: AnalyserNode is set up in handleAudioReady once we have an AudioContext
      // _audioCtx is null here until audio is first decoded

      // Legacy single audio-ready event (for non-paginated playback)
      const unAudioReady = await listen<string>("audio-ready", async (e) => {
        console.log("[PlaybackStore] audio-ready received");
        await this.handleAudioReady(e.payload);
      });

      // New streaming fragment-ready event
      const unFragmentReady = await listen<{
        audio_base64: string;
        fragment_index: number;
        fragment_total: number;
        is_final: boolean;
        text: string;
      }>("audio-fragment-ready", async (e) => {
        console.log("[PlaybackStore] audio-fragment-ready received, index:", e.payload.fragment_index, "total:", e.payload.fragment_total);
        await this.handleFragmentReady(e.payload);
      });

      console.log("[PlaybackStore] All listeners registered");

      const unPlaybackStop = await listen("playback-stop", () => {
        this.handleStop();
      });

      const unTogglePause = await listen("playback-toggle-pause", () => {
        this.handleTogglePause();
      });

      const unSynthesis = await listen<boolean>(
        "synthesis-state-change",
        (e) => {
          this.isSynthesizing = e.payload;
        },
      );

      const unAbort = await listen("synthesis-aborted", () => {
        // Handle abort event from backend - clear queue and stop
        this.handleStop();
      });

      this._unlistenFns = [
        unAudioReady,
        unFragmentReady,
        unPlaybackStop,
        unTogglePause,
        unSynthesis,
        unAbort,
      ];
    } catch (e) {
      console.error("Failed to setup playback listeners:", e);
    }
  }

  teardownListeners() {
    this._analyser.stop();
    this._analyser.destroy();
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
