import type { SupportedLocale } from "$lib/types";

// List of RTL (Right-to-Left) locales
export const RTL_LOCALES: readonly SupportedLocale[] = ["ar", "he"];

// Check if a locale is RTL
export function isRtlLocale(locale: SupportedLocale): boolean {
  return RTL_LOCALES.includes(locale);
}

// Get display name for a locale
type LocaleNames = { [K in SupportedLocale]?: string };

export function getLocaleDisplayName(locale: SupportedLocale): string {
  const names: LocaleNames = {
    en: "English",
    es: "Español"
    // ar: "العربية",
    // he: "עברית"
  };
  return names[locale] || locale;
}

// Get all supported locales with display names.
// Keep in sync with the register() calls in index.ts: a listed locale needs a catalog there.
export function getSupportedLocales(): Array<{ value: SupportedLocale; label: string }> {
  return [
    { value: "en", label: "English" },
    { value: "es", label: "Español" }
    // { value: "ar", label: "العربية" },
    // { value: "he", label: "עברית" }
  ];
}

// Runtime check for values from config, which the Rust side stores as plain strings
export function isSupportedLocale(value: string): value is SupportedLocale {
  return getSupportedLocales().some((locale) => locale.value === value);
}
