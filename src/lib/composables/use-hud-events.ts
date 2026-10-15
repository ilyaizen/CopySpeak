import { isTauri } from "#lib/services/tauri.js";
import { hudStore } from "#lib/stores/hud-store.svelte.js";
import type {
  HudStartPayload,
  HudSynthesizingPayload,
  HudPlaybackStartPayload,
  SynthesisProgressPayload,
  PaginationPayload,
  ClipboardCopiedPayload,
  AmplitudePayload,
  HudCaptionPayload
} from "#lib/types/hud.js";

export function useHudEvents() {
  const unlisteners: Array<() => void> = [];
  // HMR can destroy the overlay while setup still awaits `listen`; cleanup then
  // misses what registers afterwards, and every HUD event would fire twice.
  let destroyed = false;

  async function setupEventListeners() {
    if (!isTauri) return;

    try {
      const eventApi = await import("@tauri-apps/api/event");

      unlisteners.push(
        await eventApi.listen<HudStartPayload>("hud:start", (event) => {
          void eventApi.emit("hud:diag:start-received");
          hudStore.handleStart(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<HudSynthesizingPayload>("hud:synthesizing", (event) => {
          hudStore.handleSynthesizing(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<HudPlaybackStartPayload>("hud:playback_start", (event) => {
          hudStore.handlePlaybackStart(event.payload, event.payload.audio_duration_ms ?? 0);
        })
      );

      unlisteners.push(
        await eventApi.listen("hud:stop", () => {
          hudStore.handleStop();
        })
      );

      unlisteners.push(
        await eventApi.listen("playback-toggle-pause", () => {
          hudStore.togglePause();
        })
      );

      unlisteners.push(
        await eventApi.listen<PaginationPayload>("pagination:started", (event) => {
          hudStore.handlePagination(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<PaginationPayload>("pagination:fragment-started", (event) => {
          hudStore.handlePagination(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<PaginationPayload>("pagination:fragment-ready", (event) => {
          hudStore.handlePagination(event.payload, true);
        })
      );

      unlisteners.push(
        await eventApi.listen<AmplitudePayload>("hud:amplitude", (event) => {
          hudStore.handleAmplitude(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<ClipboardCopiedPayload>("hud:clipboard-copied", (event) => {
          hudStore.handleClipboardCopied(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<SynthesisProgressPayload>("hud:synthesis-progress", (event) => {
          hudStore.handleSynthesisProgress(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<number>("hud:audio-duration", (event) => {
          hudStore.setAccurateDurationMs(event.payload);
        })
      );

      unlisteners.push(
        await eventApi.listen<HudCaptionPayload>("hud:caption", (event) => {
          hudStore.handleCaption(event.payload);
        })
      );

      // Cleanup already ran, so these registered after it: drop them.
      if (destroyed) {
        cleanupEventListeners();
        return;
      }

      // Diagnostic: confirm all listeners registered and IPC works
      await eventApi.emit("hud:diag:listeners-ready");
      console.log("[HUD] diagnostic listeners-ready event emitted");
    } catch (e) {
      console.error("[HUD] Failed to set up Tauri event listeners:", e);
    }
  }

  function cleanupEventListeners() {
    destroyed = true;
    // splice empties the list, so a second cleanup only reaches later registrations.
    for (const unlisten of unlisteners.splice(0)) unlisten();
  }

  return {
    setupEventListeners,
    cleanupEventListeners
  };
}
