<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { HTMLButtonAttributes } from "svelte/elements";

  export type SwitchProps = Omit<HTMLButtonAttributes, "onchange"> & {
    class?: string;
    checked?: boolean;
    disabled?: boolean;
    id?: string;
    name?: string;
    "aria-label"?: string;
    onchange?: (checked: boolean) => void;
  };
</script>

<script lang="ts">
  let {
    class: className,
    checked = $bindable(false),
    disabled = false,
    id,
    name,
    "aria-label": ariaLabel,
    onchange,
    ...restProps
  }: SwitchProps = $props();

  function handleClick() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      checked = !checked;
      onchange?.(checked);
    }
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel}
  {id}
  {name}
  {disabled}
  data-slot="switch"
  class={cn(
    "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center justify-between rounded-sm border-2 border-transparent shadow-xs transition-colors",
    "focus-visible:ring-ring focus-visible:ring-offset-background focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none",
    "disabled:cursor-not-allowed disabled:opacity-50",
    checked ? "bg-primary" : "bg-input",
    className
  )}
  onclick={handleClick}
  onkeydown={handleKeydown}
  {...restProps}
>
  <!-- Use opacity instead of translate for better RTL support -->
  <span
    class={cn(
      "bg-background pointer-events-none block h-4 w-4 rounded-sm shadow-lg ring-0 transition-opacity",
      checked ? "opacity-0" : "opacity-100"
    )}
  ></span>
  <span
    class={cn(
      "bg-background pointer-events-none block h-4 w-4 rounded-sm shadow-lg ring-0 transition-opacity",
      checked ? "opacity-100" : "opacity-0"
    )}
  ></span>
</button>
