<script lang="ts">
  import GeneralSettings from "$lib/components/settings/general-settings.svelte";
  import AppearanceSettings from "$lib/components/settings/appearance-settings.svelte";
  import PlaybackSettings from "$lib/components/settings/playback-settings.svelte";
  import TriggerSettings from "$lib/components/settings/trigger-settings.svelte";
  import PaginationSettings from "$lib/components/settings/pagination-settings.svelte";
  import HotkeySettings from "$lib/components/settings/hotkey-settings.svelte";
  import SanitizationSettings from "$lib/components/settings/sanitization-settings.svelte";
  import HistorySettings from "$lib/components/settings/history-settings.svelte";
  import ImportExportSettings from "$lib/components/settings/import-export-settings.svelte";
  import HudSettings from "$lib/components/settings/hud-settings.svelte";
  import AboutSettings from "$lib/components/settings/about-settings.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "$lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { onMount, onDestroy } from "svelte";
  import { _ } from "svelte-i18n";

  import type { AppConfig } from "$lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let activeSection = $state("app");
  let activeTab = $state<"general" | "advanced" | "about">("general");

  // Staggered rendering: mount components one-by-one to avoid WebView2 crash
  let mountedCount = $state(999);

  // Flag to prevent IntersectionObserver from fighting with manual scroll clicks
  let isManualScroll = false;

  const hudPositionOptions = [
    { value: "top-left", label: $_("settings.hud.topLeft") },
    { value: "top-center", label: $_("settings.hud.topCenter") },
    { value: "top-right", label: $_("settings.hud.topRight") },
    { value: "bottom-left", label: $_("settings.hud.bottomLeft") },
    { value: "bottom-center", label: $_("settings.hud.bottomCenter") },
    { value: "bottom-right", label: $_("settings.hud.bottomRight") }
  ];

  function handlePositionChange(e: Event): void {
    const target = e.target as HTMLSelectElement;
    if (!localConfig?.hud) return;
    localConfig.hud.position = target.value as any;
  }

  const retriggerModeOptions = [
    { value: "stop", label: $_("settings.playback.stopAndRestart") },
    { value: "queue", label: $_("settings.playback.queueAfterCurrent") },
    { value: "ignore", label: $_("settings.playback.ignoreNewTrigger") }
  ];

  // const audioFormatOptions = [
  //   { value: "wav", label: "WAV (Uncompressed)" },
  //   { value: "mp3", label: "MP3 (Requires ffmpeg)" },
  //   { value: "ogg", label: "OGG Vorbis (Requires ffmpeg)" },
  //   { value: "flac", label: "FLAC (Lossless, Requires ffmpeg)" },
  // ];

  const tabs = [
    { id: "general" as const, labelKey: "settings.tabs.general" },
    { id: "advanced" as const, labelKey: "settings.tabs.advanced" },
    { id: "about" as const, labelKey: "settings.tabs.about" }
  ];
  const tabSections: Record<string, { id: string; categoryKey: string; labelKey: string }[]> = {
    general: [
      { id: "app", categoryKey: "general", labelKey: "settings.sections.startup" },
      { id: "appearance", categoryKey: "appearance", labelKey: "settings.sections.appearance" },
      { id: "playback", categoryKey: "playback", labelKey: "settings.sections.playback" },
      { id: "hud", categoryKey: "hud", labelKey: "settings.sections.hud" },
      { id: "history", categoryKey: "history", labelKey: "settings.sections.history" },
      { id: "triggers", categoryKey: "triggers", labelKey: "settings.sections.triggers" },
      { id: "hotkeys", categoryKey: "hotkeys", labelKey: "settings.sections.hotkeys" }
    ],
    advanced: [
      { id: "pagination", categoryKey: "pagination", labelKey: "settings.sections.pagination" },
      {
        id: "sanitization",
        categoryKey: "sanitization",
        labelKey: "settings.sections.sanitization"
      }
    ],
    about: [{ id: "about", categoryKey: "about", labelKey: "settings.sections.appInfo" }]
  }; // Legacy categories kept for scroll observer compatibility
  const settingsCategories = [
    { id: "app", categoryKey: "general" },
    { id: "playback", categoryKey: "playback" },
    { id: "triggers", categoryKey: "triggers" },
    { id: "pagination", categoryKey: "pagination" },
    { id: "sanitization", categoryKey: "sanitization" },
    { id: "history", categoryKey: "history" },
    { id: "hud", categoryKey: "hud" },
    { id: "about", categoryKey: "about" }
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

  function scrollToSection(sectionId: string) {
    isManualScroll = true;
    activeSection = sectionId;
    const element = document.getElementById(`section-${sectionId}`);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "start" });
    }
    setTimeout(() => {
      isManualScroll = false;
    }, 800);
  }

  function switchTab(tabId: "general" | "advanced" | "about") {
    activeTab = tabId;
    const firstSection = tabSections[tabId][0];
    if (firstSection) {
      activeSection = firstSection.id;
    }
    // Reset scroll position for smooth UX
    window.scrollTo({ top: 0, behavior: "instant" });
  }

  function staggerMount() {
    setupScrollObserver();
  }

  // IntersectionObserver for scroll-aware sidebar tracking
  let observer: IntersectionObserver | null = null;

  function setupScrollObserver() {
    if (observer) observer.disconnect();

    observer = new IntersectionObserver(
      (entries) => {
        if (isManualScroll) return;
        let topEntry: IntersectionObserverEntry | null = null;
        for (const entry of entries) {
          if (entry.isIntersecting) {
            if (!topEntry || entry.boundingClientRect.top < topEntry.boundingClientRect.top) {
              topEntry = entry;
            }
          }
        }
        if (topEntry) {
          const id = topEntry.target.id.replace("section-", "");
          if (activeSection !== id) activeSection = id;
        }
      },
      { rootMargin: "-120px 0px -60% 0px", threshold: 0 }
    );

    for (const cat of settingsCategories) {
      const el = document.getElementById(`section-${cat.id}`);
      if (el) observer.observe(el);
    }
  }

  onMount(() => {
    loadConfig().then(() => {
      requestAnimationFrame(staggerMount);
    });
  });

  onDestroy(() => {
    if (observer) observer.disconnect();
  });

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

  async function handleTestHud() {
    try {
      await invoke("test_show_hud");
      toast.success($_("toast.success.hudTest"));
    } catch (e) {
      toast.error(`HUD test failed: ${e}`);
    }
  }
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
      <!-- Left Sidebar Menu (sticky) -->
      <aside class="w-28 shrink-0">
        <nav class="sticky top-24 space-y-0.5">
          {#each settingsCategories as category}
            <button
              class="w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {activeSection ===
              category.id
                ? 'bg-primary/10 text-primary border-primary border-l-2 font-medium'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
              onclick={() => scrollToSection(category.id)}
            >
              {$_(`settings.categories.${category.categoryKey}`)}
            </button>
          {/each}
        </nav>
      </aside>

      <!-- Main Content -->
      <main class="flex-1 space-y-8 pb-20">
        <!-- General -->
        <section id="section-app" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.general")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.general")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 0}
                <ImportExportSettings
                  {localConfig}
                  onImport={handleImport}
                  onReset={resetToDefaults}
                />
                <GeneralSettings bind:localConfig showDebugMode={true} />
                <AppearanceSettings
                  appearance={localConfig!.general.appearance}
                  onchange={(v) => (localConfig!.general.appearance = v)}
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
        </section>

        <!-- Playback -->
        <section id="section-playback" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.playback")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.playback")}</p>
            </div>
            <div class="p-4">
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
          </div>
        </section>

        <!-- Triggers -->
        <section id="section-triggers" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.triggers")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.triggers")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 4}
                <TriggerSettings bind:localConfig {errors} />
                <div class="border-border mt-4 border-t pt-4">
                  <HotkeySettings bind:localConfig {errors} />
                </div>
              {:else}
                <div class="flex h-24 items-center justify-center">
                  <div
                    class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                  ></div>
                </div>
              {/if}
            </div>
          </div>
        </section>

        <!-- Pagination -->
        <section id="section-pagination" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.pagination")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.pagination")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 5}
                <PaginationSettings bind:localConfig {errors} />
              {:else}
                <div class="flex h-24 items-center justify-center">
                  <div
                    class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                  ></div>
                </div>
              {/if}
            </div>
          </div>
        </section>

        <!-- Sanitization -->
        <section id="section-sanitization" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.sanitization")}</h2>
              <p class="text-muted-foreground text-sm">
                {$_("settings.descriptions.sanitization")}
              </p>
            </div>
            <div class="p-4">
              {#if mountedCount > 6}
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
        </section>

        <!-- History -->
        <section id="section-history" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.history")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.history")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 7}
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
        </section>

        <!-- HUD Overlay -->
        <section id="section-hud" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.hud")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.hud")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 8}
                <HudSettings {localConfig} {hudPositionOptions} {handlePositionChange} />
                <div class="border-border mt-4 border-t pt-4">
                  <Button onclick={handleTestHud} variant="outline" size="sm">
                    {$_("settings.hud.testHud")}
                  </Button>
                </div>
              {:else}
                <div class="flex h-24 items-center justify-center">
                  <div
                    class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                  ></div>
                </div>
              {/if}
            </div>
          </div>
        </section>

        <!-- About -->
        <section id="section-about" class="scroll-mt-32">
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <h2 class="text-lg font-semibold">{$_("settings.categories.about")}</h2>
              <p class="text-muted-foreground text-sm">{$_("settings.descriptions.about")}</p>
            </div>
            <div class="p-4">
              {#if mountedCount > 9}
                <AboutSettings />
              {:else}
                <div class="flex h-24 items-center justify-center">
                  <div
                    class="border-primary h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
                  ></div>
                </div>
              {/if}
            </div>
          </div>
        </section>
      </main>
    </div>

    <!-- Save Bar -->
    {#if hasChanges}
      <div
        class="border-border bg-card fixed right-4 bottom-12 z-60 flex items-center gap-3 border px-4 py-2.5 shadow-lg"
      >
        <!-- <span class="text-muted-foreground text-xs whitespace-nowrap">Unsaved setting changes</span> -->
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
