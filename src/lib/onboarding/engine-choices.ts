// Onboarding engine cards, in display order. Names are brand names and stay
// untranslated; every piece of copy renders through an i18n key so all nine
// locales ship the same card structure.
export type EngineChoice = {
  // TtsEngine serde id. Onboarding passes it to complete_onboarding.
  id: string;
  name: string;
  keyless: boolean;
  // Installer engine id for local engines that download during onboarding.
  installer: string | null;
  // Voices the installer receives. The first one becomes the profile default.
  installVoices: string[];
  // Operating systems that show the card. null shows it everywhere.
  platforms: string[] | null;
  detailKey: string;
};

// Shared-model engine: one download covers every voice. Mirrors the default
// set the installer records in manifest.json (Settings path passes the same).
const KOKORO_VOICES = [
  "af_heart",
  "af_bella",
  "af_nicole",
  "af_sarah",
  "am_adam",
  "am_michael",
  "bf_emma",
  "bm_george"
];

export const ENGINE_CHOICES: EngineChoice[] = [
  {
    id: "kale",
    name: "Kale",
    keyless: true,
    installer: "kale",
    installVoices: [
      "Kenzie",
      "Simone",
      "Amy",
      "Frankie",
      "Darlene",
      "Owen",
      "Toby",
      "Jack",
      "Earl",
      "Magnus"
    ],
    // Windows until the .sh twin gets a clean-install pass on macOS/Linux.
    platforms: ["windows"],
    detailKey: "onboarding.engines.kale.detail"
  },
  {
    id: "tiny-voca",
    name: "tiny-voca",
    keyless: true,
    installer: "tiny-voca",
    installVoices: ["M2", "F2"],
    platforms: null,
    detailKey: "onboarding.engines.tinyvoca.detail"
  },
  {
    id: "kokoro",
    name: "Kokoro",
    keyless: true,
    installer: "kokoro",
    installVoices: KOKORO_VOICES,
    platforms: null,
    detailKey: "onboarding.engines.kokoro.detail"
  },
  {
    id: "edge",
    name: "Edge-TTS",
    keyless: true,
    installer: null,
    installVoices: [],
    platforms: null,
    detailKey: "onboarding.engines.edge.detail"
  },
  {
    id: "fish",
    name: "Fish Audio",
    keyless: false,
    installer: null,
    installVoices: [],
    platforms: null,
    detailKey: "onboarding.engines.fish.detail"
  }
];

// Fresh-install default: kale on Windows, tiny-voca elsewhere. Mirrors
// TtsEngine::default in src-tauri/src/config/tts.rs. On macOS/Linux tiny-voca
// stays the recommendation until install-kale.sh passes a clean-install check.
export function recommendedEngineId(platform: string): string {
  return platform === "windows" ? "kale" : "tiny-voca";
}

export function visibleEngineChoices(platform: string): EngineChoice[] {
  return ENGINE_CHOICES.filter(
    (choice) => choice.platforms === null || choice.platforms.includes(platform)
  );
}
