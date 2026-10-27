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
    <Switch
      id="launch-on-login"
      aria-label={$_("settings.general.launchOnLogin")}
      bind:checked={localConfig.general.launch_on_login}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.showTrayIcon")}
    tooltip={$_("settings.general.showTrayIconTooltip")}
  >
    <Switch
      id="show-tray-icon"
      aria-label={$_("settings.general.showTrayIcon")}
      bind:checked={localConfig.general.show_tray_icon}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.startMinimizedLabel")}
    tooltip={$_("settings.general.startMinimizedTooltip")}
  >
    <Switch
      id="start-minimized"
      aria-label={$_("settings.general.startMinimizedLabel")}
      disabled={!localConfig.general.show_tray_icon}
      bind:checked={localConfig.general.start_minimized}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.general.minimizeToTray")}
    tooltip={$_("settings.general.minimizeToTrayTooltip")}
  >
    <Switch
      id="minimize-to-tray"
      aria-label={$_("settings.general.minimizeToTray")}
      disabled={!localConfig.general.show_tray_icon}
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
