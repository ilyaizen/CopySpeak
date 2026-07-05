<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "$lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { showSaveBar, hideSaveBar } from "$lib/stores/save-bar.svelte";
  import ProfileManager from "$lib/components/engine/profile-manager.svelte";
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
      toast.success("Voices saved successfully");
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`Failed to save: ${e}`);
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
        $_("settings.actions.save"),
        $_("settings.actions.cancel")
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
        <div
          class="border-primary mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
        ></div>
        <p class="text-muted-foreground">{$_("common.loading")}</p>
      </div>
    </div>
  {:else if localConfig}
    <main class="mx-auto max-w-3xl pb-20">
      <ProfileManager bind:localConfig />
    </main>
  {:else}
    <div class="flex min-h-[60vh] items-center justify-center px-6">
      <div class="mx-auto w-full max-w-sm text-center">
        <h2 class="mb-2 text-xl font-semibold">Failed to load</h2>
        <Button onclick={loadConfig}>Try again</Button>
      </div>
    </div>
  {/if}
</div>
