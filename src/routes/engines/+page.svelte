<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { showSaveBar, hideSaveBar } from "$lib/stores/save-bar.svelte";
  import EngineSetup from "$lib/components/engine/engine-setup.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Loader2 } from "@lucide/svelte";
  import type { AppConfig } from "$lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);

  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );

  async function loadConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      localConfig = JSON.parse(JSON.stringify(config));
      originalConfig = JSON.parse(JSON.stringify(config));
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    try {
      await invoke("set_config", { newConfig: localConfig });
      originalConfig = JSON.parse(JSON.stringify(localConfig));
      toast.success($_("engine.saveBar.saveChanges"));
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`${$_("engine.error.loadFailed")}: ${e}`);
      throw e;
    }
  }

  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = JSON.parse(JSON.stringify(originalConfig));
  }

  $effect(() => {
    if (hasChanges) {
      showSaveBar(
        saveConfig,
        cancelChanges,
        $_("engine.saveBar.saveChanges"),
        $_("engine.saveBar.cancel")
      );
    } else {
      hideSaveBar();
    }
    return () => hideSaveBar();
  });

  onMount(() => {
    loadConfig();
  });
</script>

<div class="w-full">
  {#if isLoading}
    <div class="flex min-h-[60vh] items-center justify-center">
      <div class="text-center">
        <Loader2 class="text-primary mx-auto mb-4 h-8 w-8 animate-spin" />
        <p class="text-muted-foreground">{$_("engine.loading")}</p>
      </div>
    </div>
  {:else if localConfig}
    <main class="mx-auto max-w-3xl pb-4">
      <div class="mb-4">
        <h1 class="text-xl font-semibold">{$_("engines.title")}</h1>
        <p class="text-muted-foreground mt-1 text-sm">{$_("engines.subtitle")}</p>
      </div>
      <EngineSetup bind:localConfig isDirty={hasChanges} onSave={saveConfig} />
    </main>
  {:else}
    <div class="flex min-h-[60vh] items-center justify-center px-6">
      <div class="mx-auto w-full max-w-sm text-center">
        <h2 class="mb-2 text-xl font-semibold">{$_("engine.error.loadFailed")}</h2>
        <Button onclick={loadConfig}>{$_("engine.setup.tryAgain")}</Button>
      </div>
    </div>
  {/if}
</div>
