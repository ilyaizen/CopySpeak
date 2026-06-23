# CopySpeak Profile Integrations + Engine Settings Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Rework CopySpeak profiles into the primary way to select voice/engine behavior from the UI, control server, and CLI, while making engine-specific settings explicit, discoverable, and documented instead of hidden in ad-hoc global fields or custom CLI parameters.

**Architecture:** Keep credentials/global transport config outside shareable profiles. Make profiles own the non-secret synthesis intent: engine, voice, model, speed, pitch, output format, optional engine knobs, effects, and optional text post-processing. Add a small engine catalog layer that exposes human-readable engine names, docs links, voice labels, and supported options to both Rust and Svelte. Keep the existing localhost control server as the automation boundary; build a thin first-party CLI wrapper on top of it instead of inventing a second execution path.

**Tech Stack:** Tauri v2, Rust config/IPC/backend code, Svelte 5 runes UI, Vitest for frontend tests, Rust unit tests under `src-tauri`. Follow repo rule: do **not** run `bun check`, `bun test`, `cargo check`, or `cargo test` without explicit user confirmation.

---

## Non-negotiable product decisions

1. **Profiles are the user-facing abstraction.**
   - Users should say “use Pi voice”, “use Chatterbox narrator”, “use ElevenLabs Rachel”, not pass `--engine --voice --model --effect --post-process ...` every time.
   - CLI and HTTP may still accept one-off overrides for debugging, but the happy path is `profile`.

2. **Profiles must be shareable and safe.**
   - Profiles must not export API keys.
   - API keys, auth headers that contain secrets, and install paths that are machine-specific should live in global engine config unless there is a clear reason otherwise.

3. **Engine settings must be typed, not loose JSON in practice.**
   - Current `VoiceProfile.engine_options: serde_json::Value` exists but is unused. That is a smell.
   - Keep backward-compatible JSON migration, but internally expose typed per-engine profile options.

4. **The control server is already the integration spine.**
   - Current server: `src-tauri/src/control_server.rs` listens on `127.0.0.1:43117` or `COPYSPEAK_CONTROL_ADDR`.
   - Current endpoints: `GET /health`, `POST /speak`.
   - Current `POST /speak` accepts `{ text, engine?, effect?, profile? }` and mutates saved config when overrides are present.
   - New behavior should avoid surprise config mutation for one-off speaks unless the request explicitly asks to persist.

5. **KISS/YAGNI boundary.**
   - Do not build a plugin framework, generic schema language, visual workflow editor, or full OpenAPI server.
   - Build a typed catalog and profile resolution pipeline that covers the engines CopySpeak already has.

6. **Golden overruling-boundary.**
   - This is a complex task and will require much research. Do not shy away from looking for a default voice lists each engine offers, and documentation, for instance. Infer all the other best options for getting this done correctly.

---

## Current repo facts that drive the plan

### Existing profile model

Files:
- `src-tauri/src/config/tts.rs`
- `src-tauri/src/commands/tts/helpers.rs`
- `src/lib/components/engine/profile-manager.svelte`
- `src/lib/types.ts`

Current Rust profile shape:

```rust
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub engine: TtsEngine,
    pub voice: String,
    pub speed: f32,
    pub pitch: f32,
    pub effects: ProfileEffects,
    pub engine_options: serde_json::Value,
}
```

Current behavior:
- `migrate_tts_config()` creates one `default` profile from old global engine config.
- `resolve_effective()` treats `default` as legacy passthrough.
- Named profiles only resolve `engine`, `voice`, `speed`, `pitch`, `effects`.
- `engine_options` is not used by synthesis.
- `speed` is passed through to `TtsBackend::synthesize`, but some backends ignore it.
- `pitch` and profile `effects` are currently resolved but not meaningfully applied in the synthesis path.

### Existing engines

Files:
- `src-tauri/src/tts/mod.rs`
- `src-tauri/src/tts/cli.rs`
- `src-tauri/src/tts/http.rs`
- `src-tauri/src/tts/openai.rs`
- `src-tauri/src/tts/elevenlabs.rs`
- `src-tauri/src/tts/cartesia.rs`
- `src-tauri/src/tts/google.rs`
- `src-tauri/src/tts/microsoft.rs`

Existing `TtsEngine` values:
- `local`
- `http`
- `openai`
- `elevenlabs`
- `cartesia`
- `google`
- `microsoft`

Current notable limitations:
- OpenAI `synthesize()` ignores the `voice` argument and uses `self.config.voice` instead. That means a profile voice may not actually affect OpenAI.
- Cartesia uses the `voice` argument, but model/output format come from global config.
- Google uses the `voice` argument, but model/output format come from global config.
- Microsoft uses the `voice` argument, but endpoint/model/output format come from global config.
- HTTP uses `voice` and `speed`, but URL/body/headers/format are global config.
- Local CLI uses `voice`, but command/args/preset are global config.
- ElevenLabs uses `voice`, but model/output/voice settings come from global config.

That means the current “profile” is not actually a complete profile. It is a partial override glued to old global settings.

### Existing control server

File: `src-tauri/src/control_server.rs`

Current behavior:

```http
GET /health
POST /speak
Content-Type: application/json

{
  "text": "hello",
  "profile": "pi",
  "engine": "cartesia",
  "effect": "walkie_talkie"
}
```

Problem:
- If `profile`, `engine`, or `effect` is present, server mutates `cfg.tts.active_profile_id`, `cfg.tts.active_backend`, or global `cfg.effects`, then saves config.
- That is bad for automations. A Pi extension saying the last agent message should not silently change the user’s active desktop profile forever unless asked.

---

## Engine docs + option inventory

Use these as starting documentation links. During implementation, verify each against current provider docs and record findings in a checked-in markdown matrix.

| Engine                | Docs to verify                                                                                                    | Voices / names                                                                                                                            | Profile-owned synthesis options                                                                                                                                                      | Global-only options                                                  |
| --------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| Local CLI             | Existing wrappers: `kittentts-cli.py`, `scripts/chatterbox/copyspeak-chatterbox.py`, `src-tauri/src/tts/cli.rs`   | Depends on preset; Kitten voices include names like `Rosie`; Chatterbox voices are local `.wav` prompt names                              | `preset`, `command`, `args_template`, `voice`, `speed` if wrapper supports it, optional `model` if preset supports model                                                             | Installed engine directories if machine-specific                     |
| HTTP                  | `src-tauri/src/tts/http.rs`; target server docs per profile                                                       | User-provided `voice` string; optionally profile catalog labels                                                                           | `url_template`, `method`, `headers` without secrets, `body_template`, `voice`, `response_format`, `timeout_secs`, `speed`                                                            | Secret headers/API keys if any                                       |
| OpenAI                | `https://platform.openai.com/docs/guides/text-to-speech` and audio speech API reference                           | Built-in voices such as `alloy`, `ash`, `ballad`, `coral`, `echo`, `fable`, `nova`, `onyx`, `sage`, `shimmer`, `verse` after verification | `model`, `voice`, `speed`, `response_format`, likely `instructions` for newer models if supported                                                                                    | `api_key`                                                            |
| ElevenLabs            | `https://elevenlabs.io/docs/api-reference/text-to-speech/convert`, voices API                                     | Use `GET /v1/voices`; cache `voice_id`, `name`, labels, preview URL                                                                       | `voice_id`, `voice_name`, `model_id`, `output_format`, `stability`, `similarity_boost`, `style`, `use_speaker_boost`, possible text normalization / pronunciation knobs if supported | `api_key`                                                            |
| Cartesia              | `https://docs.cartesia.ai/api-reference/tts/bytes`, voices API if available                                       | Use provider voice list when possible; fallback built-ins like Katie / Jameson                                                            | `model_id`, `voice_id`, `voice_name`, `output_format.container`, encoding/sample-rate if exposed, speed/emotion if docs support                                                      | `api_key`                                                            |
| Google Gemini TTS     | `https://ai.google.dev/gemini-api/docs/speech-generation`                                                         | Prebuilt Gemini voice names such as `Kore`; verify full list                                                                              | `model`, `voice_name`, output format, optional speaking style prompt/instructions if docs recommend text prompt style rather than API knob                                           | `api_key`                                                            |
| Microsoft MAI / Azure | `https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech` and Azure AI Foundry MAI docs | Deployment-specific; may be user-entered                                                                                                  | `endpoint`, `model`, `voice_name`, `output_format`, auth mode if needed                                                                                                              | `api_key`; possibly endpoint if considered machine/deployment global |

Important: do not fake “human-readable voice names”. Build a mechanism:
- provider API when available,
- checked-in static fallback catalog for known built-ins,
- manual label field for local/http/custom voices.

---

## Target UX

### Profiles page

Route/component:
- Existing route: `src/routes/profiles/+page.svelte`
- Existing component: `src/lib/components/engine/profile-manager.svelte`

Target behavior:
- Left/select: list profiles by friendly name.
- Main editor sections:
  1. Profile identity: name, id/slug, description optional.
  2. Engine: provider select with human labels (`OpenAI`, `ElevenLabs`, `Local CLI`, etc.).
  3. Voice: voice picker with human names and metadata when known; manual entry fallback.
  4. Engine Settings: typed fields based on selected engine.
  5. Text Processing: optional profile-specific pre/post-processing settings.
  6. Audio Effects: walkie/gameboy/etc.
  7. Actions: duplicate, export, import, delete, “set active”, “test speak”.

### CLI UX

Preferred CLI should be boring:

```bash
copyspeak health
copyspeak speak --profile pi "The latest Hermes message"
copyspeak speak -p narrator --stdin
copyspeak profiles list
copyspeak profiles use pi
copyspeak profiles show pi
copyspeak profiles export pi --out pi.json
copyspeak profiles import pi.json
copyspeak engines list
copyspeak voices list --engine elevenlabs
```

Deliberately avoid this as the primary interface:

```bash
copyspeak speak --engine elevenlabs --voice 21m00Tcm4TlvDq8ikWAM --model eleven_turbo_v2_5 --stability 0.5 --similarity 0.75 --effect none "..."
```

One-off overrides may exist, but they should be sparse and explicit:

```bash
copyspeak speak --profile pi --set-active false "temporary speech"
copyspeak speak --profile pi --override voice=Rachel "debug only"
```

### HTTP automation API UX

Keep current simple endpoints, but add non-mutating semantics.

```http
POST /speak
{
  "text": "hello",
  "profile": "pi"
}
```

Default: uses profile for this request only; does **not** save active profile.

To persist active profile:

```http
POST /profiles/active
{ "profile": "pi" }
```

or:

```http
POST /speak
{
  "text": "hello",
  "profile": "pi",
  "persist_selection": true
}
```

Add read endpoints:

```http
GET /profiles
GET /profiles/{id}
GET /engines
GET /engines/{engine}/voices
```

Do not expose API keys in any HTTP response.

---

## Target data model

### Rust config model

Modify: `src-tauri/src/config/tts.rs`

Add schema v2. Keep old fields for migration and backward compatibility initially.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub engine: TtsEngine,
    pub voice: String,
    pub voice_label: Option<String>,
    pub speed: f32,
    pub pitch: f32,
    pub effects: ProfileEffects,
    pub text_processing: ProfileTextProcessing,
    pub engine_options: ProfileEngineOptions,
}
```

Add profile text processing:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileTextProcessing {
    pub mode: ProfileTextProcessingMode,
    pub strip_emote_brackets: bool,
    pub bracketed_emote_strategy: BracketedEmoteStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTextProcessingMode {
    InheritGlobal,
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BracketedEmoteStrategy {
    KeepLiteral,
    Strip,
    ConvertToSsmlOrInstruction,
}
```

Why this shape:
- `mode` avoids copying global post-processing config into every profile.
- bracket handling covers “emoted TTS text with brackets” without baking provider-specific fantasy into every engine.
- `ConvertToSsmlOrInstruction` should only be active for engines whose catalog says they support it; otherwise degrade to `Strip` or show a validation warning.

Add typed engine options. Keep this simple and explicit:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProfileEngineOptions {
    Local(LocalProfileOptions),
    Http(HttpProfileOptions),
    OpenAI(OpenAiProfileOptions),
    ElevenLabs(ElevenLabsProfileOptions),
    Cartesia(CartesiaProfileOptions),
    Google(GoogleProfileOptions),
    Microsoft(MicrosoftProfileOptions),
}
```

Suggested option structs:

```rust
pub struct LocalProfileOptions {
    pub preset: String,
    pub command: String,
    pub args_template: Vec<String>,
}

pub struct HttpProfileOptions {
    pub url_template: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body_template: Option<String>,
    pub response_format: String,
    pub timeout_secs: u64,
}

pub struct OpenAiProfileOptions {
    pub model: String,
    pub response_format: String,
    pub instructions: Option<String>,
}

pub struct ElevenLabsProfileOptions {
    pub model_id: String,
    pub output_format: crate::tts::elevenlabs::ElevenLabsOutputFormat,
    pub stability: f32,
    pub similarity_boost: f32,
    pub style: Option<f32>,
    pub use_speaker_boost: Option<bool>,
}

pub struct CartesiaProfileOptions {
    pub model_id: String,
    pub output_format: String,
    pub encoding: Option<String>,
    pub sample_rate: Option<u32>,
}

pub struct GoogleProfileOptions {
    pub model: String,
    pub output_format: String,
}

pub struct MicrosoftProfileOptions {
    pub endpoint: String,
    pub model: String,
    pub output_format: String,
}
```

Note: some fields may already exist globally. In v2, synthesis should prefer profile options. Keep global fields as account/defaults/migration fallback until the UI no longer needs them.

---

## Target engine catalog

Create: `src-tauri/src/tts/catalog.rs`

Purpose:
- centralize engine display names,
- expose docs URLs,
- expose supported fields,
- expose static voices where known,
- define whether voice refresh is supported,
- prevent UI and backend from each inventing labels.

Suggested structs:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct EngineCatalogEntry {
    pub engine: TtsEngine,
    pub label: String,
    pub description: String,
    pub docs_url: String,
    pub supports_voice_refresh: bool,
    pub supports_speed: bool,
    pub supports_pitch: bool,
    pub supports_bracket_emotes: bool,
    pub options: Vec<EngineOptionDescriptor>,
    pub voices: Vec<VoiceCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineOptionDescriptor {
    pub key: String,
    pub label: String,
    pub kind: EngineOptionKind,
    pub help: String,
    pub default_value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceCatalogEntry {
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub preview_url: Option<String>,
}
```

Expose IPC:
- `list_tts_engines() -> Vec<EngineCatalogEntry>`
- `list_tts_voices(engine: TtsEngine) -> Result<Vec<VoiceCatalogEntry>, String>`
- `refresh_tts_voices(engine: TtsEngine) -> Result<Vec<VoiceCatalogEntry>, String>` if credentials are needed.

Register in `src-tauri/src/main.rs`.

Mirror types in `src/lib/types.ts`.

---

## Effective request model

Modify: `src-tauri/src/commands/tts/helpers.rs`

Current `EffectiveTtsRequest` is too small. Expand it:

```rust
pub(crate) struct EffectiveTtsRequest {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub engine: TtsEngine,
    pub voice: String,
    pub voice_label: Option<String>,
    pub speed: f32,
    pub pitch: f32,
    pub effects: crate::config::ProfileEffects,
    pub text_processing: crate::config::ProfileTextProcessing,
    pub engine_options: crate::config::ProfileEngineOptions,
}
```

Add:

```rust
pub(crate) fn resolve_effective_for_profile(
    tts_config: &TtsConfig,
    profile_id: Option<&str>,
) -> Result<EffectiveTtsRequest, String>
```

Rules:
- `None` means active profile.
- `Some(id)` means request-local profile resolution.
- Unknown profile returns `Err("unknown profile: {id}")`.
- Default profile still works, but after v2 migration it should be a real profile rather than magical passthrough.
- No function in this module should save config. Resolution is pure.

---

## Backend construction target

Modify:
- `src-tauri/src/commands/tts/helpers.rs`
- `src-tauri/src/tts/openai.rs`
- `src-tauri/src/tts/elevenlabs.rs`
- `src-tauri/src/tts/cartesia.rs`
- `src-tauri/src/tts/google.rs`
- `src-tauri/src/tts/microsoft.rs`
- `src-tauri/src/tts/http.rs`
- `src-tauri/src/tts/cli.rs`

Add a conversion function:

```rust
pub(crate) fn create_backend_from_effective(
    eff: &EffectiveTtsRequest,
    tts_config: &TtsConfig,
) -> Box<dyn TtsBackend>
```

This function should merge:
- global secrets/account defaults from `tts_config.openai.api_key`, `tts_config.elevenlabs.api_key`, etc.
- profile non-secret synthesis options from `eff.engine_options`.

Specific fixes:
- OpenAI backend must use the `voice` parameter passed to `synthesize()` or the effective config, not always `self.config.voice`.
- OpenAI backend should support profile `response_format`; default `wav` for playback.
- OpenAI backend should include `instructions` only when set and only after docs confirm support for the selected model.
- ElevenLabs backend should use profile voice settings, not only global settings.
- Cartesia backend should use profile `model_id` and output format.
- Google backend should use profile model/output settings.
- Microsoft backend should use profile endpoint/model/output settings.
- HTTP backend should use profile URL/body/headers/timeout.
- Local backend should use profile preset/command/args.

Do not change the `TtsBackend` trait unless forced. Current trait is small and good.

---

## Text processing / bracketed emotes

Files:
- `src-tauri/src/post_processing.rs`
- `src-tauri/src/config/post_processing.rs`
- `src-tauri/src/sanitize/tts_normalize.rs`
- `src-tauri/src/commands/tts/synthesis.rs`

Current flow:

```rust
let post_processing_config = config.lock().unwrap().post_processing.clone();
let text = crate::post_processing::process_text(&post_processing_config, &text).await?;
```

Target flow:
1. Resolve effective profile before post-processing.
2. Apply profile text processing policy.
3. Apply global post-processing only if profile says `InheritGlobal` or `Enabled`.
4. Apply bracket handling before synthesis.

Add tests for:
- `[laughs] hello` with `KeepLiteral` remains unchanged.
- `[laughs] hello` with `Strip` becomes `hello` or `hello` with clean whitespace.
- `[laughs] hello` with unsupported `ConvertToSsmlOrInstruction` falls back predictably.

Do not make this an LLM-only feature. Bracket stripping must be deterministic and local.

---

## Control server plan

Modify: `src-tauri/src/control_server.rs`

### Request schema

```rust
struct SpeakRequest {
    text: String,
    profile: Option<String>,
    engine: Option<String>,
    effect: Option<String>,
    persist_selection: Option<bool>,
}
```

Behavior:
- If `profile` is set and `persist_selection != Some(true)`, speak with that profile for this request only.
- If `persist_selection == Some(true)`, update `active_profile_id` and save config.
- Keep `engine`/`effect` backward-compatible, but mark them as debug shorthands in comments and docs.
- Do not mutate global config for one-off `engine`/`effect` unless `persist_selection` is true.

Add endpoints:
- `GET /profiles`
- `GET /profiles/{id}`
- `POST /profiles/active`
- `GET /engines`
- `GET /engines/{engine}/voices`

KISS parser note:
- The current server manually parses HTTP. Keep it if endpoints remain simple.
- If route parsing gets ugly, add tiny helper functions in the same file. Do not bring in Axum just for five localhost endpoints unless manual parsing becomes unmaintainable.

---

## CLI plan

Create one thin CLI first. Do not overbuild.

Recommended first implementation:
- Create: `scripts/copyspeak.mjs`
- Optional Windows shim later: `scripts/copyspeak.ps1`

Why script first:
- It talks to the already-running localhost control server.
- It avoids fighting Tauri single-instance process args.
- It can be packaged later as a sidecar if the UX proves worth it.

Commands:

```bash
node scripts/copyspeak.mjs health
node scripts/copyspeak.mjs speak --profile pi "hello"
node scripts/copyspeak.mjs speak -p pi --stdin
node scripts/copyspeak.mjs profiles list
node scripts/copyspeak.mjs profiles use pi
node scripts/copyspeak.mjs profiles show pi
node scripts/copyspeak.mjs engines list
node scripts/copyspeak.mjs voices list --engine elevenlabs
```

Implementation details:
- Read `COPYSPEAK_CONTROL_ADDR`, default `http://127.0.0.1:43117`.
- Use Node 18+ global `fetch`.
- For `--stdin`, read all stdin.
- Return non-zero exit codes on non-2xx responses.
- Print JSON for `show`, `engines`, `voices`; print compact text table for `profiles list`.

Do **not** add a dependency like `commander` unless argument parsing becomes painful. This CLI is tiny.

---

## Detailed task breakdown

### Task 1: Write engine/profile requirements doc

**Objective:** Create a checked-in source of truth before touching schema.

**Files:**
- Create: `docs/profile-engine-settings.md`

**Content to include:**
- Motivation: profiles replace long custom CLI parameter lists.
- Safety: profiles export without secrets.
- Engine matrix from this plan.
- Current docs URLs and TODO checkboxes for verification.
- Non-mutating HTTP/CLI semantics.

**Validation:**
- Read the doc and verify it answers: “what belongs in a profile vs global config?”

### Task 2: Add engine catalog types and static entries

**Objective:** Create shared backend metadata for engines, docs, options, and static voices.

**Files:**
- Create: `src-tauri/src/tts/catalog.rs`
- Modify: `src-tauri/src/tts/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/types.ts`

**Implementation notes:**
- Include all seven existing engines.
- Add docs URLs.
- Add static voice fallbacks for OpenAI, Google, Cartesia defaults, ElevenLabs defaults.
- Mark refresh support true only where implemented or easy to add now: ElevenLabs first.

**Tests:**
- Add Rust unit test in `catalog.rs` verifying every `TtsEngine` has exactly one catalog entry.
- Add Rust unit test verifying no catalog entry has an empty label or docs URL.

### Task 3: Add IPC commands for engine catalog and voices

**Objective:** Let frontend/CLI/control server query engine metadata without duplicating constants.

**Files:**
- Modify: `src-tauri/src/commands/tts/voices.rs`
- Modify: `src-tauri/src/commands/tts/mod.rs`
- Modify: `src-tauri/src/main.rs`

**Commands:**
- `list_tts_engines() -> Vec<EngineCatalogEntry>`
- `list_tts_voices(engine: TtsEngine) -> Result<Vec<VoiceCatalogEntry>, String>`

**Notes:**
- For ElevenLabs, adapt current `list_elevenlabs_voices()` response into `VoiceCatalogEntry`.
- Keep `list_elevenlabs_voices()` for backward compatibility until UI is migrated.

### Task 4: Define schema v2 profile option structs

**Objective:** Replace loose profile `engine_options` behavior with typed options while keeping deserialization compatible.

**Files:**
- Modify: `src-tauri/src/config/tts.rs`
- Modify: `src/lib/types.ts`
- Modify: `src-tauri/src/config/tests.rs`

**Steps:**
1. Add `ProfileTextProcessing`, enums, and defaults.
2. Add `ProfileEngineOptions` enum and per-engine option structs.
3. Add defaults for every option struct from current global config defaults.
4. Bump profile/TTS schema version to 2.
5. Keep migration from schema 0/1 to 2.
6. Preserve old JSON with `engine_options: {}` by converting it based on `profile.engine`.

**Tests:**
- Legacy v0 no profiles → v2 default profile.
- Existing v1 profile with empty `engine_options` → v2 typed defaults for its engine.
- Existing named profile voice survives migration.
- Export/import profile JSON roundtrip does not include credentials.

### Task 5: Make default profile real, not magical passthrough

**Objective:** Remove the special behavior where `default` silently means “read legacy globals”.

**Files:**
- Modify: `src-tauri/src/commands/tts/helpers.rs`
- Modify: `src-tauri/src/config/tts.rs`
- Modify: `src-tauri/src/config/tests.rs`

**Rules:**
- `default` can remain undeletable in UI, but it should be a normal `VoiceProfile` after migration.
- Synthesis should use `resolve_effective_for_profile(config, None)` for active profile.
- No profile resolution should mutate config.

**Tests:**
- Active `default` profile with OpenAI voice `nova` resolves to OpenAI/nova.
- Active named profile with Cartesia resolves to Cartesia profile options.
- Unknown profile ID returns clear error.

### Task 6: Update backend construction to use effective profile options

**Objective:** Ensure profiles actually control engine-specific settings.

**Files:**
- Modify: `src-tauri/src/commands/tts/helpers.rs`
- Modify: every backend file listed in “Backend construction target”.

**High-priority bug fix:**
- Fix `src-tauri/src/tts/openai.rs` so `synthesize(&self, text, voice, speed)` uses the `voice` argument or effective config, not `self.config.voice` unconditionally.

**Tests:**
- Unit-test config merging where possible without network.
- For HTTP backend, existing placeholder test should be extended with profile URL/body/speed.
- For OpenAI, add a request-body construction helper and unit-test model/voice/speed/format/instructions without sending network requests.

### Task 7: Apply profile text processing before synthesis

**Objective:** Make bracketed emote handling and global post-processing predictable per profile.

**Files:**
- Modify: `src-tauri/src/commands/tts/synthesis.rs`
- Modify: `src-tauri/src/post_processing.rs` or create helper in `src-tauri/src/sanitize/tts_normalize.rs`
- Modify: `src-tauri/src/config/tests.rs` or relevant sanitize tests

**Implementation:**
- Resolve effective profile before `process_text()`.
- Add deterministic bracket handling helper.
- Honor profile `text_processing.mode`.

**Tests:**
- Bracket strip.
- Bracket keep.
- Global post-processing inherited.
- Profile disabled post-processing skips global post-processing.

### Task 8: Rework Profiles UI around engine catalog

**Objective:** Stop the UI from hardcoding lowercase engine names and raw voice IDs only.

**Files:**
- Modify: `src/lib/components/engine/profile-manager.svelte`
- Possibly create: `src/lib/components/profiles/engine-options-editor.svelte`
- Possibly create: `src/lib/components/profiles/voice-picker.svelte`
- Modify: `src/lib/types.ts`
- Modify tests under `src/lib/components/engine/*.test.ts` or move tests with new components.

**UI requirements:**
- Engine select uses catalog labels.
- Voice field becomes picker + manual fallback.
- Engine settings render based on selected engine.
- Profile export strips secrets.
- Profile import validates schema v2.
- Show docs link for selected engine.

**Svelte 5 rules:**
- Use `$state`, `$derived`, `$props`, `$effect`.
- Use `onclick`, not `on:click`.
- Avoid `$effect` to write user slider changes back to config; use explicit `onchange` to avoid cancel/reset race.

### Task 9: Update control server to be profile-first and non-mutating by default

**Objective:** Make integrations like Pi voice safe and sane.

**Files:**
- Modify: `src-tauri/src/control_server.rs`
- Modify: `src-tauri/src/commands/tts/synthesis.rs` if `speak_now` needs profile override param

**Implementation approach:**
- Add an internal `speak_with_options(app, text, profile_id_override)` or extend `speak_now` carefully.
- Avoid saving config for one-off profile speaks.
- Add JSON endpoints listed above.

**Tests:**
- Add parser tests if current file has test module or create one.
- Verify `POST /speak` with profile does not call config save unless `persist_selection` is true. If hard to unit-test save calls, factor mutation into pure function and test that.

### Task 10: Add first-party CLI wrapper

**Objective:** Give CopySpeak CLI abilities without long custom parameter lists.

**Files:**
- Create: `scripts/copyspeak.mjs`
- Optional: `scripts/copyspeak.ps1`
- Modify: `README.md` or `docs/profile-engine-settings.md`

**Commands:**
- `health`
- `speak --profile/-p <id> [text]`
- `speak --stdin --profile/-p <id>`
- `profiles list`
- `profiles use <id>`
- `profiles show <id>`
- `engines list`
- `voices list --engine <engine>`

**Tests:**
- If no HTTP server is running, CLI exits non-zero with “CopySpeak control server is not reachable at ...”.
- Use a tiny mocked HTTP server in a Node test only if existing test infra makes that cheap. Otherwise keep CLI simple and manually verify with running app after user approval.

### Task 11: Add profile-aware docs for integrations

**Objective:** Make Pi/Hermes/skill speech integrations obvious.

**Files:**
- Modify: `README.md`
- Modify or create: `docs/integrations.md`
- Update existing: `COPYSPEAK_PI_VOICE.md`, `COPYSPEAK_PI_VOICE_2.md` if still current.

**Include examples:**

```bash
node scripts/copyspeak.mjs speak --profile pi --stdin
```

```http
POST http://127.0.0.1:43117/speak
Content-Type: application/json

{ "profile": "pi", "text": "Last agent message..." }
```

Warn clearly:
- server is localhost trusted automation, not a public network API,
- do not bind to `0.0.0.0` unless you add auth first.

### Task 12: Clean up legacy paths after migration is stable

**Objective:** Reduce duplication without breaking users.

**Files:**
- `src-tauri/src/config/tts.rs`
- `src/lib/components/engine/engine-page.svelte`
- old engine-specific UI components if profiles page supersedes them

**Rule:**
- Do this only after profile synthesis and UI pass tests.
- Keep old global config fields in serialized config for at least one release if removing them risks user data loss.
- Prefer hiding old UI fields over deleting persisted fields immediately.

---

## Validation plan

Because repo instructions say not to run checks without confirmation, implementation should ask before executing these. When approved, run in this order:

```bash
# Frontend focused tests
bun run test profile-manager
bun run test engine-page

# Rust focused tests
cd src-tauri && cargo test config::tests
cd src-tauri && cargo test tts::catalog
cd src-tauri && cargo test tts::http
cd src-tauri && cargo test commands::tts

# Broader checks, only after focused tests pass
bun run test
cd src-tauri && cargo test
bun run check
```

Manual verification with running app:
1. Create profile `pi` using HTTP/local engine.
2. Create profile `narrator` using a cloud engine.
3. Switch active profile in UI; speak clipboard.
4. Run CLI `health`.
5. Run CLI `speak --profile pi "hello"`.
6. Call HTTP `POST /speak` with `profile: pi`; verify active profile in UI did not change.
7. Call `POST /profiles/active`; verify active profile changed.
8. Export profile; verify no API key or secret header is present.
9. Import profile; verify voice label and engine settings survive.
10. Test bracketed emote modes with text like `[laughs] hello there`.

---

## Risks and tradeoffs

1. **Schema churn risk.**
   - Mitigation: schema v2 migration tests before UI work.

2. **Provider docs drift.**
   - Mitigation: catalog stores docs URLs and static fallbacks; provider API refresh wins when available.

3. **Profiles becoming too powerful.**
   - Mitigation: keep credentials global; no workflow/plugin system; no generic templating beyond existing HTTP/local templates.

4. **HTTP server scope creep.**
   - Mitigation: localhost only, simple endpoints, no auth unless binding beyond localhost is added.

5. **CLI packaging ambiguity.**
   - Mitigation: start with `scripts/copyspeak.mjs`; package later only if it proves useful.

6. **Engine option mismatch.**
   - Mitigation: typed options plus catalog descriptors; unsupported fields disabled in UI.

---

## Open questions for Ilya

1. Should local/HTTP profiles be allowed to include machine-specific paths and URL templates in exported JSON, or should export have a “portable” mode that strips those too?
2. For Pi voice, should `POST /speak` queue behind current speech, interrupt it, or follow global playback retrigger mode?
3. Should “set active profile” be global app state only, or should integrations be able to have named default profiles per caller later? Default recommendation: global only; per-caller defaults are YAGNI.
4. Should bracketed emotes be mostly stripped, or should certain engines get an “instruction prompt” transformation? Default recommendation: strip by default; provider-specific emotional prompting can be opt-in later.

---

## Closed answers for Ilya

1. a “portable” mode only?
2. follow global playback retrigger mode or have an option to interrupt as passed parameters also? Infer.
3. global only.
4. no, we already do that with the sanitation steps etc.

---

## Default implementation order

Do it in this order. Do not start with UI polish.

1. Documentation matrix.
2. Engine catalog.
3. Schema v2 + migration tests.
4. Effective request resolution.
5. Backend option merging and OpenAI voice bug fix.
6. Profile text processing.
7. Control server non-mutating profile requests.
8. CLI wrapper.
9. Profiles UI rewrite.
10. Docs/readme/manual verification.

This order keeps the abstraction honest: data model and synthesis behavior first, pretty controls second. Otherwise we get a nice profile screen that still lies about what it controls.
