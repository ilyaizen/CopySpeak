<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
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

<div class="space-y-4">
  <SettingRow
    label={$_("settings.triggers.listen")}
    tooltip={$_("settings.triggers.listenDescription")}
  >
    <Switch id="listen-double-copy" bind:checked={localConfig.trigger.listen_enabled} />
  </SettingRow>

  <SettingRow
    label={$_("settings.triggers.doubleCopyWindow")}
    tooltip={$_("settings.triggers.doubleCopyWindowDescription")}
  >
    <div class="space-y-1">
      <Input
        id="double-copy-window"
        type="number"
        min={100}
        max={5000}
        step={50}
        bind:value={localConfig.trigger.double_copy_window_ms}
        class="w-32"
      />
      {#if errors.double_copy_window_ms}
        <p class="text-destructive text-xs">{errors.double_copy_window_ms}</p>
      {/if}
    </div>
  </SettingRow>

  <SettingRow
    label={$_("settings.triggers.maxTextLength")}
    tooltip={$_("settings.triggers.maxTextLengthDescription")}
  >
    <div class="space-y-1">
      <Input
        id="max-text-length"
        type="number"
        min={100}
        max={1000000}
        step={1000}
        bind:value={localConfig.trigger.max_text_length}
        class="w-32"
      />
      {#if errors.max_text_length}
        <p class="text-destructive text-xs">{errors.max_text_length}</p>
      {/if}
    </div>
  </SettingRow>
</div>
