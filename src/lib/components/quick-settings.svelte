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

  let { config = $bindable() }: Props = $props();

  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

<div class="border-border bg-card flex flex-col gap-2 rounded-lg border p-3 shadow-sm">
  {#if error}
    <p class="text-destructive text-xs">{error}</p>
  {/if}
  {#if config}
    <div
      class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border p-2"
    >
      <div class="flex items-center gap-1">
        <Label for="listen-double-copy" class="text-xs">Listen</Label>
        <InfoTooltip text="Monitor clipboard for double-copy" />
      </div>
      <Switch id="listen-double-copy" checked={isListening} onchange={handleToggle} />
    </div>
    <div
      class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border p-2"
    >
      <div class="flex items-center gap-1">
        <Label for="qs-hotkey" class="text-xs">Hotkey</Label>
        <InfoTooltip text="Global hotkey to speak clipboard (default: Win+Shift+A)" />
      </div>
      <Switch id="qs-hotkey" bind:checked={config.hotkey.enabled} />
    </div>
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
      <div class="flex items-center justify-between">
        <Label for="qs-volume" class="text-xs">Volume</Label>
        <span class="text-muted-foreground text-xs">{config.playback.volume}%</span>
      </div>
      <Slider id="qs-volume" min={0} max={100} step={1} bind:value={config.playback.volume} />
    </div>
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
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
    <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
      <div class="flex items-center justify-between">
        <Label for="qs-pitch" class="text-xs">Pitch</Label>
        <span class="text-muted-foreground text-xs">{config.playback.pitch}x</span>
      </div>
      <Slider id="qs-pitch" min={0.5} max={2} step={0.05} bind:value={config.playback.pitch} />
    </div>
  {/if}
</div>
