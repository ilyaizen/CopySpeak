import { init, register, locale as localeStore, isLoading } from "svelte-i18n";
import { get } from "svelte/store";
import type { SupportedLocale } from "$lib/types";

const STORAGE_KEY = "copyspeak-locale";

// TODO: Re-enable multi-language support when ready
// Currently only English is active to avoid dead-code warnings during deployment

function getBrowserLocale(): SupportedLocale {
  if (typeof window === "undefined") return "en";
  // TODO: Re-add language detection when i18n is re-enabled
  // const navLang = navigator.language.split("-")[0];
  // if (navLang === "es") return "es";
  // if (navLang === "he") return "he";
  return "en";
}

function getStoredLocale(): SupportedLocale | null {
  if (typeof window === "undefined") return null;
  const stored = localStorage.getItem(STORAGE_KEY);
  // TODO: Re-add other locales when i18n is re-enabled
  // if (stored === "en" || stored === "es" || stored === "ar" || stored === "he") return stored;
  if (stored === "en") return stored;
  return null;
}

// Only register English for now
register("en", () => import("$lib/locales/en.json"));
// TODO: Re-register these when i18n is re-enabled
// register("es", () => import("$lib/locales/es.json"));
// register("ar", () => import("$lib/locales/ar.json"));
// register("he", () => import("$lib/locales/he.json"));

const initialLocale = "en"; // TODO: Re-enable dynamic locale detection: getStoredLocale() ?? getBrowserLocale();

const initPromise = init({
  fallbackLocale: "en",
  initialLocale
});

export { localeStore as locale };

export function setLocale(newLocale: SupportedLocale): void {
  // TODO: Re-enable locale switching when i18n is re-enabled
  // localeStore.set(newLocale);
  // if (typeof window !== "undefined") {
  //   localStorage.setItem(STORAGE_KEY, newLocale);
  // }
}

export function getCurrentLocale(): SupportedLocale {
  return "en"; // TODO: return get(localeStore) as SupportedLocale;
}

export async function waitForI18nReady(): Promise<void> {
  await initPromise;
  while (get(isLoading)) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

export { _ } from "svelte-i18n";
