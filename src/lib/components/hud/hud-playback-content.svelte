<script lang="ts">
  import Waveform from "../waveform.svelte";
  import Progress from "$lib/components/ui/progress/progress.svelte";
  import { hudStore } from "$lib/stores/hud-store.svelte.js";
  import { activeCaptionWord, buildCaptions } from "$lib/models/captions.js";

  let { barValues, spokenText }: { barValues: number[]; spokenText: string | null } = $props();
  let words = $derived(buildCaptions(spokenText ?? ""));
  let positionIndex = $derived(
    activeCaptionWord(words, hudStore.caption?.position_ms ?? 0, hudStore.caption?.duration_ms ?? 0)
  );
  let activeIndex = $derived(hudStore.caption?.active ? positionIndex : -1);
  // Retain the current phrase during buffering; only the highlight goes away.
  let phraseIndex = $derived(words[Math.max(0, positionIndex)]?.phrase ?? 0);
  let phrase = $derived(words.filter((word) => word.phrase === phraseIndex));
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
        {#each phrase as word (word.start)}
          <span
            class:spoken={activeIndex >= 0 && word.end <= (words[activeIndex]?.start ?? 0)}
            class:current={word === words[activeIndex]}>{word.text}</span
          >
        {:else}
          {spokenText}
        {/each}
      </p>
    {:else}
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
    color: #e2e8f0;
    font-size: 18px;
    font-weight: 600;
    line-height: 1.45;
    text-align: center;
    text-wrap: balance;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .caption span {
    border-radius: 4px;
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }
  .caption .spoken {
    color: #aebdd0;
  }
  .caption .current {
    color: #101827;
    background: #93c5fd;
    box-shadow: 0 0 0 2px #93c5fd;
  }
</style>
