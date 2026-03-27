<script lang="ts">
  import GeneralSettings from "$lib/components/settings/general-settings.svelte";
  import AppearanceSettings from "$lib/components/settings/appearance-settings.svelte";
  import PlaybackSettings from "$lib/components/settings/playback-settings.svelte";
  import PaginationSettings from "$lib/components/settings/pagination-settings.svelte";
  import HotkeySettings from "$lib/components/settings/hotkey-settings.svelte";
  import SanitizationSettings from "$lib/components/settings/sanitization-settings.svelte";
  import HistorySettings from "$lib/components/settings/history-settings.svelte";
  import ImportExportSettings from "$lib/components/settings/import-export-settings.svelte";
  import AboutSettings from "$lib/components/settings/about-settings.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { invoke } from "$lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";

  import type { AppConfig, HudPosition } from "$lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let activeTab = $state<"general" | "advanced" | "about">("general");

  // Staggered rendering: mount components one-by-one to avoid WebView2 crash
  let mountedCount = $state(999);

  const retriggerModeOptions = [
    { value: "stop", label: $_("settings.playback.stopAndRestart") },
    { value: "queue", label: $_("settings.playback.queueAfterCurrent") },
    { value: "ignore", label: $_("settings.playback.ignoreNewTrigger") }
  ];

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
    localConfig?.hud
      ? localConfig.hud.enabled
        ? localConfig.hud.position
        : "disabled"
      : "disabled"
  );

  function handleHudChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    const value = target.value;
    if (!localConfig?.hud) return;

    if (value === "disabled") {
      localConfig.hud.enabled = false;
    } else {
      localConfig.hud.enabled = true;
      localConfig.hud.position = value as HudPosition;
    }
  }

  const tabs = [
    { id: "general" as const, labelKey: "settings.tabs.general" },
    { id: "advanced" as const, labelKey: "settings.tabs.advanced" },
    { id: "about" as const, labelKey: "settings.tabs.about" }
  ];

  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );
  let errors = $state<Record<string, string>>({});

  async function loadConfig() {
    isLoading = true;
    errors = {};
    try {
      const config = await invoke<AppConfig>("get_config");
      localConfig = JSON.parse(JSON.stringify(config));
      originalConfig = JSON.parse(JSON.stringify(config));
    } catch (e) {
      console.error("Failed to load config:", e);
      errors.load = e instanceof Error ? e.message : String(e);
      toast.error("Failed to load settings");
    } finally {
      isLoading = false;
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    isSaving = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      originalConfig = JSON.parse(JSON.stringify(localConfig));

      // Reload page to apply new locale and appearance
      window.location.reload();
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`Failed to save settings: ${e}`);
    } finally {
      isSaving = false;
    }
  }

  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = JSON.parse(JSON.stringify(originalConfig));
  }

  async function resetToDefaults() {
    await invoke("reset_config");
    await loadConfig();
  }

  function handleImport(config: AppConfig) {
    localConfig = JSON.parse(JSON.stringify(config));
  }

  async function handleRunCleanup() {
    try {
      await invoke("run_history_cleanup");
      toast.success($_("toast.success.historyCleanup"));
    } catch (e) {
      toast.error(`Cleanup failed: ${e}`);
    }
  }

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
        <p class="text-muted-foreground">{$_("settings.title")}...</p>
      </div>
    </div>
  {:else if localConfig}
    <div class="flex flex-row gap-4">
      <!-- Left Sidebar - Tab Navigation -->
      <aside class="w-28 shrink-0">
        <nav class="sticky top-24">
          <div class="space-y-0.5">
            {#each tabs as tab}
              <button
                class="w-full rounded-md px-2 py-1.5 text-left text-sm font-medium transition-colors {activeTab ===
                tab.id
                  ? 'bg-primary/10 text-primary border-primary border-l-2'
                  : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
                onclick={() => (activeTab = tab.id)}
              >
                {$_(tab.labelKey)}
              </button>
            {/each}
          </div>
        </nav>
      </aside>

      <!-- Main Content -->
      <main class="flex-1 space-y-6 pb-20">
        {#if activeTab === "general"}
          <!-- General Tab -->
          <section class="scroll-mt-32">
            <div class="border-border overflow-hidden rounded-lg border">
              <div class="space-y-0">
                <!-- General -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.general")}
                  </h3>
                  {#if mountedCount > 0}
                    <div class="space-y-4">
                      <!-- Listen (double-copy detection) -->
                      <SettingRow
                        label={$_("settings.triggers.listen")}
                        tooltip={$_("settings.triggers.listenDescription")}
                      >
                        <Switch
                          id="listen-double-copy"
                          bind:checked={localConfig.trigger.listen_enabled}
                        />
                      </SettingRow>
                      <!-- Global Hotkey -->
                      <HotkeySettings bind:localConfig {errors} />
                      <!-- HUD Overlay -->
                      {#if localConfig.hud}
                        <SettingRow
                          label={$_("settings.hud.overlay")}
                          tooltip={$_("settings.hud.enabledDescription")}
                        >
                          <Select
                            options={HUD_OPTIONS}
                            value={hudValue}
                            onchange={handleHudChange}
                            class="w-40"
                          />
                        </SettingRow>
                      {/if}
                    </div>
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- Startup -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.startup")}
                  </h3>
                  {#if mountedCount > 1}
                    <GeneralSettings bind:localConfig />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- Appearance -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.appearance")}
                  </h3>
                  {#if mountedCount > 2}
                    <AppearanceSettings bind:localConfig />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- Playback -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.playback")}
                  </h3>
                  {#if mountedCount > 3}
                    <PlaybackSettings bind:localConfig {retriggerModeOptions} />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- History -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.history")}
                  </h3>
                  {#if mountedCount > 5}
                    <HistorySettings bind:localConfig onRunCleanup={handleRunCleanup} />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </section>
        {:else if activeTab === "advanced"}
          <!-- Advanced Tab -->
          <section class="scroll-mt-32">
            <div class="border-border overflow-hidden rounded-lg border">
              <div class="space-y-0">
                <!-- Pagination -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.pagination")}
                  </h3>
                  {#if mountedCount > 6}
                    <PaginationSettings bind:localConfig />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- Sanitization -->
                <div class="p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.sanitization")}
                  </h3>
                  {#if mountedCount > 7}
                    <SanitizationSettings bind:localConfig />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </section>
        {:else if activeTab === "about"}
          <!-- About Tab -->
          <section class="scroll-mt-32">
            <div class="border-border overflow-hidden rounded-lg border">
              <div class="space-y-0">
                <!-- App Info -->
                <div class="border-border border-b p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.appInfo")}
                  </h3>
                  {#if mountedCount > 8}
                    <AboutSettings bind:localConfig showDebugMode={true} />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>

                <!-- Import / Export -->
                <div class="p-4">
                  <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                    {$_("settings.sections.importExport")}
                  </h3>
                  {#if mountedCount > 9}
                    <ImportExportSettings
                      {localConfig}
                      onImport={handleImport}
                      onReset={resetToDefaults}
                    />
                  {:else}
                    <div class="flex h-24 items-center justify-center">
                      <div
                        class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </section>
        {/if}
      </main>
    </div>

    <!-- Save Bar -->
    {#if hasChanges}
      <div
        class="border-border bg-card fixed right-4 bottom-12 z-60 flex items-center gap-3 border px-4 py-2.5 shadow-lg"
      >
        <Button
          size="sm"
          variant="ghost"
          onclick={cancelChanges}
          disabled={isSaving}
          class="h-8 px-3"
        >
          {$_("settings.actions.cancel")}
        </Button>
        <Button size="sm" onclick={saveConfig} disabled={isSaving || isLoading} class="h-8 px-4">
          {isSaving ? $_("common.loading") : $_("settings.actions.save")}
        </Button>
      </div>
    {/if}
  {:else}
    <div class="flex min-h-[60vh] items-center justify-center px-6">
      <div class="mx-auto w-full max-w-sm text-center">
        <h2 class="mb-2 text-xl font-semibold">{$_("toast.error.loadSettings")}</h2>
        <p class="text-muted-foreground mb-4">
          {errors.load || "An error occurred while loading your settings."}
        </p>
        <Button onclick={loadConfig}>{$_("settings.actions.tryAgain")}</Button>
      </div>
    </div>
  {/if}
</div>
