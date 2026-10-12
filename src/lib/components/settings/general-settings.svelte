<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "#lib/types";

  let {
    localConfig = $bindable()
  }: {
    localConfig: AppConfig;
  } = $props();
</script>

<div class="space-y-4">
  <SettingRow
    label={$_("settings.general.launchOnLogin")}
    tooltip={$_("settings.general.launchOnLoginTooltip")}
  >
    <Switch id="launch-on-login" bind:checked={localConfig.general.launch_on_login} />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.startMinimizedLabel")}
    tooltip={$_("settings.general.startMinimizedTooltip")}
  >
    <Switch id="start-minimized" bind:checked={localConfig.general.start_minimized} />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.minimizeToTray")}
    tooltip={$_("settings.general.minimizeToTrayTooltip")}
  >
    <Switch
      id="minimize-to-tray"
      checked={localConfig.general.close_behavior === "minimize-to-tray"}
      onchange={(v) => {
        localConfig.general.close_behavior = v ? "minimize-to-tray" : "exit";
      }}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.autoSave")}
    tooltip={$_("settings.general.autoSaveDescription")}
  >
    <Switch id="auto-save-profiles" bind:checked={localConfig.general.auto_save_profiles} />
  </SettingRow>
</div>
