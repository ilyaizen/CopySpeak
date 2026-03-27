<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
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
    localConfig.hotkey.enabled = false;
  }
</script>

<div class="space-y-4">
  <SettingRow label="Enable Global Hotkey" tooltip="Trigger TTS from any application">
    <Switch id="hotkey-enabled" bind:checked={localConfig.hotkey.enabled} />
  </SettingRow>

  {#if localConfig.hotkey.enabled}
    <SettingRow
      label="Global Hotkey"
      tooltip="Press a key combination to set. Requires at least one modifier (Ctrl, Alt, Shift, Win)."
    >
      <HotkeyCapture
        value={localConfig.hotkey.shortcut}
        disabled={!localConfig.hotkey.enabled}
        onchange={handleShortcutChange}
        onclear={handleShortcutClear}
      />
    </SettingRow>
    {#if errors.hotkey}
      <p class="text-destructive text-sm">
        {errors.hotkey}
      </p>
    {/if}
  {/if}
</div>
