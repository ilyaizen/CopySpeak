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

<div class="flex min-h-9 min-w-0 flex-wrap items-center justify-between gap-x-4 gap-y-2 py-2">
  <div class="flex min-w-0 items-center gap-1.5">
    <Label>{label}</Label>
    {#if tooltip}
      <InfoTooltip text={tooltip} />
    {/if}
  </div>
  <div class="flex max-w-full min-w-0 items-center justify-end [&>*]:max-w-full">
    {@render children()}
  </div>
</div>
