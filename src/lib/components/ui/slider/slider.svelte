<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { HTMLAttributes } from "svelte/elements";

  export type SliderProps = Omit<HTMLAttributes<HTMLDivElement>, "onchange"> & {
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
    ...restProps
  }: SliderProps = $props();

  // Calculate percentage for thumb position
  const percentage = $derived(((value - min) / (max - min)) * 100);
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
    onchange={() => onchange?.(value)}
    aria-label={ariaLabel}
    data-slot="slider"
    class="absolute h-full w-full cursor-pointer opacity-0 disabled:cursor-not-allowed"
  />
  <div
    class="border-primary/50 bg-background pointer-events-none absolute block h-4 w-4 rounded-full border shadow transition-colors"
    style="left: calc({percentage}% - 8px)"
  ></div>
</div>
