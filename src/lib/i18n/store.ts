import { derived } from "svelte/store";
import { locale as svelteLocale } from "svelte-i18n";
import type { SupportedLocale } from "#lib/types";
import { isRtlLocale } from "#lib/i18n/utils";

// Store for current locale
export const locale = svelteLocale;

// Derived store for RTL detection
export const isRtl = derived<typeof svelteLocale, boolean>(svelteLocale, ($locale) => {
  // SAFETY: locales are set only through setLocale (SupportedLocale).
  return isRtlLocale($locale as SupportedLocale);
});
