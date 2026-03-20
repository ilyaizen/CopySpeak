/**
 * AnalyserNode management for playback store
 * Handles audio analysis, amplitude monitoring, and HUD waveform data emission
 */

import { buildBarValues } from "./audio-utils.js";

export interface AnalyserConfig {
  fftSize?: number;
  smoothingTimeConstant?: number;
  emitTo?: ((target: string, name: string, payload: unknown) => Promise<void>) | null;
}

export class AudioAnalyser {
  private _analyser: AnalyserNode | null = null;
  private _sourceNode: MediaElementAudioSourceNode | null = null;
  private _amplitudeLoopId: number | null = null;
  private _lastEmitTime = 0;
  private _emitTo: AnalyserConfig["emitTo"] = null;

    /**
     * Wire up the AnalyserNode to the audio element
     * Should be called once per audio element (guard prevents double-wiring)
     */
    setup(
        audioEl: HTMLAudioElement,
        audioCtx: AudioContext,
        config: AnalyserConfig = {}
    ): void {
        if (this._sourceNode) return; // Already wired

        this._sourceNode = audioCtx.createMediaElementSource(audioEl);
        this._analyser = audioCtx.createAnalyser();
        this._analyser.fftSize = config.fftSize ?? 256; // 128 frequency bins
        this._analyser.smoothingTimeConstant = config.smoothingTimeConstant ?? 0.1; // Reduced for faster response
        this._sourceNode.connect(this._analyser);
        this._analyser.connect(audioCtx.destination); // CRITICAL: or audio is silent
        this._emitTo = config.emitTo ?? null;
    }

    /**
     * Start the amplitude monitoring loop
     * Captures frequency data and emits to HUD at ~60fps
     */
    start(): void {
        if (this._amplitudeLoopId !== null) return; // already running

        const loop = (timestamp: number) => {
            if (!this._analyser) {
                this._amplitudeLoopId = null;
                return;
            }
            if (timestamp - this._lastEmitTime >= 16) { // ~60fps for faster response
                const dataArray = new Uint8Array(this._analyser.frequencyBinCount);
                this._analyser.getByteFrequencyData(dataArray);
                const bars = buildBarValues(dataArray, 16);
                void this._emitTo?.("hud", "hud:amplitude", { bars });
                this._lastEmitTime = timestamp;
            }
            this._amplitudeLoopId = requestAnimationFrame(loop);
        };
        this._amplitudeLoopId = requestAnimationFrame(loop);
    }

    /**
     * Stop the amplitude monitoring loop
     */
    stop(): void {
        if (this._amplitudeLoopId !== null) {
            cancelAnimationFrame(this._amplitudeLoopId);
            this._amplitudeLoopId = null;
        }
    }

    /**
     * Get the AnalyserNode instance (for direct access if needed)
     */
    getAnalyser(): AnalyserNode | null {
        return this._analyser;
    }

    /**
     * Check if the amplitude loop is currently running
     */
    isRunning(): boolean {
        return this._amplitudeLoopId !== null;
    }

    /**
     * Clean up resources
     */
    destroy(): void {
        this.stop();
        this._analyser = null;
        this._sourceNode = null;
        this._emitTo = null;
    }
}
