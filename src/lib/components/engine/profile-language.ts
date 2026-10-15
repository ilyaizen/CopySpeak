import { choicesWithCurrent } from "./select-choices";

// Every ISO 639-1 code the Rust detector can return (src-tauri/src/language.rs,
// iso_639_1). A profile's language is picked from this list, so auto-switching
// can match it. The test reads the Rust file and fails when the two drift.
export const DETECTED_LANGUAGE_CODES: readonly string[] = [
  "af",
  "ak",
  "am",
  "ar",
  "az",
  "be",
  "bg",
  "bn",
  "ca",
  "cs",
  "cy",
  "da",
  "de",
  "el",
  "en",
  "eo",
  "es",
  "et",
  "fa",
  "fi",
  "fr",
  "gu",
  "he",
  "hi",
  "hr",
  "hu",
  "hy",
  "id",
  "it",
  "ja",
  "jv",
  "ka",
  "km",
  "kn",
  "ko",
  "la",
  "lt",
  "lv",
  "mk",
  "ml",
  "mr",
  "my",
  "nb",
  "ne",
  "nl",
  "or",
  "pa",
  "pl",
  "pt",
  "ro",
  "ru",
  "si",
  "sk",
  "sl",
  "sn",
  "sr",
  "sv",
  "ta",
  "te",
  "th",
  "tk",
  "tl",
  "tr",
  "uk",
  "ur",
  "uz",
  "vi",
  "yi",
  "zh",
  "zu"
];

/**
 * The detector code a catalog voice language maps to, or null when the voice
 * has none or the detector has no code for it. Mirrors `catalog_language_code`
 * in src-tauri/src/config/tts.rs: base subtag, lowercased, `no` as `nb`.
 */
export function profileLanguage(voiceLanguage: string | null | undefined): string | null {
  const base = voiceLanguage?.split(/[-_]/)[0].toLowerCase();
  if (!base) return null;
  const code = base === "no" ? "nb" : base;
  return DETECTED_LANGUAGE_CODES.includes(code) ? code : null;
}

export interface LanguageChoice {
  value: string;
  label: string;
}

/**
 * Detector codes labelled in `locale` and sorted by label. A saved `current`
 * value outside the list is kept, labelled by its raw text when it is not a
 * valid language tag, so it stays visible and selectable.
 */
export function languageChoices(locale: string, current?: string | null): LanguageChoice[] {
  const names = new Intl.DisplayNames([locale], { type: "language" });
  const labelFor = (code: string): string => {
    try {
      return names.of(code) ?? code;
    } catch {
      return code;
    }
  };
  return choicesWithCurrent(DETECTED_LANGUAGE_CODES, current)
    .map((code) => ({ value: code, label: labelFor(code) }))
    .sort((a, b) => a.label.localeCompare(b.label, locale));
}
