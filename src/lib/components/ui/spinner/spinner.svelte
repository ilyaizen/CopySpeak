<script lang="ts">
  import { cn } from "$lib/utils.js";
  import { Loader2, type LucideProps } from "@lucide/svelte";

  let {
    class: className,
    role = "status",
    "aria-label": ariaLabel = "Loading",
    ...restProps
  }: Partial<LucideProps> = $props();
</script>

<Loader2
  {role}
  aria-label={ariaLabel}
  class={cn("size-4 animate-spin", className)}
  {...restProps}
/>
