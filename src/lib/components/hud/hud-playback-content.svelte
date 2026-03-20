<script lang="ts">
  import Waveform from "../waveform.svelte";
  import { formatTextForDisplay } from "$lib/utils/text.js";

  let { barValues, spokenText }: { barValues: number[]; spokenText: string | null } = $props();
</script>

<div class="hud-waveform-row">
  <Waveform
    {barValues}
    barColor="rgba(255, 255, 255, 0.3)"
    activeBarColor="rgba(96, 165, 250, 1)"
    barGap={3}
    barRadius={2}
    minBarHeight={0.15}
    attackRate={0.8}
    decayRate={0.5}
  />
</div>
<div class="hud-divider"></div>
{#if spokenText}
  <div class="hud-text-container">
    <div class="hud-text-content">{formatTextForDisplay(spokenText)}</div>
  </div>
{/if}

<style>
  .hud-waveform-row {
    flex-shrink: 0;
    height: 40px;
  }

  .hud-divider {
    flex-shrink: 0;
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
    margin: 0 -2px;
  }

  .hud-text-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    position: relative;
  }

  .hud-text-content {
    font-size: 11px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.75);
    line-clamp: 2;
    box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
</style>
