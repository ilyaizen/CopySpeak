<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { HTMLSelectAttributes } from "svelte/elements";

  export type SelectProps = HTMLSelectAttributes & {
    class?: string;
    ref?: HTMLSelectElement | null;
    options: Array<{ value: string; label: string }>;
  };
</script>

<script lang="ts">
  let {
    class: className,
    value = $bindable(),
    ref = $bindable(null),
    options,
    ...restProps
  }: SelectProps = $props();
</script>

<div class="relative inline-flex">
  <select
    bind:this={ref}
    bind:value
    data-slot="select"
    class={cn(
      "border-input h-9 w-full appearance-none rounded-sm border bg-transparent pr-10 pl-3 text-sm shadow-xs",
      "ring-offset-background",
      "focus:ring-ring focus:ring-1 focus:outline-none",
      "disabled:cursor-not-allowed disabled:opacity-50",
      "[&>option]:bg-background [&>option]:text-foreground",
      className
    )}
    {...restProps}
  >
    {#each options as option}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    class="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 opacity-50"
  >
    <path d="m6 9 6 6 6-6" />
  </svg>
</div>
