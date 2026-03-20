<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable(),
    hudPositionOptions,
    handlePositionChange
  }: {
    localConfig: import("$lib/types").AppConfig;
    hudPositionOptions: Array<{ value: string; label: string }>;
    handlePositionChange: (e: Event) => void;
  } = $props();
</script>

{#if localConfig.hud}
  <div class="border-border bg-card rounded-lg border p-4 shadow-sm">
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <div class="space-y-0.5">
          <Label for="hud-enabled">{$_("settings.hud.enabled")}</Label>
          <p class="text-muted-foreground text-xs">{$_("settings.hud.enabledDescription")}</p>
        </div>
        <Switch id="hud-enabled" bind:checked={localConfig.hud.enabled} />
      </div>

      {#if localConfig.hud.enabled}
        <div class="space-y-2">
          <Label for="hud-position">{$_("settings.hud.position")}</Label>
          <Select
            id="hud-position"
            options={hudPositionOptions}
            value={localConfig.hud.position as string}
            onchange={handlePositionChange}
          />
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="border-border bg-card rounded-lg border p-4 shadow-sm">
    <div class="flex h-32 items-center justify-center">
      <div
        class="border-primary h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
      ></div>
    </div>
  </div>
{/if}
