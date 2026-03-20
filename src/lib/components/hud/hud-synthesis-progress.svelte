<script lang="ts">
  let {
    hasEstimate,
    progressPercent,
    estimatedTotalMs,
    elapsedMs
  }: {
    hasEstimate: boolean;
    progressPercent: number;
    estimatedTotalMs: number | null;
    elapsedMs: number;
  } = $props();

  function formatTime(ms: number): string {
    const seconds = Math.ceil(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  function formatRemaining(elapsed: number, total: number | null): string {
    if (total === null || total <= 0) return "";
    const remaining = Math.max(0, total - elapsed);
    return formatTime(remaining);
  }

  let remainingTime = $derived(formatRemaining(elapsedMs, estimatedTotalMs));
  let displayPercent = $derived(Math.min(99, Math.max(0, progressPercent)));
</script>

<div class="synthesis-progress-container">
  <div class="progress-pill" style="--progress: {displayPercent}%;">
    <div class="progress-track"></div>
    <span class="progress-label">
      {#if hasEstimate && remainingTime}
        Processing {remainingTime}
      {:else}
        Processing<span class="dots"></span>
      {/if}
    </span>
  </div>
</div>

<style>
  @property --progress {
    syntax: "<percentage>";
    initial-value: 0%;
    inherits: true;
  }

  .synthesis-progress-container {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px 0;
  }

  .progress-pill {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 140px;
    padding: 10px 20px;
    background: oklch(0.18 0.01 264.8/ 0.85);
    border-radius: 100px;
    overflow: hidden;
  }

  .progress-pill::before {
    content: "";
    position: absolute;
    inset: -2px;
    border-radius: 100px;
    padding: 2px;
    background: conic-gradient(
      from 180deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(62.3% 0.214 259.815) var(--progress),
      oklch(62.3% 0.214 259.815/ 0.2) var(--progress),
      oklch(62.3% 0.214 259.815/ 0.2) 100%
    );
    mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    mask-composite: exclude;
    transition: --progress 0.4s cubic-bezier(0, 0.7, 0.1, 1);
  }

  .progress-pill::after {
    content: "";
    position: absolute;
    inset: -2px;
    border-radius: 100px;
    padding: 2px;
    background: conic-gradient(
      from 180deg,
      oklch(62.3% 0.214 259.815/ 0.1) 0%,
      oklch(62.3% 0.214 259.815/ 0.1) var(--progress),
      oklch(62.3% 0.214 259.815/ 0) var(--progress),
      oklch(62.3% 0.214 259.815/ 0) 100%
    );
    mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    mask-composite: exclude;
    animation: subtle-pulse 2s ease-in-out infinite;
  }

  .progress-track {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      oklch(54.3% 0.2 259.815/ 0) 0%,
      oklch(54.3% 0.2 259.815/ 0.08) var(--progress),
      oklch(54.3% 0.2 259.815/ 0) calc(var(--progress) + 8%)
    );
    transition: --progress 0.4s cubic-bezier(0, 0.7, 0.1, 1);
  }

  .progress-label {
    position: relative;
    font-size: 12px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
    z-index: 1;
  }

  .dots::after {
    content: "...";
    animation: dots 1.2s steps(4, end) infinite;
  }

  @keyframes dots {
    0%,
    20% {
      content: "";
    }
    40% {
      content: ".";
    }
    60% {
      content: "..";
    }
    80%,
    100% {
      content: "...";
    }
  }

  @keyframes subtle-pulse {
    0%,
    100% {
      opacity: 0.3;
    }
    50% {
      opacity: 0.6;
    }
  }
</style>
