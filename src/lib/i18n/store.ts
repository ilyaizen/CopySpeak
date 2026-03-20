import { derived } from "svelte/store";
import { locale as svelteLocale } from "svelte-i18n";
import type { SupportedLocale } from "$lib/types";

// RTL locales list
const RTL_LOCALES: readonly SupportedLocale[] = ["ar"]; // Arabic only for now

// Store for current locale
export const locale = svelteLocale;

// Derived store for RTL detection
export const isRtl = derived<typeof svelteLocale, boolean>(
  svelteLocale,
  ($locale) => {
    return RTL_LOCALES.includes($locale as SupportedLocale);
  }
);

// Load locale from config
export async function loadLocaleFromConfig(savedLocale: SupportedLocale): Promise<void> {
  locale.set(savedLocale);
}

// Get initial locale (for SSR/layout load)
export function getInitialLocale(): SupportedLocale {
  if (typeof window === "undefined") {
    return "en";
  }
  return "en"; // Will be overridden after config loads
}
