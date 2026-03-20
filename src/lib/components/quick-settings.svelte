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

  // Get reactive references from the store
  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  async function handleToggle() {
    await listeningStore.toggle();
  }
</script>

{#if error}
  <p class="text-destructive mb-4 text-xs">
    {error}
  </p>
{/if}
<div class="grid grid-cols-3 gap-4">
  <div
    class="border-border bg-muted/30 flex items-center justify-between gap-2 rounded-lg border p-3"
  >
    <div class="flex items-center gap-1.5">
      <Label for="listen-double-copy" class="text-sm whitespace-nowrap">Listen</Label>
      <InfoTooltip text="Monitor clipboard for double-copy events" />
    </div>
    <Switch id="listen-double-copy" checked={isListening} onchange={handleToggle} />
  </div>

  {#if config}
    <div class="border-border bg-muted/30 space-y-2 rounded-lg border p-3">
      <div class="flex items-center justify-between">
        <Label for="qs-speed" class="text-sm">Speed</Label>
        <span class="text-muted-foreground text-xs"
          >{config.playback.playback_speed.toFixed(2)}x</span
        >
      </div>
      <Slider
        id="qs-speed"
        min={0.5}
        max={2}
        step={0.01}
        bind:value={config.playback.playback_speed}
      />
    </div>
    <div class="border-border bg-muted/30 space-y-2 rounded-lg border p-3">
      <div class="flex items-center justify-between">
        <Label for="qs-pitch" class="text-sm">Pitch</Label>
        <span class="text-muted-foreground text-xs">{config.playback.pitch.toFixed(2)}x</span>
      </div>
      <Slider id="qs-pitch" min={0.5} max={2} step={0.01} bind:value={config.playback.pitch} />
    </div>
  {/if}
</div>
