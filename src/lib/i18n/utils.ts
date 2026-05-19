import type { SupportedLocale } from "$lib/types";

// List of RTL (Right-to-Left) locales
export const RTL_LOCALES: readonly SupportedLocale[] = ["ar", "he"];

// Check if a locale is RTL
export function isRtlLocale(locale: SupportedLocale): boolean {
  return RTL_LOCALES.includes(locale);
}

// Get display name for a locale
export function getLocaleDisplayName(locale: SupportedLocale): string {
  const names: Partial<Record<SupportedLocale, string>> = {
    en: "English"
  };
  return names[locale] || locale;
}

// Get all supported locales with display names
export function getSupportedLocales(): Array<{ value: SupportedLocale; label: string }> {
  return [{ value: "en", label: "English" }];
}
