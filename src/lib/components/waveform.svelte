<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    /** 16 floats, 0.0–1.0, from AnalyserNode — empty [] during synthesis */
    barValues: number[];
    /** Bar color (CSS color string) */
    barColor?: string;
    /** Active bar color during playback */
    activeBarColor?: string;
    /** Background color */
    backgroundColor?: string;
    /** Gap between bars in pixels */
    barGap?: number;
    /** Minimum bar height as fraction of canvas height */
    minBarHeight?: number;
    /** Border radius of bars in pixels */
    barRadius?: number;
    /** Attack rate - how fast bars rise (0-1, higher = faster) */
    attackRate?: number;
    /** Decay rate - how fast bars fall (0-1, higher = faster) */
    decayRate?: number;
  }

  let {
    barValues = [],
    barColor = "rgba(255, 255, 255, 0.3)",
    activeBarColor = "rgba(96, 165, 250, 1)",
    backgroundColor = "transparent",
    barGap = 3,
    minBarHeight = 0.15,
    barRadius = 2,
    attackRate = 0.85,
    decayRate = 0.4
  }: Props = $props();

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;

  // Responsive canvas sizing
  let containerWidth = $state(0);
  let containerHeight = $state(0);

  // Smoothed bar values for animation (interpolated with attack/decay)
  let smoothedBars: number[] = [];

  // Animation frame ID for the render loop
  let animationFrameId: number | null = null;

  function resizeCanvas() {
    if (!canvas) return;

    const rect = canvas.parentElement?.getBoundingClientRect();
    if (rect) {
      containerWidth = rect.width;
      containerHeight = rect.height;

      // Set canvas size with device pixel ratio for crisp rendering
      const dpr = window.devicePixelRatio || 1;
      canvas.width = containerWidth * dpr;
      canvas.height = containerHeight * dpr;
      canvas.style.width = `${containerWidth}px`;
      canvas.style.height = `${containerHeight}px`;

      // Reset transform and apply fresh scale (prevents accumulation)
      ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
  }

  /**
   * Update smoothed bars with attack/decay interpolation.
   * - Attack: bars rise quickly to show responsiveness
   * - Decay: bars fall slower for more "weight" and visibility
   */
  function updateSmoothedBars() {
    if (!barValues.length) {
      smoothedBars = [];
      return;
    }

    // Initialize smoothed bars if needed
    if (smoothedBars.length !== barValues.length) {
      smoothedBars = [...barValues];
      return;
    }

    // Interpolate each bar with attack/decay
    for (let i = 0; i < barValues.length; i++) {
      const target = barValues[i];
      const current = smoothedBars[i];

      if (target > current) {
        // Attack: rise quickly
        smoothedBars[i] = current + (target - current) * attackRate;
      } else {
        // Decay: fall with force (faster drop)
        smoothedBars[i] = current + (target - current) * decayRate;
      }
    }
  }

  function drawWaveform() {
    if (!ctx) return;

    // Use smoothed bars if available, otherwise fall back to barValues
    const bars = smoothedBars.length ? smoothedBars : barValues;
    if (!bars.length) return;

    // Draw background if not transparent
    if (backgroundColor !== "transparent") {
      ctx.fillStyle = backgroundColor;
      ctx.fillRect(0, 0, containerWidth, containerHeight);
    } else {
      ctx.clearRect(0, 0, containerWidth, containerHeight);
    }

    const numBars = bars.length;
    const totalGapWidth = barGap * (numBars - 1);
    const barWidth = (containerWidth - totalGapWidth) / numBars;
    const maxBarHeight = containerHeight * 0.9;
    const minHeight = containerHeight * minBarHeight;

    for (let i = 0; i < numBars; i++) {
      const amplitude = bars[i];
      const barHeight = Math.max(minHeight, amplitude * maxBarHeight);
      const x = i * (barWidth + barGap);
      const y = (containerHeight - barHeight) / 2; // vertically centered
      // Use activeBarColor for live bars; barColor for silent/placeholder bars
      ctx.fillStyle = amplitude > minBarHeight ? activeBarColor : barColor;
      drawRoundedRect(ctx, x, y, barWidth, barHeight, barRadius);
    }
  }

  function drawRoundedRect(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    width: number,
    height: number,
    radius: number
  ) {
    const r = Math.min(radius, width / 2, height / 2);
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + width - r, y);
    ctx.quadraticCurveTo(x + width, y, x + width, y + r);
    ctx.lineTo(x + width, y + height - r);
    ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);
    ctx.lineTo(x + r, y + height);
    ctx.quadraticCurveTo(x, y + height, x, y + height - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
    ctx.fill();
  }

  // Animation loop - runs continuously for smooth decay animation
  function animationLoop() {
    updateSmoothedBars();
    drawWaveform();
    animationFrameId = requestAnimationFrame(animationLoop);
  }

  // Start/stop animation based on barValues
  $effect(() => {
    barValues; // reference to establish reactivity

    if (barValues.length > 0 && animationFrameId === null) {
      // Start animation loop when we have data
      animationLoop();
    } else if (barValues.length === 0 && animationFrameId !== null) {
      // Stop animation loop when data is cleared (saves CPU when idle)
      cancelAnimationFrame(animationFrameId);
      animationFrameId = null;
    }
  });

  // Redraw on canvas resize
  $effect(() => {
    if (containerWidth && containerHeight) {
      ctx = canvas?.getContext("2d") || null;
      drawWaveform();
    }
  });

  onMount(() => {
    ctx = canvas.getContext("2d");
    resizeCanvas();

    // Handle container resize
    const resizeObserver = new ResizeObserver(() => {
      resizeCanvas();
      drawWaveform();
    });

    if (canvas.parentElement) {
      resizeObserver.observe(canvas.parentElement);
    }

    return () => {
      resizeObserver.disconnect();
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
      }
    };
  });
</script>

<div class="waveform-container">
  <canvas bind:this={canvas} class="waveform-canvas"></canvas>
</div>

<style>
  .waveform-container {
    width: 100%;
    height: 100%;
    position: relative;
  }

  .waveform-canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
