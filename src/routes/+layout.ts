// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const prerender = true;
export const ssr = false;

import { waitForI18nReady } from "$lib/i18n";
import { browser } from "$app/environment";
import type { SupportedLocale } from "$lib/types";

// Load initial locale before app renders
export async function load() {
  if (browser) {
    // Wait for svelte-i18n to fully initialize before rendering
    // This prevents "Cannot format a message without first setting the initial locale" errors
    await waitForI18nReady();
  }

  return {
    locale: "en" as SupportedLocale
  };
}
