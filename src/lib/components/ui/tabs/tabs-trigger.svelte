<script lang="ts" module>
  import type { TabsTriggerProps, TabsContext } from "./tabs.svelte";
  export type { TabsTriggerProps };
</script>

<script lang="ts">
  import { cn } from "$lib/utils.js";
  import { getContext, onMount, tick } from "svelte";

  let {
    class: className,
    value,
    disabled = false,
    children,
    ...restProps
  }: TabsTriggerProps = $props();

  const ctx = getContext<TabsContext>("tabs-context");

  // Track the current tab value from context
  let currentValue = $state(ctx.getValue());

  // Subscribe to tab changes using onMount
  onMount(() => {
    console.log("[TabsTrigger] onMount called for value:", value);
    const unsubscribe = ctx.subscribe(async (v) => {
      console.log(
        "[TabsTrigger] subscribe callback received:",
        v,
        "my value:",
        value,
        "was:",
        currentValue
      );
      currentValue = v;
      await tick();
      console.log("[TabsTrigger] after tick, currentValue:", currentValue);
    });
    return unsubscribe;
  });

  const isActive = $derived(currentValue === value);

  function handleClick() {
    console.log("[TabsTrigger] handleClick called, value:", value, "disabled:", disabled);
    if (!disabled) {
      console.log("[TabsTrigger] calling ctx.setValue with:", value);
      ctx.setValue(value);
      console.log("[TabsTrigger] after setValue, getValue returns:", ctx.getValue());
    }
  }
</script>

<button
  type="button"
  role="tab"
  aria-selected={isActive}
  {disabled}
  data-slot="tabs-trigger"
  data-state={isActive ? "active" : "inactive"}
  class={cn(
    "ring-offset-background inline-flex items-center justify-center rounded-sm px-3 py-1 text-sm font-medium whitespace-nowrap transition-all",
    "focus-visible:ring-ring focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none",
    "disabled:pointer-events-none disabled:opacity-50",
    isActive
      ? "bg-background text-foreground shadow"
      : "hover:bg-background/50 hover:text-foreground",
    className
  )}
  onclick={handleClick}
  {...restProps}
>
  {@render children?.()}
</button>
