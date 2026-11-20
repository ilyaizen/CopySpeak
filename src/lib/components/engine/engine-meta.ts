// Single source of truth for engine *setup* metadata: credentials, installers,
// docs. Consumed by the Engines page. Synthesis capability (voices/options/
// supports_*) lives in the runtime catalog from `list_tts_engines` and is
// consumed by the profile editor — the two registries do not overlap.
//
// Per docs/profile-engine-settings.md: credentials + installers are global
// (machine/account); voice/model/knobs are profile-owned.

export type CredentialKind = "none" | "api_key" | "api_key_endpoint";
export type CredentialTarget =
  | "openai"
  | "elevenlabs"
  | "cartesia"
  | "fish"
  | "xai"
  | "mistral"
  | "google"
  | "microsoft";

// Panel state machines shared between engine-setup (orchestrator) and engine-panel (view).
export type TestState = "idle" | "testing" | "success" | "fail";

/**
 * How the install dialog presents voices for an installable engine:
 * - `per-voice`: each voice is a separate model download (piper)
 * - `shared`: one selected model download covers every listed voice (kitten, kokoro)
 * - `none`: nothing to pick — a binary install only (pocket)
 */
export type VoiceMode = "per-voice" | "shared" | "none";

export interface EngineSetupEntry {
  /** Unique id; also the i18n key suffix under `engine.<id>.title/description`. */
  id: string;
  /**
   * Short engine label, kept shorter than the full title so the table scans
   * fast ("Microsoft", not "Microsoft Foundry"). Brand names stay untranslated
   * (see onboarding/engine-choices.ts); falls back to `engine.<id>.title`.
   */
  menuLabel?: string;
  kind: "cloud" | "local";
  /** Passed to `install_engine`. Omitted for pure API-key engines. */
  installerId?: string;
  /** Required whenever `installerId` is set. */
  voiceMode?: VoiceMode;
  /** Approximate download size, shown in the install dialog header. */
  downloadSize?: string;
  credential: CredentialKind;
  credentialTarget?: CredentialTarget;
  /** i18n key under `engine.apiSetup.<placeholderKey>` for the API key placeholder. */
  placeholderKey?: string;
  docsUrl: string;
}

// Cloud engines: API-key (or none). Each can be tested via test_tts_engine_config.
export const CLOUD_ENGINES: EngineSetupEntry[] = [
  {
    id: "edge",
    menuLabel: "Edge",
    kind: "cloud",
    // Cloud endpoint, reached through the local pinned `edge-tts` client
    // (uv resolves `edge-tts==7.3.1` on demand — no app installer; a missing
    // uv binary surfaces the command in the synthesis error).
    credential: "none",
    docsUrl: "https://github.com/rany2/edge-tts"
  },
  {
    id: "openai",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "openai",
    placeholderKey: "placeholderOpenai",
    docsUrl: "https://platform.openai.com/docs/guides/text-to-speech"
  },
  {
    id: "elevenlabs",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "elevenlabs",
    placeholderKey: "placeholderElevenlabs",
    docsUrl: "https://elevenlabs.io/docs/api-reference/text-to-speech/convert"
  },
  {
    id: "fish",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "fish",
    placeholderKey: "placeholderFish",
    docsUrl: "https://docs.fish.audio/api-reference/endpoint/openapi-v1/text-to-speech"
  },
  {
    id: "cartesia",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "cartesia",
    placeholderKey: "placeholderCartesia",
    docsUrl: "https://docs.cartesia.ai/api-reference/tts/bytes"
  },
  {
    id: "xai",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "xai",
    placeholderKey: "placeholderXai",
    docsUrl: "https://docs.x.ai/developers/model-capabilities/audio/voice"
  },
  {
    id: "mistral",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "mistral",
    placeholderKey: "placeholderMistral",
    docsUrl: "https://docs.mistral.ai/studio/audio/text-to-speech/overview"
  },
  {
    id: "google",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "google",
    placeholderKey: "placeholderGoogle",
    docsUrl: "https://ai.google.dev/gemini-api/docs/speech-generation"
  },
  {
    id: "microsoft",
    menuLabel: "Microsoft",
    kind: "cloud",
    credential: "api_key_endpoint",
    credentialTarget: "microsoft",
    placeholderKey: "placeholderMicrosoft",
    docsUrl: "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech"
  }
];

// Local engines use the install dialog and one-shot synthesis tests. The first
// three (kale, tiny-voca, kokoro) are the local cards onboarding shows first.
export const LOCAL_ENGINES: EngineSetupEntry[] = [
  {
    id: "kale",
    menuLabel: "Kale",
    kind: "local",
    installerId: "kale",
    // Ten voices (US and British), all served by the one 15 MB int8 ONNX model.
    voiceMode: "shared",
    downloadSize: "~29 MB incl. G2P data, fetched at install",
    credential: "none",
    docsUrl: "https://huggingface.co/KaleComputer/kale-tts-1-nano"
  },
  {
    id: "tiny-voca",
    menuLabel: "tiny-voca",
    kind: "local",
    installerId: "tiny-voca",
    // Two voices (F2, M2), both served by the one 18 MB ONNX model.
    voiceMode: "shared",
    downloadSize: "~45 MB, fetched at install",
    credential: "none",
    docsUrl: "https://github.com/n4ze3m/tiny-voca"
  },
  {
    id: "kokoro",
    menuLabel: "Kokoro",
    kind: "local",
    installerId: "kokoro",
    voiceMode: "shared",
    downloadSize: "~335 MB shared model",
    credential: "none",
    docsUrl: "https://github.com/thewh1teagle/kokoro-onnx"
  },
  {
    id: "kitten",
    menuLabel: "Kitten",
    kind: "local",
    installerId: "kitten",
    voiceMode: "shared",
    downloadSize: "~25–80 MB selected model, fetched on first use",
    credential: "none",
    docsUrl: "https://github.com/KittenML/KittenTTS"
  },
  {
    id: "qwen",
    menuLabel: "Qwen3",
    kind: "local",
    installerId: "qwen",
    voiceMode: "shared",
    downloadSize: "~1.2 GB selected model, fetched on first use",
    credential: "none",
    docsUrl: "https://github.com/QwenLM/Qwen3-TTS"
  },
  {
    id: "piper",
    menuLabel: "Piper",
    kind: "local",
    installerId: "piper",
    voiceMode: "per-voice",
    downloadSize: "~20-100 MB / voice",
    credential: "none",
    docsUrl: "https://github.com/OHF-Voice/piper1-gpl"
  },
  {
    id: "pocket",
    menuLabel: "Pocket",
    kind: "local",
    installerId: "pocket",
    // 14 built-in voices, all served by the one 100M-parameter model.
    voiceMode: "shared",
    downloadSize: "~100M params, fetched on first use",
    credential: "none",
    docsUrl: "https://github.com/kyutai-labs/pocket-tts"
  }
];

// uv is NOT a setup entry: it is a prerequisite for every local installer.
// The Engines page shows a uv notice above the table while uv is missing.
// `install_engine` handles id "uv" via install-uv.ps1. uninstall_engine refuses it.

// Engines page order: local first, then cloud.
export const ALL_SETUP_ENTRIES: EngineSetupEntry[] = [...LOCAL_ENGINES, ...CLOUD_ENGINES];

export function findSetupEntry(id: string): EngineSetupEntry | undefined {
  return ALL_SETUP_ENTRIES.find((e) => e.id === id);
}
