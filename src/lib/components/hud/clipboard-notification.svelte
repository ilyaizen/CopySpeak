<script lang="ts">
  import Progress from "$lib/components/ui/progress/progress.svelte";
  import { onMount } from "svelte";

  let { durationMs }: { durationMs: number } = $props();

  let progressValue = $state(0);
  let startTime = $state(0);

  onMount(() => {
    startTime = Date.now();
    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      progressValue = Math.min(100, (elapsed / durationMs) * 100);
      if (progressValue >= 100) {
        clearInterval(interval);
      }
    }, 16);

    return () => clearInterval(interval);
  });
</script>

<div class="clipboard-notification">
  <span class="clipboard-title">Clipboard Copied</span>
  <Progress value={progressValue} max={100} class="progress-bar" />
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

  .progress-bar {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: oklch(0.25 0.02 264.8 / 0.5);
  }

  .progress-bar :global([data-slot="progress-indicator"]) {
    background: linear-gradient(
      90deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(54.3% 0.2 259.815) 100%
    );
    border-radius: 3px;
  }
</style>
