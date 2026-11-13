<script lang="ts">
  let {
    isSynthesizing,
    isPaused,
    statusLabel,
    providerVoiceLabel,
    isPaginated,
    currentFragment,
    totalFragments
  }: {
    isSynthesizing: boolean;
    isPaused: boolean;
    statusLabel: string;
    providerVoiceLabel: string | null;
    isPaginated: boolean;
    currentFragment: number | null;
    totalFragments: number | null;
  } = $props();

  let dotPulsing = $derived(isSynthesizing || (!isPaused && !isSynthesizing));
</script>

<div class="hud-status-row">
  <div class="status-left">
    {#if isSynthesizing}
      <div class="spinner"></div>
    {:else}
      <div class="status-dot" class:pulsing={dotPulsing}></div>
    {/if}
    <span class="status-label">{statusLabel}</span>
    {#if providerVoiceLabel}
      <span class="provider-voice">{providerVoiceLabel}</span>
    {/if}
  </div>
  {#if isPaginated && currentFragment !== null && totalFragments !== null}
    <div class="pagination-badge">
      <span>{currentFragment + 1}/{totalFragments}</span>
    </div>
  {/if}
</div>

<style>
  .hud-status-row {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 20px;
  }

  .status-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
  }

  .status-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--primary);
  }

  .status-dot.pulsing {
    animation: dot-pulse 1.4s ease-in-out infinite;
  }

  @keyframes dot-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .status-label {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--foreground);
    letter-spacing: 0.02em;
  }

  .provider-voice {
    font-size: 10px;
    color: var(--muted-foreground);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .pagination-badge {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 1px 7px;
    background: color-mix(in srgb, var(--foreground) 15%, transparent);
    border-radius: 10px;
    font-size: 10px;
    color: var(--foreground);
    font-weight: 500;
    white-space: nowrap;
  }

  .spinner {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border: 2px solid color-mix(in srgb, var(--foreground) 25%, transparent);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
