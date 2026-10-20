<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import { Slider } from "#lib/components/ui/slider/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import type { AppConfig } from "#lib/types";
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
    label={$_("settings.playback.streaming")}
    tooltip={$_("settings.playback.streamingDescription")}
  >
    <Switch id="playback-streaming" bind:checked={localConfig.playback.streaming_enabled} />
  </SettingRow>

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

  <div>
    <SettingRow
      label={$_("settings.playback.duck")}
      tooltip={$_("settings.playback.duckDescription")}
    >
      <Switch
        id="playback-duck"
        aria-label={$_("settings.playback.duck")}
        bind:checked={localConfig.playback.duck.enabled}
      />
    </SettingRow>
  </div>

  {#if localConfig.playback.duck.enabled}
    <SettingRow
      label={$_("settings.playback.duckLevel")}
      tooltip={$_("settings.playback.duckLevelDescription")}
    >
      <div class="flex items-center gap-3">
        <span class="text-muted-foreground w-10 text-end text-sm tabular-nums"
          >{localConfig.playback.duck.level_percent}%</span
        >
        <Slider
          id="duck-level"
          min={0}
          max={100}
          step={5}
          bind:value={localConfig.playback.duck.level_percent}
          class="w-32"
        />
      </div>
    </SettingRow>
  {/if}
</div>
