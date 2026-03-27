<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { Snippet } from "svelte";

  let {
    label,
    tooltip,
    children
  }: {
    label: string;
    tooltip?: string;
    children: Snippet;
  } = $props();
</script>

<div class="flex min-h-9 items-center justify-between gap-x-4 py-1">
  <div class="flex items-center gap-1.5">
    <Label>{label}</Label>
    {#if tooltip}
      <InfoTooltip text={tooltip} />
    {/if}
  </div>
  <div class="flex shrink-0 items-center justify-end">
    {@render children()}
  </div>
</div>
