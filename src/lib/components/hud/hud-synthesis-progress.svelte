<script lang="ts">
  let {
    estimatedDurationMs,
    elapsedMs
  }: {
    estimatedDurationMs: number | null;
    elapsedMs: number;
  } = $props();

  // Calculate remaining time for animation
  let remainingMs = $derived(
    estimatedDurationMs !== null && estimatedDurationMs > elapsedMs
      ? estimatedDurationMs - elapsedMs
      : 0
  );

  // Only animate if we have a valid estimate
  let shouldAnimate = $derived(estimatedDurationMs !== null && estimatedDurationMs > 0);
</script>

<div class="synthesis-progress-container">
  <div class="progress-pill" class:animate={shouldAnimate} style="--dur: {remainingMs}ms;">
    {#if shouldAnimate}
      <div class="progress-fill"></div>
    {/if}
  </div>
</div>

<style>
  .synthesis-progress-container {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .progress-pill {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 24px;
    background: oklch(0.18 0.01 264.8 / 0.85);
    border-radius: 100px;
    overflow: hidden;
    min-width: 120px;
  }

  .progress-pill.animate {
    animation: pill-in 0.4s cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  .progress-fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 100%;
    background: linear-gradient(
      90deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(54.3% 0.2 259.815) 100%
    );
    transform-origin: left;
    animation: fill-progress var(--dur, 3000ms) linear forwards;
  }

  @keyframes pill-in {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @keyframes fill-progress {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }
</style>
