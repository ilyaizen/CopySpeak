<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "#lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle
  } from "#lib/components/ui/alert-dialog/index.js";
  import { firstEnvKeyConflict } from "#lib/components/engine/env-keys";
  import { hideSaveBar } from "#lib/stores/save-bar.svelte";
  import { syncSaveBarOrAutoSave, flushAutoSave } from "#lib/stores/auto-save.svelte";
  import EngineSetup from "#lib/components/engine/engine-setup.svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Loader2 } from "@lucide/svelte";
  import type { AppConfig } from "#lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  // engine → winning env var for engines whose key currently comes from .env.
  let envKeySources = $state<Record<string, string>>({});
  // Non-null shows the env-key override warning for that engine.
  let envKeyConflict = $state<{ engine: string; envVar: string } | null>(null);

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
      // Absent in browser mode — the sentinel simply never shows there.
      envKeySources = await invoke<Record<string, string>>("get_env_key_sources").catch(() => ({}));
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error($_("toast.error.loadConfig"));
    } finally {
      isLoading = false;
    }
  }

  // SAFETY: engine ids on tts are exactly the CredentialTarget keys
  // get_env_key_sources reports; each per-engine struct carries api_key.
  function ttsKeys(config: AppConfig | null): Record<string, { api_key?: string }> {
    if (!config) return {};
    return config.tts as unknown as Record<string, { api_key?: string }>;
  }

  async function performSave() {
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

  async function saveConfig() {
    if (!localConfig) return;
    // An env-sourced engine with a freshly typed key needs the override
    // warning first: env precedence keeps the .env key active after saving.
    for (const engine of Object.keys(envKeySources)) {
      const conflict = firstEnvKeyConflict(
        engine,
        envKeySources[engine],
        ttsKeys(localConfig)[engine],
        ttsKeys(originalConfig)[engine]
      );
      if (conflict) {
        envKeyConflict = conflict;
        return;
      }
    }
    await performSave();
  }

  function confirmEnvKeySave() {
    envKeyConflict = null;
    void performSave();
  }

  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = JSON.parse(JSON.stringify(originalConfig));
  }

  $effect(() => {
    syncSaveBarOrAutoSave(
      hasChanges,
      localConfig?.general.auto_save_profiles === true,
      saveConfig,
      cancelChanges,
      $_("engine.saveBar.saveChanges"),
      $_("engine.saveBar.cancel")
    );
    return () => hideSaveBar();
  });

  onMount(() => {
    loadConfig();
  });

  onDestroy(() => {
    flushAutoSave();
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
    <main class="w-full min-w-0 pb-4">
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

<AlertDialog
  open={envKeyConflict !== null}
  onOpenChange={(open) => {
    if (!open) envKeyConflict = null;
  }}
>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>{$_("engine.envKeyDialog.title")}</AlertDialogTitle>
      <AlertDialogDescription>
        {#if envKeyConflict}
          {$_("engine.envKeyDialog.body", {
            values: {
              engine: $_(`engine.${envKeyConflict.engine}.title`),
              envVar: envKeyConflict.envVar
            }
          })}
        {/if}
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>{$_("common.cancel")}</AlertDialogCancel>
      <AlertDialogAction onclick={confirmEnvKeySave}>
        {$_("engine.envKeyDialog.confirm")}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
