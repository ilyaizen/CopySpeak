<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { Slider } from "#lib/components/ui/slider/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { listeningStore } from "#lib/stores/listening-store.svelte";
  import { invoke } from "#lib/services/tauri";
  import type { AppConfig, VoiceProfile } from "#lib/types";

  interface Props {
    config?: AppConfig;
  }

  // Bindable so parent can pass config down and receive updates via two-way binding
  let { config = $bindable() }: Props = $props();

  // Derive listening state from the global store (not from config, since it's runtime state)
  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  // Derive speed/pitch/effects from the active profile (sole source of truth)
  const activeProfile = $derived(
    config?.tts.profiles.find((p) => p.id === config.tts.active_profile_id) ?? null
  );
  const profileSpeed = $derived(activeProfile?.speed ?? 1.0);
  const profilePitch = $derived(activeProfile?.pitch ?? 1.0);
  const profileEffectsEnabled = $derived(activeProfile?.effects.enabled ?? false);

  // Dramatize needs emote-tag support for the active profile's (engine, model) pair.
  // The backend catalog owns that answer; the model key mirrors `effective_model`.
  function profileModel(profile: VoiceProfile | null): string {
    if (!profile) return "";
    const key =
      profile.engine === "elevenlabs" ? "model_id" : profile.engine === "fish" ? "model" : null;
    return key ? String(profile.engine_options[key] ?? "") : "";
  }

  let dramatizeSupported = $state(false);
  $effect(() => {
    const engine = activeProfile?.engine;
    const model = profileModel(activeProfile);
    if (!engine) {
      dramatizeSupported = false;
      return;
    }
    let stale = false;
    invoke<boolean>("supports_emote_tags", { engine, model })
      .then((ok) => {
        if (!stale) dramatizeSupported = ok;
      })
      .catch(() => {
        if (!stale) dramatizeSupported = false;
      });
    return () => {
      stale = true;
    };
  });

  // Live drag values: shown while a slider is dragged, cleared on commit so the
  // profile becomes the source of truth again (also handles profile switching).
  let speedLive = $state<number | null>(null);
  let pitchLive = $state<number | null>(null);

  // Toggle clipboard listener on/off — delegates to the store which manages the Tauri backend
  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

<!-- Compact settings panel for quick access to the most-used playback and listening controls -->
<div
  class="bg-card border-border divide-border flex min-w-0 flex-col divide-y rounded-lg border p-3 shadow-sm"
>
  <!-- Show clipboard listener errors (e.g. permission denied, backend failure) -->
  {#if error}
    <p class="text-destructive text-xs">{error}</p>
  {/if}
  {#if config}
    <!-- Double-copy listener toggle — uses onchange (not bind) because state lives in the store, not config -->
    <SettingRow label={$_("quick.doubleCopy")} tooltip={$_("quick.doubleCopyTooltip")}>
      <Switch
        id="listen-double-copy"
        aria-label={$_("quick.doubleCopy")}
        checked={isListening}
        onchange={handleToggle}
      />
    </SettingRow>
    <!-- Global hotkey toggle — binds directly to config since it's a persistent preference -->
    <SettingRow
      label={$_("common.hotkey")}
      tooltip={config.hotkey.shortcut || "Speak clipboard with a keyboard shortcut"}
    >
      <Switch
        id="qs-hotkey"
        aria-label={$_("common.hotkey")}
        bind:checked={config.hotkey.enabled}
      />
    </SettingRow>
    {#if config.general.experimental_enabled === true}
      <!-- Effects toggle — binds to active profile's effects -->
      <SettingRow label={$_("common.effects")} tooltip={$_("quick.effectsTooltip")}>
        <Switch
          id="qs-effects"
          aria-label={$_("common.effects")}
          disabled={!activeProfile}
          checked={profileEffectsEnabled}
          onchange={() => {
            if (activeProfile) {
              activeProfile.effects.enabled = !activeProfile.effects.enabled;
              if (!activeProfile.effects.enabled) {
                activeProfile.effects.active_effect = "none";
              }
            }
          }}
        />
      </SettingRow>
      <!-- Dramatize: the post-process switch; disabled when the profile's model cannot read emote tags -->
      <SettingRow
        label={$_("quick.dramatize")}
        tooltip={dramatizeSupported
          ? $_("quick.dramatizeTooltip")
          : $_("quick.dramatizeUnsupported")}
      >
        <Switch
          id="qs-dramatize"
          aria-label={$_("quick.dramatize")}
          disabled={!dramatizeSupported}
          bind:checked={config.post_process.enabled}
        />
      </SettingRow>
    {/if}
    <!-- Volume slider — 0–100% range with integer steps for precise control -->
    <SettingRow label={$_("common.volume")}>
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-volume"
          aria-label={$_("common.volume")}
          min={0}
          max={100}
          step={1}
          bind:value={config.playback.volume}
        />
        <span class="text-muted-foreground w-9 text-end text-xs tabular-nums"
          >{config.playback.volume}%</span
        >
      </div>
    </SettingRow>
    <!-- Speed slider — reads from active profile -->
    <SettingRow label={$_("common.speed")}>
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-speed"
          aria-label={$_("common.speed")}
          disabled={!activeProfile}
          min={0.5}
          max={1.5}
          step={0.05}
          value={profileSpeed}
          oninput={(v) => (speedLive = v)}
          onchange={(v) => {
            if (activeProfile) activeProfile.speed = v;
            speedLive = null;
          }}
        />
        <span class="text-muted-foreground w-9 text-end text-xs tabular-nums"
          >{(speedLive ?? profileSpeed).toFixed(2)}x</span
        >
      </div>
    </SettingRow>
    <!-- Pitch slider — reads from active profile -->
    <SettingRow label={$_("common.pitch")}>
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-pitch"
          aria-label={$_("common.pitch")}
          disabled={!activeProfile}
          min={0.75}
          max={1.25}
          step={0.01}
          value={profilePitch}
          oninput={(v) => (pitchLive = v)}
          onchange={(v) => {
            if (activeProfile) activeProfile.pitch = v;
            pitchLive = null;
          }}
        />
        <span class="text-muted-foreground w-9 text-end text-xs tabular-nums"
          >{(pitchLive ?? profilePitch).toFixed(2)}x</span
        >
      </div>
    </SettingRow>
  {/if}
</div>
