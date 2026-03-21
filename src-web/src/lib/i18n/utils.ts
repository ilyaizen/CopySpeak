import type { SupportedLocale } from "$lib/types";

export const RTL_LOCALES: readonly SupportedLocale[] = [];

export function isRtlLocale(locale: SupportedLocale): boolean {
  return RTL_LOCALES.includes(locale);
}

export function getLocaleDisplayName(locale: SupportedLocale): string {
  const names: Record<SupportedLocale, string> = {
    en: "English",
    es: "Español"
  };
  return names[locale] || locale;
}

export function getSupportedLocales(): Array<{ value: SupportedLocale; label: string }> {
  return [
    { value: "en", label: "English" },
    { value: "es", label: "Español" }
  ];
}
