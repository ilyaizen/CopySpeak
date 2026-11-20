<script lang="ts">
  import GeneralSettings from "#lib/components/settings/general-settings.svelte";
  import AppearanceSettings from "#lib/components/settings/appearance-settings.svelte";
  import PlaybackSettings from "#lib/components/settings/playback-settings.svelte";
  import AdvancedSettings from "#lib/components/settings/advanced-settings.svelte";
  import PaginationSettings from "#lib/components/settings/pagination-settings.svelte";
  import IdleShutdownSettings from "#lib/components/settings/idle-shutdown-settings.svelte";
  import SanitizationSettings from "#lib/components/settings/sanitization-settings.svelte";
  import PostProcessSettings from "#lib/components/settings/post-process-settings.svelte";
  import HistorySettings from "#lib/components/settings/history-settings.svelte";
  import ImportExportSettings from "#lib/components/settings/import-export-settings.svelte";
  import OtherSettings from "#lib/components/settings/other-settings.svelte";
  import About from "#lib/components/settings/about.svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { invoke } from "#lib/services/tauri";
  import { toast } from "svelte-sonner";
  import type Lenis from "lenis";
  import { getContext, onMount, onDestroy, tick } from "svelte";
  import { _ } from "svelte-i18n";
  import { hideSaveBar } from "#lib/stores/save-bar.svelte";
  import { syncSaveBarOrAutoSave, flushAutoSave } from "#lib/stores/auto-save.svelte";

  import type { AppConfig, RetriggerMode } from "#lib/types";
  import { isHistoryEffective, isPostProcessEffective } from "#lib/experimental-gates";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  const getLenis = getContext<(() => Lenis | undefined) | undefined>("app-lenis");
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
  }[] = [
    { id: "general", labelKey: "settings.tabs.general" },
    { id: "playback", labelKey: "settings.tabs.playback" },
    { id: "advanced", labelKey: "settings.tabs.advanced" },
    { id: "history", labelKey: "settings.tabs.history" },
    { id: "postProcess", labelKey: "settings.tabs.postProcess" },
    { id: "about", labelKey: "settings.tabs.misc" }
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
    const section = document.getElementById(id);
    const lenis = getLenis?.();
    if (section && lenis) {
      // Lenis clamps the target to its cached limit. Re-measure first so a section
      // that just grew stays reachable within the same click.
      lenis.dimensions.resize();
      lenis.scrollTo(section);
    } else
      section?.scrollIntoView({
        behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth",
        block: "start"
      });
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
      localConfig?.general.experimental_enabled === true &&
        localConfig.general.auto_save_profiles === true,
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
    <div class="mb-5 px-1">
      <h1 class="text-foreground text-2xl font-semibold tracking-tight">{$_("settings.title")}</h1>
    </div>
    <div class="flex min-w-0 flex-col items-stretch gap-5 md:flex-row md:items-start">
      <!-- Left sidebar: each entry scrolls to its section -->
      <aside class="min-w-0 shrink-0 self-stretch md:w-40">
        <nav class="border-border bg-card/70 sticky top-2 rounded-xl border p-1.5 shadow-sm backdrop-blur-sm">
          <div class="flex flex-wrap gap-1 md:flex-col">
            {#each visibleSections as section (section.id)}
              <button
                class="side-nav-button settings-nav-button focus-visible:ring-ring flex items-center rounded-lg px-3 py-2 text-start font-medium whitespace-nowrap transition-[background-color,color,box-shadow] focus-visible:ring-2 focus-visible:outline-none md:w-full {activeSection === section.id ? 'bg-primary/10 text-foreground shadow-[inset_0_0_0_1px_color-mix(in_oklch,var(--primary),transparent_82%)]' : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground'}"
                aria-controls={section.id}
                aria-current={activeSection === section.id ? "page" : undefined}
                onclick={() => scrollToSection(section.id)}
              >
                <span class="side-nav-label">{$_(section.labelKey)}</span>
              </button>
            {/each}
          </div>
        </nav>
      </aside>

      <!-- Main content: one card per section, settings grouped inside -->
      <main class="min-w-0 flex-1 space-y-4 pb-20">
        <section
          id="general"
          aria-label={$_("settings.tabs.general")}
          class="settings-card bg-card scroll-mt-4 rounded-lg border"
        >
          <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.general")}</h2>
          <div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.appearance")}
              </h3>
              <AppearanceSettings bind:localConfig />
            </div>

            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.triggers")}
              </h3>
              <div class="space-y-3">
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
              </div>
            </div>

            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.startup")}
              </h3>
              <GeneralSettings bind:localConfig />
            </div>
          </div>
        </section>

        <section
          id="playback"
          aria-label={$_("settings.tabs.playback")}
          class="settings-card bg-card scroll-mt-4 rounded-lg border"
        >
          <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.playback")}</h2>
          <PlaybackSettings bind:localConfig {retriggerModeOptions} />
        </section>

        <section
          id="advanced"
          aria-label={$_("settings.tabs.advanced")}
          tabindex="-1"
          class="settings-card focus-visible:ring-ring bg-card scroll-mt-4 rounded-lg border outline-none focus-visible:ring-2"
        >
          <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.advanced")}</h2>
          <div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.audioDelivery")}
              </h3>
              <SettingRow
                label={$_("settings.playback.streaming")}
                tooltip={$_("settings.playback.streamingDescription")}
              >
                <Switch
                  id="playback-streaming"
                  aria-label={$_("settings.playback.streaming")}
                  bind:checked={localConfig.playback.streaming_enabled}
                />
              </SettingRow>
            </div>

            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.pagination")}
              </h3>
              <PaginationSettings bind:localConfig />
            </div>

            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("engines.daemonIdleTimeout")}
              </h3>
              <IdleShutdownSettings bind:localConfig />
            </div>

            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.sanitization")}
              </h3>
              <SanitizationSettings bind:localConfig />
            </div>
            <div class="settings-separator border-border my-5 border-t"></div>

            <AdvancedSettings bind:localConfig {errors} />
          </div>
        </section>

        {#if historyEffective}
          <section
            id="history"
            aria-label={$_("settings.tabs.history")}
            class="settings-card bg-card scroll-mt-4 rounded-lg border"
          >
            <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.history")}</h2>
            <HistorySettings bind:localConfig onRunCleanup={handleRunCleanup} />
          </section>
        {/if}

        {#if postProcessEffective}
          <section
            id="postProcess"
            aria-label={$_("settings.tabs.postProcess")}
            class="settings-card bg-card scroll-mt-4 rounded-lg border"
          >
            <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.postProcess")}</h2>
            <PostProcessSettings bind:localConfig />
          </section>
        {/if}

        <section
          id="about"
          aria-label={$_("settings.tabs.misc")}
          class="settings-card bg-card scroll-mt-4 rounded-lg border"
        >
          <h2 class="mb-3 text-base font-semibold">{$_("settings.tabs.misc")}</h2>
          <OtherSettings bind:localConfig showDebugMode={true}>
            <div class="settings-separator border-border my-5 border-t"></div>
            <div>
              <h3 class="settings-category mb-3">
                {$_("settings.sections.importExport")}
              </h3>
              <ImportExportSettings
                {localConfig}
                onImport={handleImport}
                onReset={resetToDefaults}
              />
            </div>
          </OtherSettings>
        </section>
      </main>
    </div>
    <About />
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
