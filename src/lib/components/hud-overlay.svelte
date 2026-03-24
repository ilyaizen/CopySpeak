<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { isTauri } from "$lib/services/tauri.js";
  import { hudStore } from "$lib/stores/hud-store.svelte.js";
  import { useHudEvents } from "$lib/composables/use-hud-events.js";
  import { createTimer, clearTimer, createTimeout, clearTimeoutState } from "$lib/utils/timer.js";
  import { ClipboardNotification, HudSynthesisProgress, HudPlaybackContent } from "./hud/index.js";

  // Timer state
  // Tracks the interval that measures elapsed synthesis time (updates every 100ms)
  // Used to show progress during TTS generation
  let elapsedTimerState = $state<{
    timer: ReturnType<typeof setInterval> | null;
    startTime: number | null;
  }>({
    timer: null,
    startTime: null
  });
  // Tracks playback elapsed time for progress bar during audio playback
  let playbackTimerState = $state<{
    timer: ReturnType<typeof setInterval> | null;
    startTime: number | null;
  }>({
    timer: null,
    startTime: null
  });
  // Tracks the timeout that auto-dismisses clipboard notifications
  // Ensures notifications disappear after the configured duration
  let clipboardDismissTimerState = $state<{ timer: ReturnType<typeof setTimeout> | null }>({
    timer: null
  });

  // Event listeners
  // Setup and cleanup functions for Tauri IPC event handlers
  const { setupEventListeners, cleanupEventListeners } = useHudEvents();

  /**
   * Starts tracking synthesis elapsed time.
   * Creates an interval that updates hudStore.elapsedMs every 100ms during TTS generation.
   * Clears any existing timer first to prevent duplicate intervals.
   */
  function startElapsedTimer() {
    clearTimer(elapsedTimerState);
    elapsedTimerState = createTimer((elapsed) => {
      hudStore.setElapsedMs(elapsed);
    }, 100);
  }

  function stopElapsedTimer() {
    // Stops the elapsed timer when synthesis completes or is cancelled
    clearTimer(elapsedTimerState);
  }

  function startPlaybackTimer() {
    clearTimer(playbackTimerState);
    playbackTimerState = createTimer((elapsed) => {
      hudStore.setPlaybackElapsedMs(elapsed);
    }, 100);
  }

  function stopPlaybackTimer() {
    clearTimer(playbackTimerState);
  }

  // Automatically manage timer based on synthesis state
  // Starts timer when TTS begins, stops when complete - ensures accurate timing throughout synthesis lifecycle
  $effect(() => {
    if (hudStore.isSynthesizing && elapsedTimerState.timer === null) {
      startElapsedTimer();
    } else if (!hudStore.isSynthesizing && elapsedTimerState.timer !== null) {
      stopElapsedTimer();
    }
  });

  // Manage playback timer when playback is active (visible and not synthesizing)
  $effect(() => {
    const isPlayback = hudStore.isVisible && !hudStore.isSynthesizing;
    if (isPlayback && playbackTimerState.timer === null) {
      startPlaybackTimer();
    } else if (!isPlayback && playbackTimerState.timer !== null) {
      stopPlaybackTimer();
    }
  });

  function clearClipboardCopied() {
    // Clears any pending dismiss timer and resets the clipboard copied state
    clearTimeoutState(clipboardDismissTimerState);
    hudStore.clearClipboardCopied();
  }

  /**
   * Handles clipboard copy detection for HUD notification.
   * Shows toast when user copies text while HUD is hidden and not currently synthesizing.
   * Automatically dismisses after triggerWindowMs + 200ms buffer.
   * @param triggerWindowMs - Duration to show the notification before auto-dismissal
   */
  function handleClipboardCopied(triggerWindowMs: number) {
    if (hudStore.isVisible || hudStore.isSynthesizing) return;

    clearClipboardCopied();
    hudStore.setClipboardDurationMs(triggerWindowMs);
    hudStore.setIsClipboardCopied(true);

    clipboardDismissTimerState = createTimeout(() => {
      hudStore.clearClipboardCopied();
      clipboardDismissTimerState = { timer: null };
    }, triggerWindowMs + 200);
  }

  onMount(async () => {
    if (isTauri) {
      // Production: Setup real Tauri event listeners for clipboard monitoring
      await setupEventListeners();
    }
  });

  onDestroy(() => {
    // Cleanup timers and event listeners to prevent memory leaks and stale callbacks
    // Critical because this component may persist across route changes
    stopElapsedTimer();
    stopPlaybackTimer();
    clearTimeoutState(clipboardDismissTimerState);
    cleanupEventListeners();
  });

  // Reactive effect that triggers clipboard notifications when store state changes
  // Monitors hudStore.isClipboardCopied to show notifications when double-copy is detected
  $effect(() => {
    if (hudStore.isClipboardCopied && clipboardDismissTimerState.timer === null) {
      handleClipboardCopied(hudStore.clipboardDurationMs);
    }
  });
</script>

<div
  class="hud-overlay"
  class:visible={hudStore.isVisible || hudStore.isClipboardCopied}
  class:has-content={hudStore.isVisible || hudStore.isClipboardCopied}
  style="border-radius: 12px;"
>
  {#if hudStore.isClipboardCopied && !hudStore.isVisible}
    <ClipboardNotification durationMs={hudStore.clipboardDurationMs} />
  {:else if hudStore.isVisible}
    <div class="hud-content">
      {#if hudStore.isSynthesizing}
        <HudSynthesisProgress
          estimatedDurationMs={hudStore.estimatedDurationMs}
          elapsedMs={hudStore.elapsedMs}
        />
      {:else}
        <HudPlaybackContent barValues={hudStore.barValues} spokenText={hudStore.spokenText} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .hud-overlay {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: stretch;
    opacity: 0;
    transition: opacity 0.2s ease-in-out;
    padding: 16px;
    box-sizing: border-box;
    background: transparent;
    border-radius: 12px;
  }

  .hud-overlay.visible {
    opacity: 1;
  }

  .hud-overlay.has-content {
    background: oklch(0.18 0.01 264.8 / 0.85);
  }

  .hud-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
