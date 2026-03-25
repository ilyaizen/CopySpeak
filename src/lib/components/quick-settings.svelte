<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { listeningStore } from "$lib/stores/listening-store.svelte";
  import type { AppConfig } from "$lib/types";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { Snippet } from "svelte";

  interface Props {
    config?: AppConfig;
    children?: Snippet;
  }

  let { config = $bindable(), children }: Props = $props();

  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

{#if error}
  <p class="text-destructive mb-2 text-xs">{error}</p>
{/if}

<div class="flex items-stretch gap-3">
  <div class="w-2/3">
    {#if children}
      {@render children()}
    {/if}
  </div>

  <div class="w-1/3 space-y-2">
    <div
      class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border p-2"
    >
      <div class="flex items-center gap-1">
        <Label for="listen-double-copy" class="text-xs">Listen</Label>
        <InfoTooltip text="Monitor clipboard for double-copy" />
      </div>
      <Switch id="listen-double-copy" checked={isListening} onchange={handleToggle} />
    </div>

    {#if config}
      <div
        class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border p-2"
      >
        <div class="flex items-center gap-1">
          <Label for="qs-hotkey" class="text-xs">Hotkey</Label>
          <InfoTooltip text="Global hotkey to speak clipboard" />
        </div>
        <Switch id="qs-hotkey" bind:checked={config.hotkey.enabled} />
      </div>

      <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
        <div class="flex items-center justify-between">
          <Label for="qs-volume" class="text-xs">Vol</Label>
          <span class="text-muted-foreground text-xs"
            >{Math.round(config.playback.volume * 100)}%</span
          >
        </div>
        <Slider id="qs-volume" min={0} max={1} step={0.01} bind:value={config.playback.volume} />
      </div>

      <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
        <div class="flex items-center justify-between">
          <Label for="qs-speed" class="text-xs">Speed</Label>
          <span class="text-muted-foreground text-xs"
            >{config.playback.playback_speed.toFixed(1)}x</span
          >
        </div>
        <Slider
          id="qs-speed"
          min={0.5}
          max={2}
          step={0.1}
          bind:value={config.playback.playback_speed}
        />
      </div>

      <div class="border-border bg-muted/30 space-y-1 rounded-lg border p-2">
        <div class="flex items-center justify-between">
          <Label for="qs-pitch" class="text-xs">Pitch</Label>
          <span class="text-muted-foreground text-xs">{config.playback.pitch.toFixed(1)}x</span>
        </div>
        <Slider id="qs-pitch" min={0.5} max={2} step={0.1} bind:value={config.playback.pitch} />
      </div>
    {/if}
  </div>
</div>
