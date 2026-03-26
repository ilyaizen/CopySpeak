<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: import("$lib/types").AppConfig } = $props();

  const HUD_OPTIONS = [
    { value: "disabled", label: "Disabled" },
    { value: "top-left", label: $_("settings.hud.topLeft") },
    { value: "top-center", label: $_("settings.hud.topCenter") },
    { value: "top-right", label: $_("settings.hud.topRight") },
    { value: "bottom-left", label: $_("settings.hud.bottomLeft") },
    { value: "bottom-center", label: $_("settings.hud.bottomCenter") },
    { value: "bottom-right", label: $_("settings.hud.bottomRight") }
  ];

  let hudValue = $derived(
    localConfig.hud ? (localConfig.hud.enabled ? localConfig.hud.position : "disabled") : "disabled"
  );

  function handleHudChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    const value = target.value;
    if (!localConfig.hud) return;

    if (value === "disabled") {
      localConfig.hud.enabled = false;
    } else {
      localConfig.hud.enabled = true;
      localConfig.hud.position = value as import("$lib/types").HudPosition;
    }
  }
</script>

{#if localConfig.hud}
  <SettingRow label={$_("settings.hud.position")} tooltip={$_("settings.hud.enabledDescription")}>
    <Select options={HUD_OPTIONS} value={hudValue} onchange={handleHudChange} class="w-40" />
  </SettingRow>
{:else}
  <div class="flex h-12 items-center justify-center">
    <div
      class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
    ></div>
  </div>
{/if}
