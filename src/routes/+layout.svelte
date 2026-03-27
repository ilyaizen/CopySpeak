<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { ModeWatcher, setMode } from "mode-watcher";
  import { invoke, isTauri } from "$lib/services/tauri";
  import AppHeader from "$lib/components/layout/app-header.svelte";
  import AppFooter from "$lib/components/layout/app-footer.svelte";
  import { startHistoryEventListeners, stopHistoryEventListeners } from "$lib/utils/history-events";
  import type { AppConfig } from "$lib/types";
  import "./+layout.css";
  import { Sonner } from "$lib/components/ui/sonner/index.js";
  import { TooltipProvider } from "$lib/components/ui/tooltip/index.js";
  import GlobalPlayer from "$lib/components/global-player.svelte";
  import { setLocale } from "$lib/i18n";
  import { isRtl } from "$lib/i18n/store";

  let { children } = $props();

  // Synchronously detect the Tauri window label.
  // In Tauri v2, this reads from window.__TAURI_INTERNALS__.metadata.currentWindow.label,
  // which is available synchronously when the page loads inside Tauri.
  function getTauriWindowLabel(): string | null {
    try {
      const internals = (window as any).__TAURI_INTERNALS__;
      return internals?.metadata?.currentWindow?.label ?? null;
    } catch {
      return null;
    }
  }

  const tauriWindowLabel = typeof window !== "undefined" ? getTauriWindowLabel() : null;
  // Detect HUD window via two independent signals:
  // 1. Window label from __TAURI_INTERNALS__ (most reliable in Tauri context)
  // 2. URL path (reliable since Tauri loads the HUD window at /hud)
  //    In Tauri v2 dev mode, window-specific url configs may be ignored and all
  //    windows load devUrl ("/"), so label detection is the primary signal.
  //    But if the URL is already /hud (e.g. production or url config works), use that.
  const isHudByLabel = tauriWindowLabel === "hud";
  const isHudByPath = typeof window !== "undefined" && window.location.pathname === "/hud";
  const isHudWindow = isHudByLabel || isHudByPath;

  const isOnboarding = $derived(page.url.pathname === "/onboarding");
  // isHud is true when we're on the /hud route.
  // isHudByPath is a synchronous const checked before SvelteKit hydration — include it
  // so the HUD renders correctly even if page.url.pathname hasn't updated yet.
  const isHud = $derived(page.url.pathname === "/hud" || isHudByPath);

  let unlistenSpeak: (() => void) | null = null;

  onMount(async () => {
    console.log(
      "[LAYOUT] onMount, isHudWindow:",
      isHudWindow,
      "isHud:",
      isHud,
      "pathname:",
      page.url.pathname
    );

    // If we're in the Tauri "hud" window but not at the /hud route (dev mode),
    // navigate to /hud immediately so SvelteKit renders the correct page.
    if (isHudWindow && page.url.pathname !== "/hud") {
      console.log("[LAYOUT] Navigating to /hud");
      await goto("/hud", { replaceState: true });
      return;
    }

    // HUD page — skip all main app shell initialization
    if (isHud) {
      console.log("[LAYOUT] HUD detected, skipping app shell");
      return;
    }

    // Sync appearance with config on app load
    try {
      const config = await invoke<AppConfig>("get_config");

      // Sync appearance
      const appearance = config.general.appearance;
      if (appearance === "system") {
        setMode("system");
      } else if (appearance === "light") {
        setMode("light");
      } else if (appearance === "dark") {
        setMode("dark");
      }

      // Sync locale
      const savedLocale = config.general.locale;
      if (savedLocale) {
        setLocale(savedLocale);
      }
    } catch (e) {
      console.error("Failed to sync appearance/locale:", e);
    }

    // First-run check
    try {
      const hasConfig = await invoke<boolean>("config_exists");
      if (!hasConfig) {
        goto("/onboarding");
        return;
      }
    } catch (e) {
      console.error("Failed to check config:", e);
    }

    await startHistoryEventListeners();

    if (isTauri) {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");

        unlistenSpeak = await listen<{ text: string }>("speak-request", async (e) => {
          console.log("[LAYOUT] speak-request received, text length:", e.payload.text.length);
          try {
            // Use speak_queued so large texts are automatically paginated
            // into fragments and played sequentially.
            await tauriInvoke("speak_queued", { text: e.payload.text });
            console.log("[LAYOUT] speak_queued completed successfully");
          } catch (err) {
            console.error("[LAYOUT] speak_queued failed:", err);
          }
        });
        console.log("[LAYOUT] speak-request listener registered");
      } catch (e) {
        console.error("Failed to set up listeners:", e);
      }
    }
  });

  // Cleanup event listeners when app unmounts
  onDestroy(async () => {
    await stopHistoryEventListeners();
    if (unlistenSpeak) unlistenSpeak();
  });
</script>

{#if isHud}
  {@render children()}
{:else if isHudWindow}
  <!-- HUD window is navigating to /hud — render nothing during transition -->
{:else}
  <ModeWatcher />
  <Sonner position="bottom-left" richColors />
  <GlobalPlayer />

  <TooltipProvider delayDuration={300}>
    <div
      class="bg-background grid h-screen grid-rows-[auto_1fr_auto] overflow-hidden"
      dir={$isRtl ? "rtl" : "ltr"}
    >
      {#if !isOnboarding}
        <AppHeader />
      {/if}

      <main
        class={isOnboarding ? "w-full overflow-y-auto" : "w-full overflow-y-auto px-4 py-6 sm:px-6"}
      >
        {@render children()}
      </main>

      {#if !isOnboarding}
        <AppFooter />
      {/if}
    </div>
  </TooltipProvider>
{/if}
