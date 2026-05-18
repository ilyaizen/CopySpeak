<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/services/tauri";
  import { isTauri } from "$lib/services/tauri";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { Wand2 } from "@lucide/svelte";
  import type { AppConfig, EffectId } from "$lib/types";

  const PREVIEW_TEXT =
    "This is a quick preview of the selected audio effect. How does it sound to you?";

  let config = $state<AppConfig | null>(null);
  let activeEffect = $state<EffectId>("none");
  let isPreviewing = $state(false);

  const effectOptions = [
    { value: "none", label: $_("settings.effects.options.none") },
    { value: "walkie_talkie", label: $_("settings.effects.options.walkieTalkie") },
    { value: "game_boy", label: $_("settings.effects.options.gameBoy") }
  ];

  onMount(async () => {
    if (!isTauri) return;
    try {
      config = await invoke<AppConfig>("get_config");
      activeEffect = config.effects?.active_effect ?? "none";
    } catch (e) {
      toast.error(`Failed to load effects: ${e}`);
    }
  });

  async function handleEffectChange(value: string) {
    if (!config) return;
    const next = value as EffectId;
    activeEffect = next;
    config.effects.active_effect = next;
    try {
      await invoke("set_config", { newConfig: config });
    } catch (e) {
      toast.error(`Failed to save effect: ${e}`);
    }
  }

  async function handlePreview() {
    if (!isTauri || isPreviewing) return;
    isPreviewing = true;
    try {
      await invoke("speak_now", { text: PREVIEW_TEXT });
    } catch (e) {
      toast.error(`Preview failed: ${e}`);
    } finally {
      isPreviewing = false;
    }
  }
</script>

<div class="mx-auto w-full max-w-2xl px-6 py-8">
  <header class="mb-6 flex items-center gap-3">
    <Wand2 class="text-primary" size={24} />
    <h1 class="text-2xl font-semibold tracking-tight">{$_("effects.title")}</h1>
  </header>

  <p class="text-muted-foreground mb-6 text-sm">{$_("effects.description")}</p>

  {#if config}
    <div class="border-border space-y-4 rounded-lg border p-4">
      <SettingRow label={$_("settings.effects.active")} tooltip={$_("effects.activeTooltip")}>
        <Select
          options={effectOptions}
          value={activeEffect}
          onchange={(e: Event) =>
            handleEffectChange((e.target as HTMLSelectElement).value)}
          class="w-44"
        />
      </SettingRow>

      <div class="flex justify-end">
        <Button onclick={handlePreview} disabled={isPreviewing}>
          {isPreviewing ? $_("effects.previewing") : $_("effects.preview")}
        </Button>
      </div>
    </div>
  {:else}
    <p class="text-muted-foreground text-sm">{$_("common.loading")}</p>
  {/if}
</div>
