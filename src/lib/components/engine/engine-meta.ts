// Single source of truth for engine *setup* metadata: credentials, installers,
// docs. Consumed by the Engines page. Synthesis capability (voices/options/
// supports_*) lives in the runtime catalog from `list_tts_engines` and is
// consumed by the profile editor — the two registries do not overlap.
//
// Per docs/profile-engine-settings.md: credentials + installers are global
// (machine/account); voice/model/knobs are profile-owned.

export type CredentialKind = "none" | "api_key" | "api_key_endpoint";
export type CredentialTarget = "openai" | "elevenlabs" | "cartesia" | "google" | "microsoft";

// Panel state machines shared between engine-setup (orchestrator) and engine-panel (view).
export type TestState = "idle" | "testing" | "success" | "fail";

/**
 * How the install dialog presents voices for an installable engine:
 * - `per-voice`: each voice is a separate model download (piper)
 * - `shared`: one model download covers every listed voice (kitten, kokoro)
 * - `none`: nothing to pick — a binary install only (uv, edge, pocket)
 */
export type VoiceMode = "per-voice" | "shared" | "none";

export interface EngineSetupEntry {
  /** Unique id; also the i18n key suffix under `engine.<id>.title/description`. */
  id: string;
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
    kind: "cloud",
    // Cloud endpoint, but it is reached through the local `edge-tts` CLI, so
    // it still needs installing (and can be uninstalled).
    installerId: "edge",
    voiceMode: "none",
    downloadSize: "~2 MB CLI, no model",
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
    id: "cartesia",
    kind: "cloud",
    credential: "api_key",
    credentialTarget: "cartesia",
    placeholderKey: "placeholderCartesia",
    docsUrl: "https://docs.cartesia.ai/api-reference/tts/bytes"
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
    kind: "cloud",
    credential: "api_key_endpoint",
    credentialTarget: "microsoft",
    placeholderKey: "placeholderMicrosoft",
    docsUrl: "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech"
  }
];

// Local engines: installed via a PowerShell installer (uv-based). The installer's
// own smoke test is the verification path — there is no runtime per-engine test.
export const LOCAL_ENGINES: EngineSetupEntry[] = [
  {
    id: "kitten",
    kind: "local",
    installerId: "kitten",
    voiceMode: "shared",
    downloadSize: "~25 MB shared model",
    credential: "none",
    docsUrl: "https://github.com/KittenML/KittenTTS"
  },
  {
    id: "piper",
    kind: "local",
    installerId: "piper",
    voiceMode: "per-voice",
    downloadSize: "~20-100 MB / voice",
    credential: "none",
    docsUrl: "https://github.com/OHF-Voice/piper1-gpl"
  },
  {
    id: "kokoro",
    kind: "local",
    installerId: "kokoro",
    voiceMode: "shared",
    downloadSize: "~335 MB shared model",
    credential: "none",
    docsUrl: "https://github.com/hexgrad/kokoro"
  },
  {
    id: "pocket",
    kind: "local",
    installerId: "pocket",
    // One built-in voice; nothing to choose.
    voiceMode: "none",
    downloadSize: "~10 MB CLI",
    credential: "none",
    docsUrl: "https://github.com/nickolas777/pocket-tts"
  }
];

// uv is the prerequisite for every local preset. Treated as a setup entry so it
// gets the same panel UI. `installerId: "uv"` is handled by install_engine.
export const UV_ENTRY: EngineSetupEntry = {
  id: "uv",
  kind: "local",
  installerId: "uv",
  voiceMode: "none",
  downloadSize: "~30 MB",
  credential: "none",
  docsUrl: "https://docs.astral.sh/uv/"
};

export const ALL_SETUP_ENTRIES: EngineSetupEntry[] = [...CLOUD_ENGINES, ...LOCAL_ENGINES, UV_ENTRY];

export function findSetupEntry(id: string): EngineSetupEntry | undefined {
  return ALL_SETUP_ENTRIES.find((e) => e.id === id);
}
