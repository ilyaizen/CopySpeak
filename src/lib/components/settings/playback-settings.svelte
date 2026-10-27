<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import { Slider } from "#lib/components/ui/slider/index.js";
  import type { AppConfig, RetriggerMode } from "#lib/types";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable(),
    retriggerModeOptions
  }: {
    localConfig: AppConfig;
    retriggerModeOptions: { value: RetriggerMode; label: string }[];
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
        // SAFETY: handler is bound to this select; validate the wire value before saving.
        const value = (e.target as HTMLSelectElement).value;
        if (value === "interrupt" || value === "queue") localConfig.playback.on_retrigger = value;
      }}
      class="w-48"
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.playback.volume")}
    tooltip={$_("settings.playback.volumeDescription")}
  >
    <div class="flex items-center gap-3">
      <span class="text-muted-foreground w-10 text-end text-sm tabular-nums"
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
