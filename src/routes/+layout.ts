// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const prerender = true;
export const ssr = false;

import { setLocale, waitForI18nReady } from "#lib/i18n";
import { resolveLocale } from "#lib/i18n/utils";
import { invoke, isTauri } from "#lib/services/tauri";
import { browser } from "$app/env";
import type { SupportedLocale } from "#lib/types";

// Empty config locale means no explicit choice, even when another setting was saved.
async function loadSavedLocale(systemLocale: string): Promise<SupportedLocale> {
  try {
    // A tiny scalar avoids the full get_config payload, which has hung on onboarding.
    const savedLocale = await invoke<string>("get_saved_locale");
    return resolveLocale(savedLocale, systemLocale);
  } catch (e) {
    console.error("Failed to load saved locale:", e);
  }
  return resolveLocale("", systemLocale);
}

// Apply the saved locale before the first render, so the UI never paints in English (LTR)
// and then flips. The HUD has no translated strings, so it skips the config read.
export async function load() {
  let locale: SupportedLocale = "en";
  if (browser) {
    // Wait for svelte-i18n to fully initialize before rendering
    // This prevents "Cannot format a message without first setting the initial locale" errors
    await waitForI18nReady();
    if (window.location.pathname !== "/hud") {
      locale = isTauri
        ? await loadSavedLocale(navigator.language)
        : resolveLocale("", navigator.language);
      setLocale(locale);
      // setLocale starts loading the catalog; wait for it so the first render has strings
      await waitForI18nReady();
    }
  }

  return { locale };
}
