import { expect, it } from "vite-plus/test";
import en from "#lib/locales/en.json";
import es from "#lib/locales/es.json";
import he from "#lib/locales/he.json";
import { isRtlLocale, isSupportedLocale } from "./utils";

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

const translated = { es, he } satisfies Record<string, Catalog>;
for (const [code, catalog] of Object.entries(translated)) {
  it(`keeps ${code} free of keys English does not define`, () => {
    const englishKeys = new Set(flattenKeys(en));
    const orphans = flattenKeys(catalog).filter((key) => !englishKeys.has(key));
    expect(orphans).toEqual([]);
  });
}
