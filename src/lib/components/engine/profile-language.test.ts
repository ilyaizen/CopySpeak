import { readFileSync } from "node:fs";
import { describe, it, expect } from "vite-plus/test";
import { DETECTED_LANGUAGE_CODES, languageChoices, profileLanguage } from "./profile-language";

describe("DETECTED_LANGUAGE_CODES", () => {
  it("matches the codes the Rust detector returns", () => {
    // Runs from the package root (bun run test), so the path is root-relative.
    const rust = readFileSync("src-tauri/src/language.rs", "utf8");
    const codes = [...rust.matchAll(/=> "([a-z]{2})",/g)].map((match) => match[1]);
    expect([...DETECTED_LANGUAGE_CODES].sort()).toEqual([...new Set(codes)].sort());
  });
});

describe("languageChoices", () => {
  it("labels each code in the locale and sorts by label", () => {
    const choices = languageChoices("en");
    expect(choices.find((choice) => choice.value === "eo")?.label).toBe("Esperanto");
    const labels = choices.map((choice) => choice.label);
    expect(labels).toEqual([...labels].sort((a, b) => a.localeCompare(b, "en")));
  });

  it("keeps a saved value that is not a detector code selectable", () => {
    const choices = languageChoices("en", "pt-BR");
    expect(choices.some((choice) => choice.value === "pt-BR")).toBe(true);
  });

  it("labels a saved value that is not a valid language tag by its raw text", () => {
    const choices = languageChoices("en", "not a tag");
    expect(choices.find((choice) => choice.value === "not a tag")?.label).toBe("not a tag");
  });
});

describe("profileLanguage", () => {
  it("keeps the base subtag of a catalog voice language", () => {
    expect(profileLanguage("zh-Beijing")).toBe("zh");
    expect(profileLanguage("EN-us")).toBe("en");
  });

  it("maps Norwegian no to the detector's nb", () => {
    expect(profileLanguage("no")).toBe("nb");
  });

  it("returns null for a language the detector has no code for", () => {
    expect(profileLanguage("yue")).toBeNull();
  });

  it("returns null when the voice has no language", () => {
    expect(profileLanguage(undefined)).toBeNull();
    expect(profileLanguage(null)).toBeNull();
    expect(profileLanguage("")).toBeNull();
  });
});
