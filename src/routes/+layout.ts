// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const prerender = true;
export const ssr = false;

import { setLocale, waitForI18nReady } from "#lib/i18n";
import { isSupportedLocale } from "#lib/i18n/utils";
import { invoke, isTauri } from "#lib/services/tauri";
import { browser } from "$app/env";
import type { AppConfig, SupportedLocale } from "#lib/types";

// Read the saved locale from config; anything unsupported falls back to en
async function loadSavedLocale(): Promise<SupportedLocale> {
  try {
    const config = await invoke<AppConfig>("get_config");
    const savedLocale: string = config.general.locale;
    if (isSupportedLocale(savedLocale)) return savedLocale;
    console.warn(`Locale ${savedLocale} is not supported, falling back to en`);
  } catch (e) {
    console.error("Failed to load saved locale:", e);
  }
  return "en";
}

// Apply the saved locale before the first render, so the UI never paints in English (LTR)
// and then flips. The HUD has no translated strings, so it skips the config read.
export async function load() {
  let locale: SupportedLocale = "en";
  if (browser) {
    // Wait for svelte-i18n to fully initialize before rendering
    // This prevents "Cannot format a message without first setting the initial locale" errors
    await waitForI18nReady();
    if (isTauri && window.location.pathname !== "/hud") {
      locale = await loadSavedLocale();
      setLocale(locale);
      // setLocale starts loading the catalog; wait for it so the first render has strings
      await waitForI18nReady();
    }
  }

  return { locale };
}
