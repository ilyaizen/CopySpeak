<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import type { AppConfig } from "$lib/types";

  let {
    localConfig = $bindable()
  }: {
    localConfig: AppConfig;
  } = $props();
</script>

<div class="space-y-4">
  <SettingRow label="Start with Windows" tooltip="Launch CopySpeak when Windows starts">
    <Switch id="start-windows" bind:checked={localConfig.general.start_with_windows} />
  </SettingRow>

  <SettingRow label="Start Minimized" tooltip="Start the application minimized to system tray">
    <Switch id="start-minimized" bind:checked={localConfig.general.start_minimized} />
  </SettingRow>

  <SettingRow
    label="Minimize to Tray on Close"
    tooltip="Minimize to system tray instead of exiting when closing the window"
  >
    <Switch
      id="minimize-to-tray"
      checked={localConfig.general.close_behavior === "minimize-to-tray"}
      onchange={(v) => {
        localConfig.general.close_behavior = v ? "minimize-to-tray" : "exit";
      }}
    />
  </SettingRow>
</div>
