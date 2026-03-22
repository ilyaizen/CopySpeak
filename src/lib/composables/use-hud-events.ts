import { isTauri } from "$lib/services/tauri.js";
import { hudStore } from "$lib/stores/hud-store.svelte.js";
import type {
  HudStartPayload,
  HudSynthesizingPayload,
  HudPlaybackStartPayload,
  SynthesisProgressPayload,
  PaginationPayload,
  ClipboardCopiedPayload,
  AmplitudePayload
} from "$lib/types/hud.js";

interface Unlisteners {
  start?: () => void;
  stop?: () => void;
  synthesizing?: () => void;
  playbackStart?: () => void;
  amplitude?: () => void;
  paginationStarted?: () => void;
  paginationFragmentStarted?: () => void;
  paginationFragmentReady?: () => void;
  togglePause?: () => void;
  clipboardCopied?: () => void;
  synthesisProgress?: () => void;
}

export function useHudEvents() {
  const unlisteners: Unlisteners = {};

  async function setupEventListeners() {
    if (!isTauri) return;

    try {
      const eventApi = await import("@tauri-apps/api/event");

      unlisteners.start = await eventApi.listen<HudStartPayload>("hud:start", (event) => {
        hudStore.handleStart(event.payload);
      });

      unlisteners.synthesizing = await eventApi.listen<HudSynthesizingPayload>(
        "hud:synthesizing",
        (event) => {
          hudStore.handleSynthesizing(event.payload);
        }
      );

      unlisteners.playbackStart = await eventApi.listen<HudPlaybackStartPayload>(
        "hud:playback_start",
        (event) => {
          hudStore.handlePlaybackStart(event.payload, event.payload.audio_duration_ms ?? 0);
        }
      );

      unlisteners.stop = await eventApi.listen("hud:stop", () => {
        hudStore.handleStop();
      });

      unlisteners.togglePause = await eventApi.listen("playback-toggle-pause", () => {
        hudStore.togglePause();
      });

      unlisteners.paginationStarted = await eventApi.listen<PaginationPayload>(
        "pagination:started",
        (event) => {
          hudStore.handlePagination(event.payload);
        }
      );

      unlisteners.paginationFragmentStarted = await eventApi.listen<PaginationPayload>(
        "pagination:fragment-started",
        (event) => {
          hudStore.handlePagination(event.payload);
        }
      );

      unlisteners.paginationFragmentReady = await eventApi.listen<PaginationPayload>(
        "pagination:fragment-ready",
        (event) => {
          hudStore.handlePagination(event.payload, true);
        }
      );

      unlisteners.amplitude = await eventApi.listen<AmplitudePayload>("hud:amplitude", (event) => {
        hudStore.handleAmplitude(event.payload);
      });

      unlisteners.clipboardCopied = await eventApi.listen<ClipboardCopiedPayload>(
        "hud:clipboard-copied",
        (event) => {
          hudStore.handleClipboardCopied(event.payload);
        }
      );

      unlisteners.synthesisProgress = await eventApi.listen<SynthesisProgressPayload>(
        "hud:synthesis-progress",
        (event) => {
          hudStore.handleSynthesisProgress(event.payload);
        }
      );
    } catch (e) {
      console.error("[HUD] Failed to set up Tauri event listeners:", e);
    }
  }

  function cleanupEventListeners() {
    Object.values(unlisteners).forEach((unlisten) => {
      if (unlisten) unlisten();
    });
  }

  return {
    setupEventListeners,
    cleanupEventListeners
  };
}
