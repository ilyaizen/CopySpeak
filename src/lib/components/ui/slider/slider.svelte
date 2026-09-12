<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { HTMLAttributes } from "svelte/elements";

  export type SliderProps = Omit<HTMLAttributes<HTMLDivElement>, "onchange" | "oninput"> & {
    class?: string;
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    id?: string;
    name?: string;
    "aria-label"?: string;
    onchange?: (value: number) => void;
    /** Fires on every drag tick. Use for live display only; commit in onchange. */
    oninput?: (value: number) => void;
  };
</script>

<script lang="ts">
  let {
    class: className,
    value = $bindable(0),
    min = 0,
    max = 100,
    step = 1,
    disabled = false,
    id,
    name,
    "aria-label": ariaLabel,
    onchange,
    oninput,
    ...restProps
  }: SliderProps = $props();

  // Calculate percentage for thumb position
  const percentage = $derived(((value - min) / (max - min)) * 100);

  // Snap to step and strip float noise (step 0.05 yields values like 0.9500000000000001).
  const snap = (v: number) => Number((min + Math.round((v - min) / step) * step).toFixed(6));
</script>

<div class={cn("relative flex w-full items-center select-none", className)} {...restProps}>
  <div class="bg-primary/20 relative h-1.5 w-full grow overflow-hidden rounded-full">
    <div class="bg-primary absolute h-full" style="width: {percentage}%"></div>
  </div>
  <input
    type="range"
    {id}
    {min}
    {max}
    {step}
    {disabled}
    bind:value
    oninput={() => {
      value = snap(value);
      oninput?.(value);
    }}
    onchange={() => {
      value = snap(value);
      onchange?.(value);
    }}
    aria-label={ariaLabel}
    data-slot="slider"
    class="absolute h-full w-full cursor-pointer opacity-0 disabled:cursor-not-allowed"
  />
  <div
    class="bg-primary pointer-events-none absolute block h-2.5 w-2.5 rounded-full transition-colors"
    style="inset-inline-start: calc({percentage}% - 5px)"
  ></div>
</div>

<style>
  /* Collapse the native thumb so its travel spans the full track — otherwise
     the browser reserves half a thumb at each edge and positions never reach 0%/100%. */
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 0;
  }
  input[type="range"]::-moz-range-thumb {
    width: 0;
    border: none;
  }
</style>
