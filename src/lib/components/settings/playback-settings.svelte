<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
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

  <SettingRow
    label={$_("settings.playback.playbackSpeed")}
    tooltip={$_("settings.playback.playbackSpeedDescription")}
  >
    <div class="flex items-center gap-3">
      <span class="text-muted-foreground w-14 text-right text-sm tabular-nums">
        {localConfig.playback.playback_speed.toFixed(2)}x
      </span>
      <Slider
        id="playback-speed"
        min={0.5}
        max={2}
        step={0.05}
        bind:value={localConfig.playback.playback_speed}
        class="w-32"
      />
    </div>
  </SettingRow>

  <SettingRow
    label={$_("settings.playback.pitch")}
    tooltip={$_("settings.playback.pitchDescription")}
  >
    <div class="flex items-center gap-3">
      <span class="text-muted-foreground w-14 text-right text-sm tabular-nums">
        {localConfig.playback.pitch.toFixed(2)}x
      </span>
      <Slider
        id="playback-pitch"
        min={0.5}
        max={2}
        step={0.05}
        bind:value={localConfig.playback.pitch}
        class="w-32"
      />
    </div>
  </SettingRow>
</div>
