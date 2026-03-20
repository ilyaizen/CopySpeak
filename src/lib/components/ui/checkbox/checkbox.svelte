<script lang="ts">
  import { cn } from "$lib/utils.js";

  let {
    checked = false,
    disabled = false,
    onchange,
    class: className,
    ...props
  } = $props<{
    checked?: boolean;
    disabled?: boolean;
    onchange?: (event: Event) => void;
    class?: string;
  }>();

  // svelte-ignore state_referenced_locally
  let internalChecked = $state(checked);

  function handleToggle() {
    if (disabled) return;
    internalChecked = !internalChecked;
    onchange?.(new Event("change"));
  }
</script>

<button
  type="button"
  role="checkbox"
  aria-checked={internalChecked}
  {disabled}
  class={cn(
    "peer border-primary ring-offset-background focus-visible:ring-ring inline-flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50",
    internalChecked && "bg-primary text-primary-foreground",
    !internalChecked && "bg-background hover:bg-accent hover:text-accent-foreground",
    className
  )}
  {...props}
  onclick={handleToggle}
>
  {#if internalChecked}
    <svg
      xmlns="http://www.w3.org/2000/svg"
      class="h-3 w-3"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="3"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <polyline points="20 6 9 17 4 12"></polyline>
    </svg>
  {/if}
</button>
