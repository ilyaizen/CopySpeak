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

  // Toggle clipboard listener on/off — delegates to the store which manages the Tauri backend
  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

<!-- Compact settings panel for quick access to the most-used playback and listening controls -->
<div class="border-border bg-card flex h-full flex-col gap-2 rounded-lg border p-3 shadow-sm">
  <!-- Show clipboard listener errors (e.g. permission denied, backend failure) -->
  {#if error}
    <p class="text-destructive text-xs">{error}</p>
  {/if}
  {#if config}
    <!-- Double-copy listener toggle — uses onchange (not bind) because state lives in the store, not config -->
    <div
      class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border px-3 py-4"
    >
      <div class="flex items-center gap-1">
        <Label for="listen-double-copy" class="text-xs">Listen</Label>
        <InfoTooltip text="Monitor clipboard for double-copy" />
      </div>
      <Switch id="listen-double-copy" checked={isListening} onchange={handleToggle} />
    </div>
    <!-- Global hotkey toggle — binds directly to config since it's a persistent preference -->
    <div
      class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border px-3 py-4"
    >
      <div class="flex items-center gap-1">
        <Label for="qs-hotkey" class="text-xs">Hotkey</Label>
        <InfoTooltip text="Global hotkey to speak clipboard (default: Win+Shift+A)" />
      </div>
      <Switch id="qs-hotkey" bind:checked={config.hotkey.enabled} />
    </div>
    <!-- Volume slider — 0–100% range with integer steps for precise control -->
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border px-3 py-4">
      <div class="flex items-center justify-between">
        <Label for="qs-volume" class="text-xs">Volume</Label>
        <span class="text-muted-foreground text-xs">{config.playback.volume}%</span>
      </div>
      <Slider id="qs-volume" min={0} max={100} step={1} bind:value={config.playback.volume} />
    </div>
    <!-- Playback speed slider — 0.5x–2x range for slow reading to fast skimming -->
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border px-3 py-4">
      <div class="flex items-center justify-between">
        <Label for="qs-speed" class="text-xs">Speed</Label>
        <span class="text-muted-foreground text-xs">{config.playback.playback_speed}x</span>
      </div>
      <Slider
        id="qs-speed"
        min={0.5}
        max={2}
        step={0.05}
        bind:value={config.playback.playback_speed}
      />
    </div>
    <!-- Pitch slider — 0.5x–2x range to adjust voice frequency -->
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border px-3 py-4">
      <div class="flex items-center justify-between">
        <Label for="qs-pitch" class="text-xs">Pitch</Label>
        <span class="text-muted-foreground text-xs">{config.playback.pitch}x</span>
      </div>
      <Slider id="qs-pitch" min={0.5} max={2} step={0.05} bind:value={config.playback.pitch} />
    </div>
  {/if}
</div>
