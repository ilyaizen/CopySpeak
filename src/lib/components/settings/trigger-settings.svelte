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
  <h3 class="text-card-foreground mb-4 text-lg font-medium">{$_("settings.triggers.title")}</h3>
  <div class="space-y-4">
    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="double-copy-window">{$_("settings.triggers.doubleCopyWindow")}</Label>
        <InfoTooltip text={$_("settings.triggers.doubleCopyWindowDescription")} />
      </div>
      <Input
        id="double-copy-window"
        type="number"
        min={100}
        max={5000}
        step={50}
        bind:value={localConfig.trigger.double_copy_window_ms}
      />
      {#if errors.double_copy_window_ms}
        <p class="text-destructive text-sm">
          {errors.double_copy_window_ms}
        </p>
      {/if}
    </div>

    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="max-text-length">{$_("settings.triggers.maxTextLength")}</Label>
        <InfoTooltip text={$_("settings.triggers.maxTextLengthDescription")} />
      </div>
      <Input
        id="max-text-length"
        type="number"
        min={100}
        max={1000000}
        step={1000}
        bind:value={localConfig.trigger.max_text_length}
      />
      {#if errors.max_text_length}
        <p class="text-destructive text-sm">
          {errors.max_text_length}
        </p>
      {/if}
    </div>

    <div class="border-border border-t pt-4">
      <h4 class="text-card-foreground mb-4 text-sm font-medium">
        {$_("settings.triggers.paginationTitle")}
      </h4>
      <div class="space-y-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-1.5">
            <Label for="pagination-enabled">{$_("settings.triggers.paginationEnabled")}</Label>
            <InfoTooltip text={$_("settings.triggers.paginationDescription")} />
          </div>
          <Switch id="pagination-enabled" bind:checked={localConfig.pagination.enabled} />
        </div>

        <div class="space-y-2">
          <div class="flex items-center gap-1.5">
            <Label for="fragment-size">{$_("settings.triggers.fragmentSize")}</Label>
            <InfoTooltip text={$_("settings.triggers.fragmentSizeDescription")} />
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
  </div>
</div>
