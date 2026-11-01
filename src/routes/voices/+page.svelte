<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { invoke, isTauri } from "#lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { hideSaveBar } from "#lib/stores/save-bar.svelte";
  import { syncSaveBarOrAutoSave, flushAutoSave } from "#lib/stores/auto-save.svelte";
  import ProfileManager from "#lib/components/engine/profile-manager.svelte";
  import type { AppConfig } from "#lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  // HMR can destroy this instance while onMount still awaits; a listener that
  // resolves afterwards would outlive it and fire alongside the new instance's.
  let destroyed = false;
  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );

  // Monotonic token: a config-changed reload can overlap the initial load (or
  // another reload); only the newest response may land.
  let loadSeq = 0;

  // `initial` shows the full-page spinner. Background reloads keep the
  // ProfileManager mounted so its selection and open dialogs survive.
  async function loadConfig(initial = false) {
    const seq = ++loadSeq;
    if (initial) isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      if (destroyed || seq !== loadSeq) return;
      localConfig = structuredClone(config);
      originalConfig = structuredClone(config);
    } catch (e) {
      if (destroyed || seq !== loadSeq) return;
      console.error("Failed to load config:", e);
      toast.error($_("toast.error.loadConfig"));
    } finally {
      if (seq === loadSeq) isLoading = false;
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    // Snapshot what is sent: edits made while the save is in flight must stay
    // dirty instead of being marked saved.
    const sent = $state.snapshot(localConfig);
    try {
      await invoke("set_config", { newConfig: sent });
      originalConfig = sent;
      toast.success($_("toast.success.voicesSaved"));
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error($_("toast.error.voicesSave", { values: { error: String(e) } }));
    }
  }

  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = $state.snapshot(originalConfig);
  }

  $effect(() => {
    syncSaveBarOrAutoSave(
      hasChanges,
      localConfig?.general.auto_save_profiles === true,
      saveConfig,
      cancelChanges,
      $_("settings.actions.save"),
      $_("settings.actions.cancel")
    );
    return () => hideSaveBar();
  });

  let unlistenConfig: (() => void) | null = null;

  onMount(async () => {
    await loadConfig(true);
    if (isTauri && !destroyed) {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenConfig = await listen("config-changed", () => {
          // Never clobber unsaved edits with an external change; the save bar
          // still owns them. Our own saves leave nothing dirty, so they reload.
          if (hasChanges) return;
          void loadConfig();
        });
        if (destroyed) unlistenConfig();
      } catch {}
    }
  });

  onDestroy(() => {
    destroyed = true;
    flushAutoSave();
    if (unlistenConfig) unlistenConfig();
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
    <main class="w-full min-w-0 pb-20">
      <ProfileManager bind:localConfig />
    </main>
  {:else}
    <div class="flex min-h-[60vh] items-center justify-center px-6">
      <div class="mx-auto w-full max-w-sm text-center">
        <h2 class="mb-2 text-xl font-semibold">{$_("voices.loadFailed")}</h2>
        <Button onclick={() => loadConfig(true)}>{$_("common.tryAgain")}</Button>
      </div>
    </div>
  {/if}
</div>
