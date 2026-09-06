<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { listeningStore } from "$lib/stores/listening-store.svelte";
  import type { AppConfig } from "$lib/types";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";

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

  // Toggle clipboard listener on/off — delegates to the store which manages the Tauri backend
  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

<!-- Compact settings panel for quick access to the most-used playback and listening controls -->
<div class="grid min-w-0 grid-cols-1 gap-x-4 sm:grid-cols-2 md:grid-cols-1">
  <!-- Show clipboard listener errors (e.g. permission denied, backend failure) -->
  {#if error}
    <p class="text-destructive text-xs">{error}</p>
  {/if}
  {#if config}
    <!-- Double-copy listener toggle — uses onchange (not bind) because state lives in the store, not config -->
    <div class="border-border flex min-w-0 items-center justify-between gap-2 border-b py-2">
      <div class="flex items-center gap-1">
        <Label for="listen-double-copy" class="text-sm">Double-copy</Label>
        <InfoTooltip text="Monitor clipboard for double-copy" />
      </div>
      <Switch id="listen-double-copy" checked={isListening} onchange={handleToggle} />
    </div>
    <!-- Global hotkey toggle — binds directly to config since it's a persistent preference -->
    <div class="border-border flex min-w-0 items-center justify-between gap-2 border-b py-2">
      <div class="flex items-center gap-1">
        <Label for="qs-hotkey" class="text-sm">Hotkey</Label>
        <InfoTooltip text={config.hotkey.shortcut || "Speak clipboard with a keyboard shortcut"} />
      </div>
      <Switch id="qs-hotkey" bind:checked={config.hotkey.enabled} />
    </div>
    <!-- Effects toggle — binds to active profile's effects -->
    <div class="border-border flex min-w-0 items-center justify-between gap-2 border-b py-2">
      <div class="flex items-center gap-1">
        <Label for="qs-effects" class="text-sm">Effects</Label>
        <InfoTooltip text="Apply audio effect to TTS playback" />
      </div>
      <Switch
        id="qs-effects"
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
    </div>
    <!-- Volume slider — 0–100% range with integer steps for precise control -->
    <div class="flex min-w-0 flex-col gap-2 py-2">
      <div class="flex items-center justify-between">
        <Label for="qs-volume" class="text-sm">Volume</Label>
        <span class="text-muted-foreground text-xs">{config.playback.volume}%</span>
      </div>
      <Slider id="qs-volume" min={0} max={100} step={1} bind:value={config.playback.volume} />
    </div>
    <!-- Speed slider — reads from active profile -->
    <div class="flex min-w-0 flex-col gap-2 py-2">
      <div class="flex items-center justify-between">
        <Label for="qs-speed" class="text-sm">Speed</Label>
        <span class="text-muted-foreground text-xs">{profileSpeed.toFixed(2)}x</span>
      </div>
      <Slider
        id="qs-speed"
        disabled={!activeProfile}
        min={0.5}
        max={2}
        step={0.05}
        value={profileSpeed}
        onchange={(v) => {
          if (activeProfile) activeProfile.speed = v;
        }}
      />
    </div>
    <!-- Pitch slider — reads from active profile -->
    <div class="flex min-w-0 flex-col gap-2 py-2">
      <div class="flex items-center justify-between">
        <Label for="qs-pitch" class="text-sm">Pitch</Label>
        <span class="text-muted-foreground text-xs">{profilePitch.toFixed(2)}x</span>
      </div>
      <Slider
        id="qs-pitch"
        disabled={!activeProfile}
        min={0.5}
        max={2}
        step={0.05}
        value={profilePitch}
        onchange={(v) => {
          if (activeProfile) activeProfile.pitch = v;
        }}
      />
    </div>
  {/if}
</div>
