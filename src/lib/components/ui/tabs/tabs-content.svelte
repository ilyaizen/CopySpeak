<script lang="ts" module>
  import type { TabsContentProps, TabsContext } from "./tabs.svelte";
  export type { TabsContentProps };
</script>

<script lang="ts">
  import { cn } from "$lib/utils.js";
  import { getContext, onMount, tick } from "svelte";

  let { class: className, value, children, ...restProps }: TabsContentProps = $props();

  const ctx = getContext<TabsContext>("tabs-context");

  // Track the current tab value from context
  let currentValue = $state(ctx.getValue());

  // Track render key to force re-render of children when switching to this tab
  let renderKey = $state(0);

  // Subscribe to tab changes using onMount
  onMount(() => {
    const unsubscribe = ctx.subscribe(async (v) => {
      const wasActive = currentValue === value;
      currentValue = v;
      const isNowActive = v === value;

      // Force re-render of children when becoming active
      if (!wasActive && isNowActive) {
        renderKey++;
      }
      await tick();
    });
    return unsubscribe;
  });

  // Derive active state from the current value
  const isActive = $derived(currentValue === value);
</script>

<!-- Always render to maintain subscription, but hide when inactive using visibility -->
<div
  data-slot="tabs-content"
  data-state={isActive ? "active" : "inactive"}
  style={!isActive ? "display: none;" : ""}
  class={cn(
    "ring-offset-background focus-visible:ring-ring mt-2 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none",
    className
  )}
  {...restProps}
>
  {#key renderKey}
    {@render children?.()}
  {/key}
</div>
