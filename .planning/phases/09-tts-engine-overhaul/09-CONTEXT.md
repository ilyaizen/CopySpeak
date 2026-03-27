# Phase 9: TTS Engine Overhaul - Context

**Gathered:** 2026-03-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Overhaul TTS engine support: consolidate CLI engines to piper/kokoro/qwen3-tts with static voice dropdowns, implement ElevenLabs voice fetch from API, add static OpenAI voice dropdown, remove HTTP engine entirely (hard delete), remove unsupported CLI presets, add two-stage credential + voice verification for API engines.

</domain>

<decisions>
## Implementation Decisions

### CLI Engine Consolidation
- Official presets: **piper**, **kokoro-tts**, **qwen3-tts** — these three only
- Remove all other CLI presets: chatterbox, coqui-tts, espeak, edge-tts — hard delete from UI
- qwen3-tts uses the same command + args_template pattern as piper/kokoro — no special config fields
- Install guidance section (inline `pip install` hints with copy button): **removed entirely**
- Voice selection: static dropdown per engine — no dynamic CLI fetch needed

### HTTP Engine Removal (Hard Delete)
- Remove HTTP engine everywhere: `TtsEngine` type changes from `"local" | "openai" | "elevenlabs" | "http"` → `"local" | "openai" | "elevenlabs"`
- Delete `src-tauri/src/tts/http.rs` entirely
- Delete `src/lib/components/engine/http-engine.svelte` entirely
- Delete `HttpTtsConfig` type from `src/lib/types.ts` and all Rust config structs
- Remove HTTP tab from engine-tabs UI
- **Migration**: on config load, if `active_backend == "http"`, reset to `"local"` and show a toast notification to the user

### Voice Selection UX
- **ElevenLabs**: Replace raw `voice_id` text field with a dropdown fetched from ElevenLabs API. Fetch user's voice library on page/section open. Implement ENG-06.
- **OpenAI**: Replace text input with static dropdown of hardcoded voices: alloy, ash, coral, echo, fable, nova, onyx, shimmer, verse
- **Piper**: Keep existing static dropdown (20 EN voices already hardcoded in local-engine.svelte) — no expansion to other languages
- **Voice "preview"**: Not a separate feature — users use the existing Test Engine button to hear a voice. No additional preview UI needed.

### API Health Check Enhancements
- Add **two-stage health check** for ElevenLabs and OpenAI:
  1. **Credential check** (fast, no synthesis): verify API key is valid with a lightweight API call
  2. **Voice verification**: confirm the configured voice ID (ElevenLabs) or voice name (OpenAI) is accepted/exists
- Health check is **manual button only** — never auto-run on page load (to avoid burning API credits)
- Error messages: keep current format — no additional troubleshooting steps beyond existing error strings
- No quota/subscription display for ElevenLabs

### Claude's Discretion
- Which ElevenLabs API endpoint to use for credential-only check (e.g., `/v1/user` or `/v1/voices`)
- How to structure the two-stage health check result in the UI (single result with stage info, or two separate indicators)
- Caching strategy for ElevenLabs voice list (per-session vs on-demand)
- Exact args_template for qwen3-tts preset (Claude picks reasonable defaults based on qwen3-tts docs)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/lib/components/engine/local-engine.svelte`: Already has `PIPER_EN_VOICES` static array (20 voices) and voice dropdown pattern. Reuse pattern for qwen3-tts preset section.
- `src/lib/components/engine/elevenlabs-engine.svelte`: Has `invoke("list_elevenlabs_voices")` call — already wired for voice fetching. Needs UI to switch from text input to dropdown using this data.
- `src/lib/components/engine/openai-engine.svelte`: Voice field is currently text input — swap for static dropdown.
- `src/lib/components/engine/engine-tabs.svelte`: HTTP tab to be removed. `TtsEngine` type reference to be updated.
- `src-tauri/src/tts/mod.rs`: `TtsBackend` trait + `Voice` struct already defined. HTTP module to be removed.
- `src-tauri/src/config/mod.rs`: Config structs include `HttpTtsConfig` — to be deleted.

### Files to Hard Delete
- `src-tauri/src/tts/http.rs`
- `src/lib/components/engine/http-engine.svelte`
- `src/lib/components/engine/http-engine.test.ts`

### Established Patterns
- Save bar pattern: `localConfig` / `originalConfig` with `hasChanges` derived — used in engine-tabs, carry forward
- Engine health check: `invoke("test_tts")` returns result with error type string — existing pattern, extend for two-stage
- Voice list fetch: `invoke("list_elevenlabs_voices")` already exists in Rust commands — wire to dropdown

### Integration Points
- `src-tauri/src/commands/tts.rs`: Add new Tauri commands for credential-only check and voice verification
- `src-tauri/src/config/tts.rs`: Remove HTTP config fields, remove `http` variant from `active_backend` enum
- `src/lib/types.ts`: Remove `"http"` from `TtsEngine` union, delete `HttpTtsConfig` interface
- `src/lib/services/tauri.ts` or `config load path`: Add migration logic for `active_backend == "http"` → reset to `"local"` + toast

</code_context>

<specifics>
## Specific Ideas

- The existing Test Engine button at the bottom of the engine config section is the voice "preview" — no separate preview button needed
- Migration toast should be specific: something like "HTTP engine has been removed. Switched to Local engine."
- The three official CLI engines (piper, kokoro, qwen3-tts) should be the only options in the CLI preset dropdown — the remove is a hard delete, not a hide

</specifics>

<deferred>
## Deferred Ideas

- ENG-07: Voice preview with sample audio clip — deferred (Test Engine button covers this use case)
- ElevenLabs quota / subscription tier display — explicitly out of scope for Phase 9
- Multi-language piper voice support — deferred to a later phase
- qwen3-tts special config for Ollama vs API endpoint modes — deferred; treat as plain CLI for now

</deferred>

---

*Phase: 09-tts-engine-overhaul*
*Context gathered: 2026-03-09*
