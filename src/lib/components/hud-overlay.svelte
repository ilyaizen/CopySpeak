<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { isTauri } from "$lib/services/tauri.js";
  import { hudStore } from "$lib/stores/hud-store.svelte.js";
  import { useHudEvents } from "$lib/composables/use-hud-events.js";
  import { createTimer, clearTimer, createTimeout, clearTimeoutState } from "$lib/utils/timer.js";
  import {
    ClipboardNotification,
    HudStatus,
    HudSynthesisProgress,
    HudPlaybackContent
  } from "./hud/index.js";

  // Timer state
  let elapsedTimerState = $state<{
    timer: ReturnType<typeof setInterval> | null;
    startTime: number | null;
  }>({
    timer: null,
    startTime: null
  });
  let clipboardDismissTimerState = $state<{ timer: ReturnType<typeof setTimeout> | null }>({
    timer: null
  });

  // Dev mode animation
  let devAnimationId: number | null = null;

  // Event listeners
  const { setupEventListeners, cleanupEventListeners } = useHudEvents();

  function startElapsedTimer() {
    clearTimer(elapsedTimerState);
    elapsedTimerState = createTimer((elapsed) => {
      hudStore.setElapsedMs(elapsed);
    }, 100);
  }

  function stopElapsedTimer() {
    clearTimer(elapsedTimerState);
  }

  // Watch for synthesis state changes to manage timer
  $effect(() => {
    if (hudStore.isSynthesizing && elapsedTimerState.timer === null) {
      startElapsedTimer();
    } else if (!hudStore.isSynthesizing && elapsedTimerState.timer !== null) {
      stopElapsedTimer();
    }
  });

  function clearClipboardCopied() {
    clearTimeoutState(clipboardDismissTimerState);
    hudStore.clearClipboardCopied();
  }

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

  function animateDevBars() {
    const bars: number[] = [0, 0];
    for (let i = 0; i < 10; i++) {
      const t = i / 10;
      const wave1 = Math.sin(t * Math.PI * 3 + Date.now() / 200) * 0.3;
      const wave2 = Math.sin(t * Math.PI * 5 + Date.now() / 150) * 0.2;
      const wave3 = Math.sin(t * Math.PI * 7 + Date.now() / 100) * 0.1;
      const value = 0.4 + wave1 + wave2 + wave3;
      bars.push(Math.max(0.1, Math.min(1, value)));
    }
    bars.push(0, 0, 0, 0);
    hudStore.setBarValues(bars);
    devAnimationId = requestAnimationFrame(animateDevBars);
  }

  onMount(async () => {
    if (isTauri) {
      await setupEventListeners();
    } else {
      // Dev mode: setup mock data and animation
      animateDevBars();
      hudStore.setSpokenText(
        "The quick brown fox jumps over the lazy dog. Once upon a time there was a fox who lived in the forest and had many adventures with friends."
      );
      hudStore.setProvider("ElevenLabs");
      hudStore.setVoice("Rachel");
      hudStore.setIsVisible(true);
    }
  });

  onDestroy(() => {
    stopElapsedTimer();
    clearTimeoutState(clipboardDismissTimerState);
    cleanupEventListeners();
    if (devAnimationId !== null) {
      cancelAnimationFrame(devAnimationId);
      devAnimationId = null;
    }
  });

  // Watch for clipboard copied events from store
  $effect(() => {
    if (hudStore.isClipboardCopied && clipboardDismissTimerState.timer === null) {
      handleClipboardCopied(hudStore.clipboardDurationMs);
    }
  });
</script>

<div
  class="hud-overlay"
  class:visible={hudStore.isVisible || hudStore.isClipboardCopied}
  class:has-content={hudStore.isVisible}
  style="border-radius: 12px;"
>
  {#if hudStore.isClipboardCopied && !hudStore.isVisible}
    <ClipboardNotification durationMs={hudStore.clipboardDurationMs} />
  {:else if hudStore.isVisible}
    <div class="hud-content">
      <HudStatus
        isSynthesizing={hudStore.isSynthesizing}
        isPaused={hudStore.isPaused}
        statusLabel={hudStore.statusLabel}
        providerVoiceLabel={hudStore.providerVoiceLabel}
        isPaginated={hudStore.isPaginated}
        currentFragment={hudStore.currentFragment}
        totalFragments={hudStore.totalFragments}
      />

      {#if hudStore.isSynthesizing}
        <HudSynthesisProgress
          hasEstimate={hudStore.hasEstimate}
          progressPercent={hudStore.progressPercent}
          estimatedTotalMs={hudStore.estimatedDurationMs}
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
    padding: 10px 12px;
    box-sizing: border-box;
    background: transparent;
  }

  .hud-overlay.visible {
    opacity: 1;
  }

  .hud-overlay.has-content {
    background: rgba(0, 0, 0, 0.75);
  }

  .hud-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
