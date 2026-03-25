<script lang="ts">
  import Progress from "$lib/components/ui/progress/progress.svelte";

  let { durationMs }: { durationMs: number } = $props();

  // Convert ms to seconds for CSS animation
  let durationSec = $derived(durationMs / 1000);
</script>

<div class="clipboard-notification">
  <span class="clipboard-title">Clipboard Copied</span>
  <Progress value={100} max={100} class="progress-bar" style="--duration: {durationSec}s" />
</div>

<style>
  .clipboard-notification {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    animation: notification-in 0.4s cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  @keyframes notification-in {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .clipboard-title {
    font-size: 18px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .progress-bar :global([data-slot="progress-indicator"]) {
    animation: progress-fill var(--duration) cubic-bezier(0, 0.7, 0.1, 1) forwards;
    transform-origin: left;
  }

  @keyframes progress-fill {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(0);
    }
  }
</style>
