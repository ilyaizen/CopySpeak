<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { Slider } from "#lib/components/ui/slider/index.js";
  import type { AppConfig } from "#lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();
  const supportsDuck = navigator.userAgent.includes("Windows");
</script>

<div class="divide-border divide-y">
  <div class="py-4">
    <h3 class="text-muted-foreground mb-3 text-sm font-medium">
      {$_("settings.sections.audioDelivery")}
    </h3>
    <SettingRow
      label={$_("settings.playback.streaming")}
      tooltip={$_("settings.playback.streamingDescription")}
    >
      <Switch
        id="playback-streaming"
        aria-label={$_("settings.playback.streaming")}
        bind:checked={localConfig.playback.streaming_enabled}
      />
    </SettingRow>
  </div>
  <div class="pt-4">
    <SettingRow
      label={$_("settings.general.experimental")}
      tooltip={$_("settings.general.experimentalDescription")}
    >
      <Switch
        id="experimental-enabled"
        aria-label={$_("settings.general.experimental")}
        bind:checked={localConfig.general.experimental_enabled}
      />
    </SettingRow>
    <p class="text-muted-foreground mt-2 text-sm leading-relaxed">
      {$_("settings.general.experimentalDescription")}
    </p>
    {#if localConfig.general.experimental_enabled === true}
      <div class="border-border mt-4 space-y-4 border-s ps-4">
        <SettingRow
          label={$_("settings.history.enabled")}
          tooltip={$_("settings.history.enabledDescription")}
        >
          <Switch
            id="history-enabled"
            aria-label={$_("settings.history.enabled")}
            bind:checked={localConfig.history.enabled}
          />
        </SettingRow>
        <SettingRow
          label={$_("settings.postProcess.enabled")}
          tooltip={$_("settings.postProcess.enabledDescription")}
        >
          <Switch
            id="post-process-enabled"
            aria-label={$_("settings.postProcess.enabled")}
            bind:checked={localConfig.post_process.enabled}
          />
        </SettingRow>
        {#if supportsDuck}
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
                  aria-label={$_("settings.playback.duckLevel")}
                  min={0}
                  max={100}
                  step={5}
                  bind:value={localConfig.playback.duck.level_percent}
                  class="w-32"
                />
              </div>
            </SettingRow>
          {/if}
        {/if}
        <p class="text-muted-foreground text-sm leading-relaxed">
          {$_("settings.general.profileEffects")}
        </p>
      </div>
    {/if}
  </div>
</div>
