<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
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

<div class="space-y-4">
  <SettingRow
    label="Enable Global Hotkey"
    tooltip="When enabled, the hotkey will work from any application to read clipboard contents aloud."
  >
    <Switch id="hotkey-enabled" bind:checked={localConfig.hotkey.enabled} />
  </SettingRow>

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
