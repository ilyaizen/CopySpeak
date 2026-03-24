import { init, register, locale as localeStore, isLoading } from "svelte-i18n";
import { get } from "svelte/store";
import type { SupportedLocale } from "$lib/types";

// Register translation dictionaries
// NOTE: Non-English translations are managed externally in src-web/src/lib/locales/DO_NOT_TOUCH/
// During pre-production, translation keys change frequently - only English is stable
register("en", () => import("$lib/locales/en.json"));
// register("es", () => import("$lib/locales/es.json"));
// register("ar", () => import("$lib/locales/ar.json"));
// register("he", () => import("$lib/locales/he.json"));

// Initialize with default locale
// Actual locale will be set from AppConfig after load
const initPromise = init({
  fallbackLocale: "en",
  initialLocale: "en"
});

// Export the locale store for use in components
export { localeStore as locale };

// Helper function to set locale programmatically
export function setLocale(newLocale: SupportedLocale): void {
  localeStore.set(newLocale);
}

// Helper function to get current locale value
export function getCurrentLocale(): SupportedLocale {
  return get(localeStore) as SupportedLocale;
}

// Wait for i18n to be ready (locale set and messages loaded)
export async function waitForI18nReady(): Promise<void> {
  await initPromise;
  // Wait for loading to complete using polling
  while (get(isLoading)) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

// Re-export _ formatter for convenience
export { _ } from "svelte-i18n";
