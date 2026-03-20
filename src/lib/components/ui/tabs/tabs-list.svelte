<script lang="ts" module>
  import type { TabsListProps } from "./tabs.svelte";
  export type { TabsListProps };
</script>

<script lang="ts">
  import { cn } from "$lib/utils.js";

  let { class: className, children, ...restProps }: TabsListProps = $props();
</script>

<div
  data-slot="tabs-list"
  class={cn(
    "bg-muted text-muted-foreground inline-flex h-9 items-center justify-center rounded-none p-1",
    className
  )}
  {...restProps}
>
  {@render children?.()}
</div>
