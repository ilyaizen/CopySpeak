// Onboarding engine cards, in display order. Names are brand names and stay
// untranslated; every piece of copy renders through an i18n key so all nine
// locales ship the same card structure.
export type EngineChoice = {
  id: string;
  name: string;
  recommended: boolean;
  keyless: boolean;
  installable: boolean;
  detailKey: string;
};

export const ENGINE_CHOICES = [
  {
    id: "kokoro",
    name: "Kokoro",
    recommended: true,
    keyless: true,
    installable: true,
    detailKey: "onboarding.engines.kokoro.detail"
  },
  {
    id: "edge",
    name: "Edge-TTS",
    recommended: false,
    keyless: true,
    installable: false,
    detailKey: "onboarding.engines.edge.detail"
  },
  {
    id: "cartesia",
    name: "Cartesia",
    recommended: false,
    keyless: false,
    installable: false,
    detailKey: "onboarding.engines.cartesia.detail"
  },
  {
    id: "elevenlabs",
    name: "ElevenLabs",
    recommended: true,
    keyless: false,
    installable: false,
    detailKey: "onboarding.engines.elevenlabs.detail"
  },
  {
    id: "fish",
    name: "Fish Audio",
    recommended: true,
    keyless: false,
    installable: false,
    detailKey: "onboarding.engines.fish.detail"
  }
] satisfies EngineChoice[];
