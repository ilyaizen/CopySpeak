import { derived } from "svelte/store";
import { locale as svelteLocale } from "svelte-i18n";
import type { SupportedLocale } from "#lib/types";
import { isRtlLocale } from "#lib/i18n/utils";

// Store for current locale
export const locale = svelteLocale;

// Derived store for RTL detection
export const isRtl = derived<typeof svelteLocale, boolean>(svelteLocale, ($locale) => {
  // SAFETY: locales are set only through loadLocaleFromConfig/setLocale (SupportedLocale).
  return isRtlLocale($locale as SupportedLocale);
});

// Load locale from config
export async function loadLocaleFromConfig(savedLocale: SupportedLocale): Promise<void> {
  void locale.set(savedLocale);
}

// Get initial locale (for SSR/layout load)
export function getInitialLocale(): SupportedLocale {
  if (!("window" in globalThis)) {
    return "en";
  }
  return "en"; // Will be overridden after config loads
}
