<script lang="ts">
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { listeningStore } from "$lib/stores/listening-store.svelte";
  import type { AppConfig } from "$lib/types";

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
<div class="border-border divide-border grid min-w-0 grid-cols-1 divide-y border-y">
  <!-- Show clipboard listener errors (e.g. permission denied, backend failure) -->
  {#if error}
    <p class="text-destructive text-xs">{error}</p>
  {/if}
  {#if config}
    <!-- Double-copy listener toggle — uses onchange (not bind) because state lives in the store, not config -->
    <SettingRow label="Double-copy" tooltip="Monitor clipboard for double-copy">
      <Switch
        id="listen-double-copy"
        aria-label="Double-copy"
        checked={isListening}
        onchange={handleToggle}
      />
    </SettingRow>
    <!-- Global hotkey toggle — binds directly to config since it's a persistent preference -->
    <SettingRow
      label="Hotkey"
      tooltip={config.hotkey.shortcut || "Speak clipboard with a keyboard shortcut"}
    >
      <Switch id="qs-hotkey" aria-label="Hotkey" bind:checked={config.hotkey.enabled} />
    </SettingRow>
    <!-- Effects toggle — binds to active profile's effects -->
    <SettingRow label="Effects" tooltip="Apply audio effect to TTS playback">
      <Switch
        id="qs-effects"
        aria-label="Effects"
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
    <!-- Volume slider — 0–100% range with integer steps for precise control -->
    <SettingRow label="Volume">
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-volume"
          aria-label="Volume"
          min={0}
          max={100}
          step={1}
          bind:value={config.playback.volume}
        />
        <span class="text-muted-foreground w-9 text-right text-xs tabular-nums"
          >{config.playback.volume}%</span
        >
      </div>
    </SettingRow>
    <!-- Speed slider — reads from active profile -->
    <SettingRow label="Speed">
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-speed"
          aria-label="Speed"
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
        <span class="text-muted-foreground w-9 text-right text-xs tabular-nums"
          >{(speedLive ?? profileSpeed).toFixed(2)}x</span
        >
      </div>
    </SettingRow>
    <!-- Pitch slider — reads from active profile -->
    <SettingRow label="Pitch">
      <div class="flex w-28 items-center gap-2">
        <Slider
          id="qs-pitch"
          aria-label="Pitch"
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
        <span class="text-muted-foreground w-9 text-right text-xs tabular-nums"
          >{(pitchLive ?? profilePitch).toFixed(2)}x</span
        >
      </div>
    </SettingRow>
  {/if}
</div>
