import type { SupportedLocale } from "$lib/types";

// List of RTL (Right-to-Left) locales
// NOTE: Non-English translations managed externally in src-web/src/lib/locales/DO_NOT_TOUCH/
export const RTL_LOCALES: readonly SupportedLocale[] = ["ar", "he"];

// Check if a locale is RTL
export function isRtlLocale(locale: SupportedLocale): boolean {
  return RTL_LOCALES.includes(locale);
}

// Get display name for a locale
export function getLocaleDisplayName(locale: SupportedLocale): string {
  const names: Partial<Record<SupportedLocale, string>> = {
    en: "English"
    // TODO: Re-enable when translation keys stabilize
    // es: "Español",
    // ar: "العربية",
    // he: "עברית"
  };
  return names[locale] || locale;
}

// Get all supported locales with display names
// NOTE: Non-English translations are managed externally in src-web/src/lib/locales/DO_NOT_TOUCH/
// During pre-production, only English is available - other locales will be enabled when keys stabilize
export function getSupportedLocales(): Array<{ value: SupportedLocale; label: string }> {
  return [
    { value: "en", label: "English" }
    // TODO: Re-enable other locales once translation keys stabilize for release
    // { value: "es", label: "Español" },
    // { value: "ar", label: "العربية" },
    // { value: "he", label: "עברית" }
  ];
}
