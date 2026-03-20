<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
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

<div class="border-border bg-card rounded-lg border p-4 shadow-sm">
  <h3 class="text-card-foreground mb-4 text-lg font-medium">{$_("settings.playback.title")}</h3>
  <div class="space-y-4">
    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="retrigger-mode">{$_("settings.playback.onRetrigger")}</Label>
        <InfoTooltip text={$_("settings.playback.onRetriggerDescription")} />
      </div>
      <Select
        id="retrigger-mode"
        options={retriggerModeOptions}
        value={localConfig.playback.on_retrigger}
        onchange={(e) => {
          localConfig.playback.on_retrigger = (e.target as HTMLSelectElement).value as any;
        }}
      />
    </div>

    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="playback-volume"
          >{$_("settings.playback.volume")}: {localConfig.playback.volume}%</Label
        >
        <InfoTooltip text={$_("settings.playback.volumeDescription")} />
      </div>
      <Slider
        id="playback-volume"
        min={0}
        max={100}
        step={1}
        bind:value={localConfig.playback.volume}
      />
    </div>

    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="playback-speed"
          >{$_("settings.playback.playbackSpeed")}: {localConfig.playback.playback_speed.toFixed(
            2
          )}x</Label
        >
        <InfoTooltip text={$_("settings.playback.playbackSpeedDescription")} />
      </div>
      <Slider
        id="playback-speed"
        min={0.5}
        max={2}
        step={0.05}
        bind:value={localConfig.playback.playback_speed}
      />
    </div>

    <div class="space-y-2">
      <div class="flex items-center gap-1.5">
        <Label for="playback-pitch"
          >{$_("settings.playback.pitch")}: {localConfig.playback.pitch.toFixed(2)}x</Label
        >
        <InfoTooltip text={$_("settings.playback.pitchDescription")} />
      </div>
      <Slider
        id="playback-pitch"
        min={0.5}
        max={2}
        step={0.05}
        bind:value={localConfig.playback.pitch}
      />
    </div>
  </div>
</div>
