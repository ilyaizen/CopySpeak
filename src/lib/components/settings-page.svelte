<script lang="ts">
  import GeneralSettings from "#lib/components/settings/general-settings.svelte";
  import AppearanceSettings from "#lib/components/settings/appearance-settings.svelte";
  import PlaybackSettings from "#lib/components/settings/playback-settings.svelte";
  import AdvancedSettings from "#lib/components/settings/advanced-settings.svelte";
  import PaginationSettings from "#lib/components/settings/pagination-settings.svelte";
  import HotkeySettings from "#lib/components/settings/hotkey-settings.svelte";
  import SanitizationSettings from "#lib/components/settings/sanitization-settings.svelte";
  import PostProcessSettings from "#lib/components/settings/post-process-settings.svelte";
  import HistorySettings from "#lib/components/settings/history-settings.svelte";
  import ImportExportSettings from "#lib/components/settings/import-export-settings.svelte";
  import AboutSettings from "#lib/components/settings/about-settings.svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { invoke } from "#lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { onMount, onDestroy, tick } from "svelte";
  import { _ } from "svelte-i18n";
  import { History, Info, Settings, Sparkles, Volume2, Wrench } from "@lucide/svelte";
  import { hideSaveBar } from "#lib/stores/save-bar.svelte";
  import { syncSaveBarOrAutoSave, flushAutoSave } from "#lib/stores/auto-save.svelte";

  import type { AppConfig, RetriggerMode } from "#lib/types";
  import { isHistoryEffective, isPostProcessEffective } from "#lib/experimental-gates";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  type SettingsSection = "general" | "playback" | "history" | "advanced" | "postProcess" | "about";

  // Sidebar highlight: set by the scroll-spy, or directly by a sidebar click.
  let activeSection = $state<SettingsSection>("general");

  const retriggerModeOptions = $derived([
    { value: "interrupt", label: $_("settings.playback.stopAndRestart") },
    { value: "queue", label: $_("settings.playback.queueAfterCurrent") }
  ] satisfies { value: RetriggerMode; label: string }[]);

  const historyEffective = $derived(isHistoryEffective(localConfig));
  const postProcessEffective = $derived(isPostProcessEffective(localConfig));
  const sections: {
    id: SettingsSection;
    labelKey: string;
    icon: typeof Settings;
  }[] = [
    { id: "general", labelKey: "settings.tabs.general", icon: Settings },
    { id: "playback", labelKey: "settings.tabs.playback", icon: Volume2 },
    { id: "advanced", labelKey: "settings.tabs.advanced", icon: Wrench },
    { id: "history", labelKey: "settings.tabs.history", icon: History },
    {
      id: "postProcess",
      labelKey: "settings.tabs.postProcess",
      icon: Sparkles
    },
    { id: "about", labelKey: "settings.tabs.about", icon: Info }
  ];

  let visibleSections = $derived(
    sections.filter(
      (section) =>
        (section.id !== "history" || historyEffective) &&
        (section.id !== "postProcess" || postProcessEffective)
    )
  );

  $effect(() => {
    if (!visibleSections.some((section) => section.id === activeSection)) {
      activeSection = "advanced";
      void tick().then(() => {
        scrollToSection("advanced");
        document.getElementById("advanced")?.focus({ preventScroll: true });
      });
    }
  });

  // Scroll-spy. Every section renders on one page. A sidebar click scrolls to
  // its section; an IntersectionObserver highlights the section crossing the
  // reading band (20%-40% of the window). The layout's <main> is the scroll
  // container, not the window, so scroll events are captured on the document.
  let isScrollingToSection = false;
  let scrollSettleTimer: ReturnType<typeof setTimeout> | undefined;

  function scrollContainerOf(el: HTMLElement): HTMLElement | null {
    let node: HTMLElement | null = el.parentElement;
    while (node) {
      const { overflowY } = getComputedStyle(node);
      if (
        (overflowY === "auto" || overflowY === "scroll") &&
        node.scrollHeight > node.clientHeight
      ) {
        return node;
      }
      node = node.parentElement;
    }
    return null;
  }

  // A short last section never reaches the reading band, so at the bottom of
  // the scroll container the last visible section is the active one.
  function atScrollBottom(): boolean {
    const probe = document.getElementById("general");
    const scroller = probe ? scrollContainerOf(probe) : null;
    if (!scroller) return false;
    return scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
  }

  function scheduleScrollRelease() {
    clearTimeout(scrollSettleTimer);
    // Smooth scrolling emits scroll events until it lands; release once they stop.
    scrollSettleTimer = setTimeout(() => {
      isScrollingToSection = false;
    }, 150);
  }

  function onDocumentScroll() {
    if (isScrollingToSection) {
      scheduleScrollRelease();
      return;
    }
    if (!atScrollBottom()) return;
    const last = visibleSections.at(-1);
    if (last) activeSection = last.id;
  }

  function scrollToSection(id: SettingsSection) {
    activeSection = id;
    isScrollingToSection = true;
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
    scheduleScrollRelease();
  }

  $effect(() => {
    // Re-created whenever the section list changes, so the Post-processing entry
    // is observed while it exists and dropped while it does not.
    if (isLoading || !localConfig) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (isScrollingToSection || atScrollBottom()) return;
        for (const entry of entries) {
          if (entry.isIntersecting) {
            // SAFETY: only the section ids from `sections` are observed.
            activeSection = entry.target.id as SettingsSection;
          }
        }
      },
      { rootMargin: "-20% 0px -60% 0px", threshold: 0 }
    );
    for (const section of visibleSections) {
      const el = document.getElementById(section.id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  });

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
      toast.error($_("toast.error.loadSettings"));
    } finally {
      isLoading = false;
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    try {
      await invoke("set_config", { newConfig: localConfig });
      originalConfig = JSON.parse(JSON.stringify(localConfig));

      window.location.reload();
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error($_("toast.error.saveFailed", { values: { error: String(e) } }));
    }
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
      $_("settings.actions.save"),
      $_("settings.actions.cancel")
    );
    return () => hideSaveBar();
  });

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
      toast.error($_("toast.error.cleanupFailed", { values: { error: String(e) } }));
    }
  }

  onMount(async () => {
    document.addEventListener("scroll", onDocumentScroll, { capture: true, passive: true });
    await loadConfig();
    // saveConfig reloads the page, so this also reports a failed re-register.
    const hotkeyError = await invoke<string | null>("get_hotkey_error").catch(() => null);
    if (hotkeyError)
      toast.error($_("toast.error.hotkeyRegistration", { values: { error: hotkeyError } }));
  });

  onDestroy(() => {
    flushAutoSave();
    document.removeEventListener("scroll", onDocumentScroll, { capture: true });
    clearTimeout(scrollSettleTimer);
  });
</script>

<div class="w-full min-w-0">
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
    <div class="flex min-w-0 flex-col items-stretch gap-4 md:flex-row md:items-start">
      <!-- Left sidebar: each entry scrolls to its section -->
      <aside class="min-w-0 shrink-0 self-stretch md:w-36">
        <nav class="sticky top-0">
          <div class="flex flex-wrap gap-1 md:flex-col">
            {#each visibleSections as section (section.id)}
              {@const Icon = section.icon}
              <button
                class="flex items-center gap-2 rounded-md px-2 py-1.5 text-start text-sm font-medium whitespace-nowrap transition-colors md:w-full {activeSection ===
                section.id
                  ? 'bg-primary/10 text-primary border-primary border-s-2'
                  : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
                aria-controls={section.id}
                aria-current={activeSection === section.id ? "page" : undefined}
                onclick={() => scrollToSection(section.id)}
              >
                <Icon class="size-4 shrink-0" />
                {$_(section.labelKey)}
              </button>
            {/each}
          </div>
        </nav>
      </aside>

      <!-- Main content: all sections stacked, one separator between them -->
      <main class="divide-border min-w-0 flex-1 divide-y pb-20">
        <section id="general" class="scroll-mt-4 py-6 first:pt-0">
          <h3 class="text-muted-foreground mb-3 text-sm font-medium">
            {$_("settings.sections.triggers")}
          </h3>
          <div class="divide-border divide-y">
            <div class="pb-4">
              <div class="space-y-4">
                <SettingRow
                  label={$_("settings.triggers.listen")}
                  tooltip={$_("settings.triggers.listenDescription")}
                >
                  <Switch
                    id="listen-double-copy"
                    bind:checked={localConfig.trigger.listen_enabled}
                  />
                </SettingRow>
                {#if localConfig.trigger.listen_enabled}
                  <SettingRow
                    label={$_("settings.triggers.doubleCopyWindow")}
                    tooltip={$_("settings.triggers.doubleCopyWindowDescription")}
                  >
                    <Input
                      type="number"
                      value={String(localConfig.trigger.double_copy_window_ms)}
                      onchange={(e) => {
                        if (!localConfig) return;
                        // SAFETY: this Input is type="number", so e.target is that element.
                        const raw = (e.target as HTMLInputElement).value;
                        const lc = localConfig!;
                        lc.trigger.double_copy_window_ms =
                          raw === "" ? 1500 : Math.max(100, Number(raw));
                      }}
                      class="w-24"
                    />
                  </SettingRow>
                {/if}
                <HotkeySettings bind:localConfig {errors} />
              </div>
            </div>

            <div class="py-4">
              <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                {$_("settings.sections.startup")}
              </h3>
              <GeneralSettings bind:localConfig />
            </div>

            <div class="pt-4">
              <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                {$_("settings.sections.appearance")}
              </h3>
              <AppearanceSettings bind:localConfig />
            </div>
          </div>
        </section>

        <section
          id="playback"
          aria-label={$_("settings.tabs.playback")}
          class="scroll-mt-4 py-6 first:pt-0"
        >
          <PlaybackSettings bind:localConfig {retriggerModeOptions} />
        </section>

        <section
          id="advanced"
          tabindex="-1"
          class="focus-visible:ring-ring scroll-mt-4 py-6 outline-none first:pt-0 focus-visible:ring-2"
        >
          <h2 class="mb-4 text-base font-semibold">
            {$_("settings.tabs.advanced")}
          </h2>
          <div class="divide-border divide-y">
            <div class="pb-4">
              <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                {$_("settings.sections.pagination")}
              </h3>
              <PaginationSettings bind:localConfig />
            </div>

            <div class="py-4">
              <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                {$_("settings.sections.sanitization")}
              </h3>
              <SanitizationSettings bind:localConfig />
            </div>

            <AdvancedSettings bind:localConfig />
          </div>
        </section>

        {#if historyEffective}
          <section
            id="history"
            aria-label={$_("settings.tabs.history")}
            class="scroll-mt-4 py-6 first:pt-0"
          >
            <HistorySettings bind:localConfig onRunCleanup={handleRunCleanup} />
          </section>
        {/if}

        {#if postProcessEffective}
          <section
            id="postProcess"
            aria-label={$_("settings.tabs.postProcess")}
            class="scroll-mt-4 py-6 first:pt-0"
          >
            <PostProcessSettings bind:localConfig />
          </section>
        {/if}

        <section
          id="about"
          aria-label={$_("settings.tabs.about")}
          class="scroll-mt-4 py-6 first:pt-0"
        >
          <div class="divide-border divide-y">
            <div class="pb-4">
              <AboutSettings bind:localConfig showDebugMode={true} />
            </div>

            <div class="pt-4">
              <h3 class="text-muted-foreground mb-3 text-sm font-medium">
                {$_("settings.sections.importExport")}
              </h3>
              <ImportExportSettings
                {localConfig}
                onImport={handleImport}
                onReset={resetToDefaults}
              />
            </div>
          </div>
        </section>
      </main>
    </div>
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
