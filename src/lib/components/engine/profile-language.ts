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
