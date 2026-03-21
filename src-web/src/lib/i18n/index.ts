import { init, register, locale as localeStore, isLoading } from "svelte-i18n";
import { get } from "svelte/store";
import type { SupportedLocale } from "$lib/types";

const STORAGE_KEY = "copyspeak-locale";

function getBrowserLocale(): SupportedLocale {
  if (typeof window === "undefined") return "en";
  const navLang = navigator.language.split("-")[0];
  if (navLang === "es") return "es";
  return "en";
}

function getStoredLocale(): SupportedLocale | null {
  if (typeof window === "undefined") return null;
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "es" || stored === "ar") return stored;
  return null;
}

register("en", () => import("$lib/locales/en.json"));
register("es", () => import("$lib/locales/es.json"));

const initialLocale = getStoredLocale() ?? getBrowserLocale();

const initPromise = init({
  fallbackLocale: "en",
  initialLocale
});

export { localeStore as locale };

export function setLocale(newLocale: SupportedLocale): void {
  localeStore.set(newLocale);
  if (typeof window !== "undefined") {
    localStorage.setItem(STORAGE_KEY, newLocale);
  }
}

export function getCurrentLocale(): SupportedLocale {
  return get(localeStore) as SupportedLocale;
}

export async function waitForI18nReady(): Promise<void> {
  await initPromise;
  while (get(isLoading)) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

export { _ } from "svelte-i18n";
