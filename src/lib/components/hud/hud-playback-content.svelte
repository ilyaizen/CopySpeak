<script lang="ts">
  import { onMount } from "svelte";
  import Waveform from "../waveform.svelte";
  import Progress from "#lib/components/ui/progress/progress.svelte";
  import { hudStore } from "#lib/stores/hud-store.svelte.js";
  import {
    activeCaptionWord,
    buildCaptions,
    captionPhrase,
    spokenUntil
  } from "#lib/models/captions.js";

  let { barValues, spokenText }: { barValues: number[]; spokenText: string | null } = $props();

  // Canvas fillStyle needs resolved colors, not var() references. The HUD window
  // is forced dark, so read the tokens once on mount.
  let waveformBarColor = $state("rgba(255, 255, 255, 0.3)");
  let waveformActiveBarColor = $state("rgba(255, 255, 255, 1)");
  onMount(() => {
    const style = getComputedStyle(document.documentElement);
    const foreground = style.getPropertyValue("--foreground").trim();
    const primary = style.getPropertyValue("--primary").trim();
    waveformBarColor = `color-mix(in srgb, ${foreground} 30%, transparent)`;
    waveformActiveBarColor = primary;
  });
  let words = $derived(buildCaptions(spokenText ?? "", hudStore.caption?.captions));
  let positionIndex = $derived(activeCaptionWord(words, hudStore.caption?.position_ms ?? 0));
  let activeIndex = $derived(hudStore.caption?.active ? positionIndex : -1);
  // Retain the current phrase during buffering; only the highlight goes away.
  let phraseIndex = $derived(captionPhrase(words, hudStore.caption?.position_ms ?? 0));
  let phrase = $derived(words.filter((word) => word.phrase === phraseIndex));
  let spokenEnd = $derived(spokenUntil(words, hudStore.caption?.position_ms ?? 0));
</script>

<div class="hud-playback-container">
  <Progress
    value={hudStore.isPlaybackReady ? hudStore.playbackProgressPercent : 0}
    max={100}
    class="progress-bar"
  />
  <div class="content-layer">
    {#if spokenText}
      <p class="caption" dir="auto">
        {#each phrase as word (word.offset)}
          <span
            class:spoken={word.end !== null && word.end <= spokenEnd}
            class:current={word === words[activeIndex]}>{word.text}</span
          >
        {:else}
          {spokenText}
        {/each}
      </p>
    {:else}
      <Waveform
        {barValues}
        barColor={waveformBarColor}
        activeBarColor={waveformActiveBarColor}
        barGap={3}
        barRadius={2}
        minBarHeight={0.15}
        attackRate={0.8}
        decayRate={0.5}
      />
    {/if}
  </div>
</div>

<style>
  .hud-playback-container {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .content-layer {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .caption {
    margin: 0;
    width: 100%;
    color: var(--foreground);
    font-size: 18px;
    font-weight: 600;
    line-height: 1.45;
    text-align: center;
    text-wrap: balance;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .caption span {
    border-radius: 2px;
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }
  .caption .spoken {
    color: var(--muted-foreground);
  }
  .caption .current {
    color: var(--primary-foreground);
    background: var(--primary);
    box-shadow: 0 0 0 2px var(--primary);
  }
</style>
