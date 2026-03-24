<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable(),
    errors
  }: {
    localConfig: AppConfig;
    errors: Record<string, string>;
  } = $props();
</script>

<div class="border-border bg-card rounded-lg border p-4 shadow-sm">
  <h3 class="text-card-foreground mb-4 text-lg font-medium">{$_("settings.pagination.title")}</h3>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1.5">
        <Label for="pagination-enabled">{$_("settings.pagination.enabled")}</Label>
        <InfoTooltip text={$_("settings.pagination.description")} />
      </div>
      <Switch id="pagination-enabled" bind:checked={localConfig.pagination.enabled} />
    </div>

    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="fragment-size">{$_("settings.pagination.fragmentSize")}</Label>
        <InfoTooltip text={$_("settings.pagination.fragmentSizeDescription")} />
      </div>
      <Input
        id="fragment-size"
        type="number"
        min={100}
        max={10000}
        step={100}
        bind:value={localConfig.pagination.fragment_size}
      />
      {#if errors.fragment_size}
        <p class="text-destructive text-sm">
          {errors.fragment_size}
        </p>
      {/if}
    </div>
  </div>
</div>
