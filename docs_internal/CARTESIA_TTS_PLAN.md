# Cartesia TTS Integration Plan

## Goal

Add Cartesia as a first-class TTS backend and make it the default TTS engine for new CopySpeak configs.

## Non-goals

- Do not hardcode API secrets in source, tests, docs, or committed config files.
- Do not redesign the engine/settings UI beyond the minimum needed to configure Cartesia.
- Do not add streaming playback unless required later; start with the existing `TtsBackend::synthesize` byte-returning flow.
- Do not refactor unrelated TTS engines.

## Assumptions to verify before implementation

- Cartesia supports a non-streaming bytes response suitable for the existing `TtsBackend` abstraction, or a streaming endpoint can be buffered into bytes.
- A default Cartesia voice/model can be selected from current Cartesia docs.
- The existing playback/history code can handle Cartesia's returned audio format via `file_extension()` like ElevenLabs/OpenAI.
- Existing user configs should continue to deserialize; only fresh defaults should switch to Cartesia.

## Proposed defaults

- `tts.active_backend`: `cartesia`
- `tts.cartesia.model_id`: use the current recommended TTS model from Cartesia docs.
- `tts.cartesia.voice_id`: use a stable default English voice from Cartesia docs.
- `tts.cartesia.output_format`: choose an existing app-supported format, preferably `wav` if Cartesia can return it; otherwise `mp3`.
- `tts.cartesia.api_key`: empty string in code defaults; users provide it through settings or local config.

## Implementation steps

1. Read current Cartesia API docs for:
   - text-to-speech endpoint
   - required headers
   - request/response schema
   - supported models, voices, and formats
   - credential validation endpoint, if available

2. Add backend config types in `src-tauri/src/config/tts.rs`:
   - Add `Cartesia` to `TtsEngine` with serde value `cartesia`.
   - Add `CartesiaConfig` with only needed fields: `api_key`, `model_id`, `voice_id`, and `output_format` if needed.
   - Add `cartesia: CartesiaConfig` to `TtsConfig`.
   - Change `TtsConfig::default().active_backend` to `TtsEngine::Cartesia`.
   - Keep validation minimal: require API key only when Cartesia is active if existing validation patterns support that.

3. Add `src-tauri/src/tts/cartesia.rs`:
   - Implement `TtsBackend` for `CartesiaTtsBackend`.
   - Build requests with `reqwest::blocking::Client`, matching existing OpenAI/ElevenLabs style.
   - Return audio bytes directly.
   - Override `file_extension()` if the response is not WAV.
   - Map provider errors to `TtsError::Http` with useful but non-secret messages.

4. Wire backend selection in Rust:
   - Export `pub mod cartesia;` from `src-tauri/src/tts/mod.rs`.
   - Update `create_backend`, `voice_for_backend`, `engine_identifier`, `voice_display_name`, and `engine_str` in `src-tauri/src/commands/tts/helpers.rs`.
   - Update `test_tts_engine` display logic in `src-tauri/src/commands/tts/health.rs`.
   - Add credential/voice IPC only if the settings UI needs it for the simple first version.

5. Update frontend types and settings UI:
   - Extend `TtsEngine` union in `src/lib/types.ts` with `cartesia`.
   - Add `CartesiaConfig` and `tts.cartesia` to `TtsConfig`.
   - Add a simple Cartesia engine panel under `src/lib/components/engine/`, modeled after OpenAI but smaller.
   - Add a Cartesia tab/card in `engine-page.svelte`.
   - Update footer engine switching/status labels where engines are enumerated.
   - Do not touch externally managed translation files.

6. Update tests where existing fixtures hardcode TTS config:
   - Add `cartesia` defaults to config builders/fixtures.
   - Update engine tab count/labels in engine-page tests.
   - Add focused tests for Cartesia UI only if equivalent OpenAI/ElevenLabs tests exist and can be copied with minimal changes.

7. Update docs/changelog:
   - Add an `[Unreleased]` changelog entry under `Added`/`Changed`.
   - Document Cartesia as supported TTS backend if README/docs list providers.
   - Do not include real API keys in documentation.

## Security notes

- Treat the provided Cartesia key as a local secret only.
- Do not write it into this plan, source code, tests, changelog, or docs.
- If a local manual test needs it, enter it through the app settings or an ignored local config file.
- Ensure logs never print request headers or full config payloads containing `api_key`.

## Verification plan

Ask for explicit confirmation before running any checks. Recommended verification after implementation:

1. `bun run check`
2. `bun run test`
3. Manual app smoke test with Cartesia selected/default:
   - fresh config starts on Cartesia
   - API key can be saved
   - test TTS succeeds
   - generated audio plays
   - history shows engine `cartesia` and expected voice display

## Risks / decisions needed

- Need to pick the exact Cartesia model, voice, and output format from current docs.
- If Cartesia only provides streaming, implementation will buffer the stream first to avoid changing playback architecture.
- If Cartesia voices require a list endpoint, decide whether to ship manual voice ID first or add voice listing immediately.
