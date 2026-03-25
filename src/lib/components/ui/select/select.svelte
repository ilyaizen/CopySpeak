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

<select
  bind:this={ref}
  bind:value
  data-slot="select"
  class={cn(
    "border-input flex h-9 w-full items-center justify-between rounded-sm border bg-transparent px-3 py-2 text-sm shadow-xs",
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
