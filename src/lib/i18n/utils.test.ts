import { expect, it } from "vite-plus/test";
import en from "$lib/locales/en.json";
import es from "$lib/locales/es.json";
import { isSupportedLocale } from "./utils";

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
  expect(isSupportedLocale("")).toBe(false);
  expect(isSupportedLocale("xx")).toBe(false);
});

it("keeps translated catalogs free of keys English does not define", () => {
  const englishKeys = new Set(flattenKeys(en));
  const orphans = flattenKeys(es).filter((key) => !englishKeys.has(key));
  expect(orphans).toEqual([]);
});
