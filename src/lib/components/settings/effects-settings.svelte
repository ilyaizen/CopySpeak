<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "$lib/types";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  const effectOptions = [
    { value: "none", label: $_("settings.effects.options.none") },
    { value: "walkie_talkie", label: $_("settings.effects.options.walkieTalkie") },
    { value: "game_boy", label: $_("settings.effects.options.gameBoy") }
  ];
</script>

<div class="space-y-4">
  <SettingRow
    label={$_("settings.effects.enabled")}
    tooltip={$_("settings.effects.enabledDescription")}
  >
    <Switch id="effects-enabled" bind:checked={localConfig.effects.enabled} />
  </SettingRow>

  {#if localConfig.effects.enabled}
    <SettingRow
      label={$_("settings.effects.active")}
      tooltip={$_("settings.effects.activeDescription")}
    >
      <Select
        options={effectOptions}
        bind:value={localConfig.effects.active_effect}
        class="w-44"
      />
    </SettingRow>
  {/if}
</div>
