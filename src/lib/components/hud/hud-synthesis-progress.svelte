<script lang="ts">
  import Progress from "$lib/components/ui/progress/progress.svelte";
  import { hudStore } from "$lib/stores/hud-store.svelte.js";

  let {
    estimatedDurationMs,
    elapsedMs
  }: {
    estimatedDurationMs: number | null;
    elapsedMs: number;
  } = $props();

  let progressPercent = $derived(
    estimatedDurationMs !== null && estimatedDurationMs > 0
      ? Math.min(100, (elapsedMs / estimatedDurationMs) * 100)
      : 0
  );

  let providerLabel = $derived(hudStore.providerVoiceLabel);
</script>

<div class="synthesis-progress-container">
  <span class="status-text">Processing...</span>
  <Progress value={progressPercent} max={100} class="progress-bar" />
  {#if providerLabel}
    <span class="provider-info">{providerLabel}</span>
  {/if}
</div>

<style>
  .synthesis-progress-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    animation: container-in 0.4s cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  @keyframes container-in {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .status-text {
    font-size: 18px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .provider-info {
    font-size: 14px;
    font-weight: 500;
    color: oklch(0.7 0.02 264.8);
    letter-spacing: 0.01em;
    white-space: nowrap;
  }
</style>
