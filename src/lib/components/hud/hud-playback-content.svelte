<script lang="ts">
  import Waveform from "../waveform.svelte";
  import Progress from "$lib/components/ui/progress/progress.svelte";
  import { hudStore } from "$lib/stores/hud-store.svelte.js";

  let {
    barValues,
    spokenText,
    durationMs = 0,
    speed = 1.0
  }: {
    barValues: number[];
    spokenText: string | null;
    durationMs?: number;
    speed?: number;
  } = $props();

  let textEl: HTMLSpanElement | undefined = $state();
  let marqueeWrapperEl: HTMLDivElement | undefined = $state();
  let textWidth = $state(0);
  let marqueeWidth = $state(0);

  let adjustedDurationMs = $derived(durationMs > 0 && speed > 0 ? durationMs / speed : 0);

  let shouldScroll = $derived(
    adjustedDurationMs > 0 && textWidth > marqueeWidth && marqueeWidth > 0
  );

  let scrollDistance = $derived(shouldScroll ? -(textWidth + marqueeWidth * 0.5) : 0);

  let progressPercent = $derived(hudStore.playbackProgressPercent);

  $effect(() => {
    if (textEl) {
      textWidth = textEl.offsetWidth;
    }
  });

  $effect(() => {
    if (marqueeWrapperEl) {
      marqueeWidth = marqueeWrapperEl.offsetWidth;
    }
  });
</script>

<div class="hud-playback-container">
  <Progress value={progressPercent} max={100} class="progress-bar" />

  <div class="content-layer">
    <div class="waveform-layer">
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

    {#if spokenText}
      <div class="marquee-wrapper" bind:this={marqueeWrapperEl} class:centered={!shouldScroll}>
        <div
          class="marquee-track"
          class:animating={shouldScroll}
          style="animation-duration: {adjustedDurationMs}ms; --end-pos: {scrollDistance}px;"
        >
          <span bind:this={textEl} class="marquee-text">{spokenText}</span>
          {#if shouldScroll}
            <span class="marquee-spacer"></span>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .hud-playback-container {
    position: relative;
    flex: 1;
    min-height: 52px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: oklch(0.25 0.02 264.8 / 0.5);
    flex-shrink: 0;
  }

  .progress-bar :global([data-slot="progress-indicator"]) {
    background: linear-gradient(
      90deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(54.3% 0.2 259.815) 100%
    );
    border-radius: 3px;
  }

  .content-layer {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .waveform-layer {
    position: relative;
    left: 20px;
    right: 20px;
    height: 32px;
    pointer-events: none;
  }

  .marquee-wrapper {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    overflow: hidden;
    pointer-events: none;
  }

  .marquee-wrapper.centered {
    justify-content: center;
  }

  .marquee-track {
    display: flex;
    align-items: center;
    will-change: transform;
  }

  .marquee-track.animating {
    animation-name: marquee-scroll;
    animation-timing-function: linear;
    animation-fill-mode: forwards;
  }

  .marquee-text {
    font-size: 18px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
    text-shadow:
      0 1px 2px rgba(0, 0, 0, 0.8),
      0 0 8px rgba(0, 0, 0, 0.6);
    flex-shrink: 0;
  }

  .marquee-spacer {
    flex-shrink: 0;
    width: 100px;
  }

  @keyframes marquee-scroll {
    0% {
      transform: translateX(0);
    }
    100% {
      transform: translateX(var(--end-pos));
    }
  }
</style>
