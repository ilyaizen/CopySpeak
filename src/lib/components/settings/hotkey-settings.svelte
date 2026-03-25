<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import HotkeyCapture from "$lib/components/hotkey-capture.svelte";
  import type { AppConfig } from "$lib/types";

  let {
    localConfig = $bindable(),
    errors
  }: {
    localConfig: AppConfig;
    errors: Record<string, string>;
  } = $props();

  function handleShortcutChange(value: string) {
    localConfig.hotkey.shortcut = value;
  }

  function handleShortcutClear() {
    localConfig.hotkey.shortcut = "Win+Shift+A";
  }
</script>

<div class="border-border bg-card rounded-lg border p-4 shadow-sm">
  <h3 class="text-card-foreground mb-4 text-lg font-medium">Global Hotkey</h3>
  <p class="text-muted-foreground mb-4 text-sm">
    Configure a global keyboard shortcut to trigger speech from clipboard content. This provides an
    alternative to the double-copy trigger.
  </p>

  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1.5">
        <Label for="hotkey-enabled">Enable Global Hotkey</Label>
        <InfoTooltip
          text="When enabled, the hotkey will work from any application to read clipboard contents aloud."
        />
      </div>
      <Switch id="hotkey-enabled" bind:checked={localConfig.hotkey.enabled} />
    </div>

    {#if localConfig.hotkey.enabled}
      <div class="space-y-2">
        <HotkeyCapture
          value={localConfig.hotkey.shortcut}
          disabled={!localConfig.hotkey.enabled}
          onchange={handleShortcutChange}
          onclear={handleShortcutClear}
        />
        {#if errors.hotkey}
          <p class="text-destructive text-sm">
            {errors.hotkey}
          </p>
        {/if}
        <p class="text-muted-foreground text-xs">
          Press the key combination you want to use. Must include at least one modifier (Ctrl, Alt
          Shift, or Win).
        </p>
        <p class="text-muted-foreground text-xs">
          <strong>Default:</strong> Win+Shift+A reads clipboard contents aloud.
        </p>
      </div>
    {/if}
  </div>
</div>
