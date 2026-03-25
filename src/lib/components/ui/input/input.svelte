<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { HTMLInputAttributes } from "svelte/elements";

  export type InputProps = HTMLInputAttributes & {
    class?: string;
    ref?: HTMLInputElement | null;
  };
</script>

<script lang="ts">
  let {
    class: className,
    type = "text",
    value = $bindable(),
    ref = $bindable(null),
    ...restProps
  }: InputProps = $props();
</script>

<input
  bind:this={ref}
  bind:value
  {type}
  data-slot="input"
  class={cn(
    "border-input flex h-9 w-full rounded-sm border bg-transparent px-3 py-1 text-sm shadow-xs transition-colors",
    "file:text-foreground file:border-0 file:bg-transparent file:text-sm file:font-medium",
    "placeholder:text-muted-foreground",
    "focus-visible:ring-ring focus-visible:ring-1 focus-visible:outline-none",
    "disabled:cursor-not-allowed disabled:opacity-50",
    className
  )}
  {...restProps}
/>
