<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { Slider } from "#lib/components/ui/slider/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import type { AppConfig, HudPosition } from "#lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();
  const supportsDuck = navigator.userAgent.includes("Windows");

  const HUD_OPTIONS = [
    { value: "disabled", label: "Disabled" },
    { value: "top-left", label: $_("settings.hud.topLeft") },
    { value: "top-center", label: $_("settings.hud.topCenter") },
    { value: "top-right", label: $_("settings.hud.topRight") },
    { value: "bottom-left", label: $_("settings.hud.bottomLeft") },
    { value: "bottom-center", label: $_("settings.hud.bottomCenter") },
    { value: "bottom-right", label: $_("settings.hud.bottomRight") }
  ];

  let hudValue = $derived(localConfig.hud.enabled ? localConfig.hud.position : "disabled");

  function handleHudChange(e: Event) {
    // SAFETY: this handler is only bound to the HUD <select>, whose target is that element.
    const target = e.target as HTMLSelectElement;
    const value = target.value;

    if (value === "disabled") {
      localConfig.hud.enabled = false;
    } else {
      localConfig.hud.enabled = true;
      // SAFETY: the only non-disabled option values are the HudPosition ids.
      localConfig.hud.position = value as HudPosition;
    }
  }
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
        <SettingRow
          label={$_("settings.hud.overlay")}
          tooltip={$_("settings.hud.enabledDescription")}
        >
          <Switch
            id="hud-enabled"
            aria-label={$_("settings.hud.overlay")}
            bind:checked={localConfig.hud.enabled}
          />
        </SettingRow>
        {#if localConfig.hud.enabled}
          <SettingRow
            label={$_("settings.hud.position")}
            tooltip={$_("settings.hud.positionDescription")}
          >
            <Select
              id="hud-position"
              options={HUD_OPTIONS}
              value={hudValue}
              onchange={handleHudChange}
              class="w-40"
            />
          </SettingRow>
        {/if}
        <p class="text-muted-foreground text-sm leading-relaxed">
          {$_("settings.general.profileEffects")}
        </p>
      </div>
    {/if}
  </div>
</div>
