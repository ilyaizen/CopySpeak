<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable(),
    retriggerModeOptions
  }: {
    localConfig: AppConfig;
    retriggerModeOptions: { value: string; label: string }[];
  } = $props();
</script>

<div class="space-y-4">
  <div>
    <SettingRow label={$_("settings.playback.streaming")}>
      <Switch
        id="playback-streaming"
        aria-label={$_("settings.playback.streaming")}
        aria-describedby="playback-streaming-description"
        bind:checked={localConfig.playback.streaming_enabled}
      />
    </SettingRow>
    <p id="playback-streaming-description" class="text-muted-foreground text-sm">
      {$_("settings.playback.streamingDescription")}
    </p>
  </div>

  <SettingRow
    label={$_("settings.playback.onRetrigger")}
    tooltip={$_("settings.playback.onRetriggerDescription")}
  >
    <Select
      id="retrigger-mode"
      options={retriggerModeOptions}
      value={localConfig.playback.on_retrigger}
      onchange={(e) => {
        localConfig.playback.on_retrigger = (e.target as HTMLSelectElement).value as any;
      }}
      class="w-48"
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.playback.volume")}
    tooltip={$_("settings.playback.volumeDescription")}
  >
    <div class="flex items-center gap-3">
      <span class="text-muted-foreground w-10 text-right text-sm tabular-nums"
        >{localConfig.playback.volume}%</span
      >
      <Slider
        id="playback-volume"
        min={0}
        max={100}
        step={1}
        bind:value={localConfig.playback.volume}
        class="w-32"
      />
    </div>
  </SettingRow>
</div>
