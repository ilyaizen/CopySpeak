import type {
    HudStartPayload,
    HudSynthesizingPayload,
    HudPlaybackStartPayload,
    SynthesisProgressPayload,
    PaginationPayload,
    ClipboardCopiedPayload,
    AmplitudePayload,
} from "$lib/types/hud.js";

// Core state
let barValues = $state<number[]>([]);
let isVisible = $state(false);
let isSynthesizing = $state(false);
let isPaused = $state(false);
let spokenText = $state<string | null>(null);
let provider = $state<string | null>(null);
let voice = $state<string | null>(null);

// Pagination state
let currentFragment = $state<number | null>(null);
let totalFragments = $state<number | null>(null);
let isPaginated = $state(false);

// Progress state
let totalChars = $state<number>(0);
let processedChars = $state<number>(0);
let progressConfidence = $state<number>(0);
let estimatedDurationMs = $state<number | null>(null);
let elapsedMs = $state<number>(0);

// Playback state
let audioDurationMs = $state<number>(0);

// Clipboard state
let isClipboardCopied = $state(false);
let clipboardDurationMs = $state(800);

// Derived values
let providerVoiceLabel = $derived(
    !provider && !voice
        ? null
        : (() => {
            const p = provider ?? "";
            const v = voice ? (voice.length > 20 ? voice.substring(0, 20) + "\u2026" : voice) : "";
            if (p && v) return `${p} \u00b7 ${v}`;
            return p || v;
        })()
);

let statusLabel = $derived(
    isSynthesizing ? "Processing..." : isPaused ? "Paused" : "Playing"
);

const DEFAULT_ESTIMATE_MS = 3000;
  const MAX_PROGRESS_WITHOUT_ESTIMATE = 65;

  let progressPercent = $derived(
      (() => {
          const charProgress = totalChars > 0 ? (processedChars / totalChars) * 100 : 0;

          // Prefer character progress when available - it's more accurate than time-based estimates
          if (totalChars > 0 && processedChars > 0) {
              return Math.min(99, charProgress);
          }

          // Fallback to time-based progress if we have an estimate
          if (estimatedDurationMs !== null && estimatedDurationMs > 0) {
              return Math.min(99, (elapsedMs / estimatedDurationMs) * 100);
          }

          // No data - show subtle animation based on elapsed time
          const effectiveMs = elapsedMs > 0 ? elapsedMs : 100;
          const defaultTimeProgress = (effectiveMs / DEFAULT_ESTIMATE_MS) * 100;
          return Math.min(MAX_PROGRESS_WITHOUT_ESTIMATE, defaultTimeProgress);
      })()
  );

  let hasEstimate = $derived(estimatedDurationMs !== null || totalChars > 0);

let dotPulsing = $derived(isSynthesizing || (!isPaused && !isSynthesizing));

// Actions
export const hudStore = {
    // Getters
    get barValues() {
        return barValues;
    },
    get isVisible() {
        return isVisible;
    },
    get isSynthesizing() {
        return isSynthesizing;
    },
    get isPaused() {
        return isPaused;
    },
    get spokenText() {
        return spokenText;
    },
    get provider() {
        return provider;
    },
    get voice() {
        return voice;
    },
    get currentFragment() {
        return currentFragment;
    },
    get totalFragments() {
        return totalFragments;
    },
    get isPaginated() {
        return isPaginated;
    },
    get totalChars() {
        return totalChars;
    },
    get processedChars() {
        return processedChars;
    },
    get progressConfidence() {
        return progressConfidence;
    },
    get estimatedDurationMs() {
        return estimatedDurationMs;
    },
    get elapsedMs() {
        return elapsedMs;
    },
    get isClipboardCopied() {
        return isClipboardCopied;
    },
    get clipboardDurationMs() {
        return clipboardDurationMs;
    },
    get providerVoiceLabel() {
        return providerVoiceLabel;
    },
    get statusLabel() {
        return statusLabel;
    },
    get progressPercent() {
        return progressPercent;
    },
    get hasEstimate() {
        return hasEstimate;
    },
    get dotPulsing() {
        return dotPulsing;
    },
    get audioDurationMs() {
        return audioDurationMs;
    },

    // Setters
    setBarValues(values: number[]) {
        barValues = values;
    },
    setIsVisible(value: boolean) {
        isVisible = value;
    },
    setIsSynthesizing(value: boolean) {
        isSynthesizing = value;
    },
    setIsPaused(value: boolean) {
        isPaused = value;
    },
    setSpokenText(text: string | null) {
        spokenText = text;
    },
    setProvider(p: string | null) {
        provider = p;
    },
    setVoice(v: string | null) {
        voice = v;
    },
    setCurrentFragment(index: number | null) {
        currentFragment = index;
    },
    setTotalFragments(total: number | null) {
        totalFragments = total;
    },
    setIsPaginated(value: boolean) {
        isPaginated = value;
    },
    setTotalChars(chars: number) {
        totalChars = chars;
    },
    setProcessedChars(chars: number) {
        processedChars = chars;
    },
    setProgressConfidence(confidence: number) {
        progressConfidence = confidence;
    },
    setEstimatedDurationMs(ms: number | null) {
        estimatedDurationMs = ms;
    },
    setElapsedMs(ms: number) {
        elapsedMs = ms;
    },
    setIsClipboardCopied(value: boolean) {
        isClipboardCopied = value;
    },
    setClipboardDurationMs(ms: number) {
        clipboardDurationMs = ms;
    },
    setAudioDurationMs(ms: number) {
        audioDurationMs = ms;
    },

    // Compound actions
    handleStart(payload: HudStartPayload) {
        spokenText = payload.text || spokenText;
        provider = payload.provider ?? null;
        voice = payload.voice ?? null;
        isSynthesizing = false;
        isPaused = false;
        isVisible = true;
        estimatedDurationMs = null;
        elapsedMs = 0;
        // Extract audio duration from envelope for progress bar
        audioDurationMs = payload.envelope?.duration_ms ?? 0;
    },

    handleSynthesizing(payload: HudSynthesizingPayload) {
        spokenText = payload.text || null;
        provider = payload.provider ?? null;
        voice = payload.voice ?? null;
        isSynthesizing = true;
        isPaused = false;
        barValues = [];
        isVisible = true;
        elapsedMs = 0;
    },

    handlePlaybackStart(payload: HudPlaybackStartPayload, durationMs: number = 0) {
        spokenText = payload.text || null;
        provider = payload.provider ?? null;
        voice = payload.voice ?? null;
        isSynthesizing = false;
        isPaused = false;
        barValues = [];
        isVisible = true;
        
        // Use provided duration or estimate from text length (~4 chars/sec for speech)
        if (durationMs > 0) {
            audioDurationMs = durationMs;
        } else if (payload.text) {
            // Rough estimate: ~250 chars per minute = ~4 chars/sec
            audioDurationMs = Math.ceil(payload.text.length * 250);
        } else {
            audioDurationMs = 0;
        }
    },

    handleStop() {
        isSynthesizing = false;
        isPaused = false;
        isVisible = false;
        barValues = [];
        currentFragment = null;
        totalFragments = null;
        isPaginated = false;
        estimatedDurationMs = null;
        elapsedMs = 0;
        totalChars = 0;
        processedChars = 0;
        progressConfidence = 0;
        audioDurationMs = 0;
    },

    handleSynthesisProgress(payload: SynthesisProgressPayload) {
        estimatedDurationMs = payload.estimated_total_ms;
        elapsedMs = payload.elapsed_ms;
        isPaginated = payload.is_paginated;
        currentFragment = payload.fragment_index;
        totalFragments = payload.fragment_total;
        totalChars = payload.total_chars;
        processedChars = payload.processed_chars;
        progressConfidence = payload.confidence;
    },

    handlePagination(payload: PaginationPayload, _fragmentReady: boolean = false) {
        isPaginated = payload.is_paginated;
        totalFragments = payload.total;
        currentFragment = payload.current_index;
    },

    handleClipboardCopied(payload: ClipboardCopiedPayload) {
        if (isVisible || isSynthesizing) return;
        clipboardDurationMs = payload.trigger_window_ms;
        isClipboardCopied = true;
    },

    handleAmplitude(payload: AmplitudePayload) {
        barValues = payload.bars;
    },

    togglePause() {
        isPaused = !isPaused;
    },

    clearClipboardCopied() {
        isClipboardCopied = false;
    },
};
