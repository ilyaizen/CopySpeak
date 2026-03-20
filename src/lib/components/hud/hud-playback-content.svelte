<script lang="ts">
  import Waveform from "../waveform.svelte";

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

  // Adjust duration based on playback speed (faster = shorter duration)
  let adjustedDurationMs = $derived(durationMs > 0 && speed > 0 ? durationMs / speed : 0);

  // Determine if text needs to scroll (is wider than container)
  let shouldScroll = $derived(
    adjustedDurationMs > 0 && textWidth > marqueeWidth && marqueeWidth > 0
  );

  // Calculate the exact pixel distance to scroll
  let scrollDistance = $derived(shouldScroll ? -(textWidth + marqueeWidth * 0.5) : 0);

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
  <!-- Progress fill background layer - animates across full duration -->
  {#if adjustedDurationMs > 0}
    <div class="progress-fill-bg" style="animation-duration: {adjustedDurationMs}ms;"></div>
  {/if}

  <!-- Waveform visualization layer -->
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

  <!-- Text marquee layer - overlayed on top -->
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

<style>
  .hud-playback-container {
    position: relative;
    flex: 1;
    min-height: 52px;
    overflow: hidden;
    border-radius: 100px;
    background: oklch(0.18 0.01 264.8 / 0.85);
  }

  /* Background progress fill - animates from left to right */
  .progress-fill-bg {
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
    transform: scaleX(0);
    animation-name: progress-fill;
    animation-timing-function: linear;
    animation-fill-mode: forwards;
    z-index: 0;
  }

  @keyframes progress-fill {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }

  /* Waveform layer - centered vertically */
  .waveform-layer {
    position: absolute;
    left: 20px;
    right: 20px;
    top: 50%;
    transform: translateY(-50%);
    height: 32px;
    pointer-events: none;
    z-index: 1;
  }

  /* Marquee wrapper - full size overlay */
  .marquee-wrapper {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    overflow: hidden;
    z-index: 2;
    pointer-events: none;
  }

  .marquee-wrapper.centered {
    justify-content: center;
  }

  /* The actual scrolling track */
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
    font-size: 1.25rem;
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

  /* Marquee animation using CSS custom property for dynamic end position */
  @keyframes marquee-scroll {
    0% {
      transform: translateX(0);
    }
    100% {
      transform: translateX(var(--end-pos));
    }
  }
</style>
