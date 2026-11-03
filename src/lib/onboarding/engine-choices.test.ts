import { readFileSync } from "node:fs";
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
import { ENGINE_CHOICES } from "./engine-choices.js";

type Catalog = { [key: string]: Catalog | string };

const CATALOGS: Array<[string, Catalog]> = [
  ["ar", ar],
  ["de", de],
  ["en", en],
  ["es", es],
  ["fr", fr],
  ["he", he],
  ["hi", hi],
  ["ru", ru],
  ["zh", zh]
];

const DASH = /[\u2013\u2014]/;

// Dot-notation lookup against a locale catalog; throws loudly on a missing or
// non-leaf key so a broken key path fails the test instead of the UI.
function get(catalog: Catalog, path: string): string {
  let node: Catalog | string = catalog;
  for (const part of path.split(".")) {
    if (Object.prototype.toString.call(node) !== "[object Object]") {
      throw new Error(`path ${path} descends past a string leaf`);
    }
    // SAFETY: a node that is not a string leaf is a nested object, so it is a Catalog.
    const next: Catalog | string | undefined = (node as Catalog)[part];
    if (next === undefined) throw new Error(`missing key ${path}`);
    node = next;
  }
  if (Object.prototype.toString.call(node) !== "[object String]") {
    throw new Error(`key ${path} is not a string leaf`);
  }
  // SAFETY: a final node that is not an object is the string leaf the path names.
  return node as string;
}

function subtree(catalog: Catalog, key: string): Catalog {
  const node: Catalog | string | undefined = catalog[key];
  if (node === undefined || Object.prototype.toString.call(node) !== "[object Object]") {
    throw new Error(`${key} is not an object`);
  }
  // SAFETY: a value that is not a string leaf is a nested object, so it is a Catalog.
  return node as Catalog;
}

function leafPaths(catalog: Catalog, prefix: string): string[] {
  return Object.entries(catalog).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (Object.prototype.toString.call(value) !== "[object Object]") return [path];
    // SAFETY: a value that is not a string leaf is a nested object, so it is a Catalog.
    return leafPaths(value as Catalog, path);
  });
}

// Every toast the onboarding page can fire.
const PAGE_TOAST_KEYS = [
  "toast.success.welcome",
  "toast.error.saveFailed",
  "toast.error.kokoroInstallFailed",
  "toast.error.kokoroInstallStart"
];

it("offers the five engines in display order", () => {
  expect(ENGINE_CHOICES.map((choice) => choice.id)).toEqual([
    "kokoro",
    "edge",
    "cartesia",
    "elevenlabs",
    "fish"
  ]);
});

it("marks exactly Kokoro, ElevenLabs and Fish as recommended", () => {
  expect(ENGINE_CHOICES.filter((choice) => choice.recommended).map((choice) => choice.id)).toEqual([
    "kokoro",
    "elevenlabs",
    "fish"
  ]);
});

it("keeps card copy short and free of dashes", () => {
  for (const choice of ENGINE_CHOICES) {
    const detail = get(en, choice.detailKey);
    expect(detail.length, choice.id).toBeLessThan(90);
    expect(detail, choice.id).not.toMatch(DASH);
    expect(choice.name, choice.id).not.toMatch(DASH);
    expect(choice.detailKey, choice.id).not.toMatch(DASH);
  }
});

it("presents Fish as free via the included free model", () => {
  const fish = ENGINE_CHOICES.find((choice) => choice.id === "fish");
  if (!fish) throw new Error("fish card missing");
  expect(get(en, fish.detailKey)).toMatch(/free model/i);
});

it("every card detail resolves in all nine locales, translated", () => {
  for (const choice of ENGINE_CHOICES) {
    const enDetail = get(en, choice.detailKey);
    for (const [lang, catalog] of CATALOGS) {
      const value = get(catalog, choice.detailKey);
      expect(value.length, `${lang} ${choice.id}`).toBeGreaterThan(0);
      if (lang !== "en") expect(value, `${lang} ${choice.id}`).not.toBe(enDetail);
    }
  }
});

it("onboarding and page toast copy is dash-free in all nine locales", () => {
  for (const [lang, catalog] of CATALOGS) {
    const onboarding = subtree(catalog, "onboarding");
    for (const path of leafPaths(onboarding, "onboarding")) {
      expect(get(catalog, path), `${lang} ${path}`).not.toMatch(DASH);
    }
    for (const key of PAGE_TOAST_KEYS) {
      expect(get(catalog, key), `${lang} ${key}`).not.toMatch(DASH);
    }
  }
});

it("the onboarding page source stays dash-free and badge-free", () => {
  // Repo-root-relative path, matching captions.test.ts; vitest runs from the repo root.
  const source = readFileSync("src/routes/onboarding/+page.svelte", "utf8");
  expect(source).not.toMatch(DASH);
  expect(source).not.toContain("Free tier");
  expect(source).not.toContain("bg-amber-");
});
