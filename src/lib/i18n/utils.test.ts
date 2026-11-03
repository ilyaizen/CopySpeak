import { expect, it } from "vite-plus/test";
import ar from "#lib/locales/ar.json";
import de from "#lib/locales/de.json";
import en from "#lib/locales/en.json";
import es from "#lib/locales/es.json";
import fr from "#lib/locales/fr.json";
import he from "#lib/locales/he.json";
import hi from "#lib/locales/hi.json";
import ru from "#lib/locales/ru.json";
import zh from "#lib/locales/zh.json";
import { isRtlLocale, isSupportedLocale, resolveLocale } from "./utils";

type Catalog = { [key: string]: Catalog | string };

// Dot-notation key paths, matching scripts/generate-i18n-types.js
function flattenKeys(catalog: Catalog, prefix = ""): string[] {
  return Object.entries(catalog).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (Object.prototype.toString.call(value) !== "[object Object]") return [path];
    // SAFETY: a value that is not a string leaf is a nested object, so it is a Catalog.
    return flattenKeys(value as Catalog, path);
  });
}

it("accepts only supported locale codes", () => {
  expect(isSupportedLocale("en")).toBe(true);
  expect(isSupportedLocale("es")).toBe(true);
  expect(isSupportedLocale("he")).toBe(true);
  expect(isSupportedLocale("")).toBe(false);
  expect(isSupportedLocale("xx")).toBe(false);
});

it("marks only right-to-left locales as rtl", () => {
  expect(isRtlLocale("he")).toBe(true);
  expect(isRtlLocale("en")).toBe(false);
  expect(isRtlLocale("es")).toBe(false);
});

it("detects supported system languages from regional locale tags", () => {
  expect(resolveLocale("", "he-IL")).toBe("he");
  expect(resolveLocale("", "ar-EG")).toBe("ar");
  expect(resolveLocale("", "zh-Hant-TW")).toBe("zh");
  expect(resolveLocale("", "DE-de")).toBe("de");
  expect(resolveLocale("", "fr-FR")).toBe("fr");
  expect(resolveLocale("", "es-MX")).toBe("es");
  expect(resolveLocale("", "hi-IN")).toBe("hi");
  expect(resolveLocale("", "ru-RU")).toBe("ru");
  expect(resolveLocale("", "en-US")).toBe("en");
  expect(resolveLocale("", "ja-JP")).toBe("en");
  expect(resolveLocale("", "")).toBe("en");
});

it("preserves an explicitly saved language over the system language", () => {
  expect(resolveLocale("en", "he-IL")).toBe("en");
  expect(resolveLocale("ar", "en-US")).toBe("ar");
  expect(resolveLocale("unsupported", "he-IL")).toBe("en");
});

const translated = { es, he, ar, de, fr, hi, ru, zh } satisfies Record<string, Catalog>;
it("provides About credits and a support label in every locale", () => {
  for (const catalog of [en, es, he, ar, de, fr, hi, ru, zh]) {
    expect(catalog.settings.about.madeWith).toBeTruthy();
    expect(catalog.settings.about.madeWith.split("{heart}")).toHaveLength(2);
    expect(catalog.settings.about.buyMeCoffee).toBeTruthy();
  }
});

for (const [code, catalog] of Object.entries(translated)) {
  it(`keeps ${code} free of keys English does not define`, () => {
    const englishKeys = new Set(flattenKeys(en));
    const orphans = flattenKeys(catalog).filter((key) => !englishKeys.has(key));
    expect(orphans).toEqual([]);
  });
}
