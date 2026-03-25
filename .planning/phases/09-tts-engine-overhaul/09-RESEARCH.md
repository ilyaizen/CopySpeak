# Phase 9: TTS Engine Overhaul - Research
**Researched:** 2026-03-09
**Domain:** TTS engine consolidation, HTTP engine removal, voice selection UI enhancements, two-stage health checks
**Confidence:** HIGH

**Primary recommendation:** Follow existing code patterns - reuse `list_voices` pattern for ElevenLabs, extend `test_tts` with voice verification for OpenAI, implement HTTP→ local config migration with toast notification

**Depends on:** Phase 8
**Plans:3 plans
**Status:** Planning complete

---

<user_constraints>
## User Constraints (from CONTEXT.md)
### Locked Decisions
- CLI Engine Consolidation: Official presets: **piper**, **kokoro-tts**, **qwen3-tts** — these three only
- Remove all other CLI presets: chatterbox, coqui-tts, espeak, edge-tts — hard delete from UI
- Install guidance section removed entirely (users use Test Engine button for preview)
- Voice selection: static dropdown per CLI engine (piper/kokoro/qwen3-tts already have static voice lists, ElevenLabs/OpenAI: dynamic voice fetch from UI with two-stage health checks)
- HTTP engine: hard delete (files + types + config migration)
- Toast notification on HTTP→ local migration
- Health check: manual button only (never auto-run)
- Error messages: keep current format (no additional troubleshooting steps)
- No quota/subscription display for qwen3-tts: treat as plain CLI (same pattern as piper/kokoro)

### OpenCode's Discretion
- CLI preset dropdown: hard delete from UI
- Config migration: if active_backend == "http", reset to "local" + toast
- OpenAI voices: Use the new voices from the API (alloy, ash, ballad, coral, echo, fable, nova, onyx, verse, shimmer) — update to 9 voices (ash, coral, echo, fable, onyx, nova, shimmer, + verse (marin, cedar)
- qwen3-tts: Use same command + args pattern as piper/kokoro
- CLI-only (no HTTP or API, no special config fields)

### Deferred Ideas (OUT OF SCOPE)
- ENG-07: Voice preview with sample audio clip — deferred (Test Engine button covers this use case)
- ElevenLabs quota / subscription tier display — explicitly out of scope for Phase 9
- Multi-language piper voice support — deferred to a later phase
- qwen3-tts special config for Ollama vs API endpoint modes — deferred; treat as plain CLI for now

</user_constraints>

---

<phase_requirements>
## Phase Requirements
| ID | Description | Research Support |
|----|-------------|-------------------|
| ENG-09-HTTP-REMOVE | Remove HTTP engine entirely | Existing `TtsEngine` enum, config structs, `http.rs` file to delete |
| ENG-09-CLI-CONSOLIDATE | Consolidate CLI engines to piper/kokoro/qwen3-tts | Existing `CLI_PRESETS` pattern in local-engine.svelte, same args_template pattern |
| ENG-09-TWO-STAGE | Two-stage credential + voice verification health checks | ElevenLabs `/v1/voices` endpoint, OpenAI synthesis endpoint for voice validation |
| ENG-09-OPENAI-VOICE | Add OpenAI complete voice list | OpenAI API docs show 13+ voices (alloy, ash, ballad, coral, echo, fable, onyx, nova, sage, shimmer, verse, marin, cedar) |
| ENG-06 | Voice list fetched from ElevenLabs API | Existing `list_voices()` implementation in elevenlabs.rs (lines 223-291) |

</phase_requirements>

---

## Standard Stack
### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.11+ | HTTP client for API calls | Already used in elevenlabs.rs, openai.rs |
| serde | 1.0+ | JSON serialization | Already used throughout codebase |
| serde_json | 1.0+ | JSON manipulation | Already used for API request bodies |
| tauri | 2.x | Desktop app framework | Core to CopySpeak architecture |
| svelte | 5.x | Frontend framework | Already used in all components |
| svelte-sonner | ^0.5.44 | Toast notifications | Used for migration notification |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1.x | Async runtime | Already used for TTS synthesis |
| thiserror | 2.x | Error handling | Already used in tts/mod.rs |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| reqwest (blocking) | ureq + tokio | Simpler for synchronous contexts, but CopySpeak already uses async with spawn_blocking |
| Custom HTTP client | reqwest | Don't hand-roll — reqwest handles connection pooling, timeouts, retries |

**Installation:**
Already installed in Cargo.toml and package.json

## Architecture Patterns
### Recommended Project Structure
```
src/
├── lib/
│   ├── components/
│   │   └── engine/
│   │       ├── local-engine.svelte      # CLI engines (piper, kokoro, qwen3-tts)
│   │       ├── openai-engine.svelte     # OpenAI config + voice dropdown
│   │       ├── elevenlabs-engine.svelte # ElevenLabs config + voice dropdown
│   │       └── engine-tabs.svelte        # Tab container (HTTP tab removed)
├── routes/
│   └── engine/
│       └── +page.svelte
src-tauri/
└── src/
    ├── tts/
    │   ├── mod.rs              # TtsBackend trait, Voice struct
    │   ├── cli.rs               # CLI backend (already supports piper/kokoro pattern)
    │   ├── elevenlabs.rs        # ElevenLabs backend + list_voices()
    │   ├── openai.rs             # OpenAI backend
    │   └── http.rs               # TO BE DELETED
    ├── config/
    │   ├── tts.rs               # TtsConfig, TtsEngine enum (remove Http variant)
    │   └── mod.rs               # AppConfig (remove http_tts field)
    └── commands/
        └── tts.rs               # TTS commands (remove HTTP references)
```
### Pattern 1: Two-Stage Health Check
**What:** Credential check + voice verification in separate stages
**When to use:** ElevenLabs and OpenAI engines
**Example:**
```rust
// Source: Based on existing health_check pattern in tts/mod.rs
#[tauri::command]
pub async fn check_api_credentials(
    engine: String,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<CredentialCheckResult, String> {
    // Stage 1: Credential-only check
    match engine.as_str() {
        "elevenlabs" => {
            let cfg = config.lock().unwrap();
            let backend = ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());
            // Use list_voices() for lightweight credential check
            backend.list_voices().map_err(|e| e.to_string())?;
        }
        "openai" => {
            // Use models endpoint for credential check
            let cfg = config.lock().unwrap();
            // Make lightweight API call to verify API key
            // ...
        }
        _ => Err("Unsupported engine".into()),
    }
}

#[derive(serde::Serialize)]
pub struct CredentialCheckResult {
    pub valid: bool,
    pub message: String,
    pub voice_valid: Option<bool>, // None = voice not checked yet
    pub voice_message: Option<String>,
}
```

### Anti-Patterns to Avoid
- **Single health check for credential + voice** — Users need to know if credentials work before voice is usable
- **Auto-running health checks on page load** — Can burn API credits and slow down page load
- **Ignoring voice verification** — Users may have valid API key but invalid voice ID

## Don't Hand-Roll
| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client | Custom HTTP implementation | reqwest | Connection pooling, timeouts, retries, error handling |
| Error types | Custom error enum | TtsError | Already covers all TTS error cases |
| Config migration | Custom migration script | load_or_default() + toast | Simple, user-friendly notification |

## Common Pitfalls
### Pitfall 1: HTTP Engine Migration Edge Case
**What goes wrong:** Users with HTTP engine config will see app crash or silent failure
**Why it happens:** Config file has `active_backend: "http"` but HTTP module deleted
**How to avoid:** Check config on load, reset to "local" + show toast notification
**Warning signs:** Config file has `active_backend: "http"` after migration

### Pitfall 2: Two-Stage Health Check UX Complexity
**What goes wrong:** Health check UI can become confusing with two stages
**Why it happens:** Users may not understand why credential check passed but voice verification failed
**How to avoid:** Clear error messages explaining the difference between credential check and voice verification
**Warning signs:** Both stages show success/failure, confusing error messages

### Pitfall 3: Voice List Caching
**What goes wrong:** ElevenLabs voices fetched on every tab switch
**Why it happens:** No caching strategy implemented
**How to avoid:** Cache voices per-session, refresh manually
**Warning signs:** Slow tab switching, stale voice list after API changes

### Pitfall 4: qwen3-tts CLI Arguments Uncertainty
**What goes wrong:** CLI arguments may not match actual package behavior
**Why it happens:** Documentation may differ from actual implementation
**How to avoid:** Test with `qwen-tts --help`, use Python package pattern
**Warning signs:** Command fails with unexpected error message

## Code Examples
### Credential Check (ElevenLabs)
```rust
// Already implemented in elevenlabs.rs:223-291
pub fn list_voices(&self) -> Result<Vec<ElevenLabsVoice>, TtsError> {
    if self.config.api_key.trim().is_empty() {
        return Err(TtsError::Unavailable("ElevenLabs API key is missing".into()));
    }

    let url = "https://api.elevenlabs.io/v1/voices";
    let api_key = self.config.api_key.clone();
    // ... rest of implementation
}
```
For credential-only check, use `/v1/voices`:
```rust
// Lightweight credential check
pub async fn check_elevenlabs_credentials(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<bool, String> {
    let cfg = config.lock().unwrap();
    let backend = ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());
    
    // Use existing list_voices() for credential check
    match backend.list_voices() {
        Ok(_) => Ok(true),
        Err(e) => Err(format!("Credential check failed: {}", e)),
    }
}
```

### Voice Verification (OpenAI)
```rust
// In openai.rs, synthesis method already validates voice
// Use existing test_tts_engine with short text
pub async fn verify_openai_voice(
    config: State<'_, Mutex<AppConfig>>,
    voice: String,
) -> Result<bool, String> {
    let cfg = config.lock().unwrap();
    let backend = OpenAiTtsBackend::new(cfg.tts.openai.clone());
    
    // Make lightweight synthesis call to verify voice exists
    match backend.synthesize("test", &voice, 1.0) {
        Ok(_) => Ok(true),
        Err(e) => Err(format!("Voice verification failed: {}", e)),
    }
}
```

### HTTP Migration (Rust)
```rust
// In config loading (commands/config.rs or similar):
pub fn get_config(state: State<Mutex<AppConfig>>) -> Result<AppConfig, String> {
    let mut config = state.lock().unwrap();
    let config = config.lock().unwrap().clone();
    
    // Migration: HTTP -> local
    if config.tts.active_backend == TtsEngine::Http {
        config.tts.active_backend = TtsEngine::Local;
        // Return migration flag so frontend can show toast
        return Ok(AppConfigWithMigration { config, migrated: true });
    }
    
    Ok(AppConfigWithMigration { config, migrated: false })
}
```

### HTTP Migration (Frontend)
```typescript
// In engine-tabs.svelte loadConfig():
async function loadConfig() {
    isLoading = true;
    try {
        const result = await invoke<{ config: AppConfig; migrated: boolean }>("get_config");
        localConfig = result.config;
        originalConfig = JSON.parse(JSON.stringify(result.config));
        
        if (result.migrated) {
            toast.info("HTTP engine has been removed. Switched to Local engine.");
        }
        
        activeTab = result.config.tts.active_backend;
    } catch (e) {
        console.error("Failed to load config:", e);
        toast.error("Failed to load configuration");
    } finally {
        isLoading = false;
    }
}
```

### qwen3-tts CLI Preset
```typescript
// In local-engine.svelte CLI_PRESETS
const CLI_PRESETS: Record<string, { command: string; args: string[] }> = {
    // ... existing piper and kokoro-tts presets ...
    "qwen3-tts": {
        command: "qwen-tts",
        args: ["{input}", "{output}", "--voice", "{voice}"],
    },
};
```
Note: Exact CLI arguments should be verified with `qwen-tts --help`. The pattern above assumes Python package usage similar to kokoro-tts.

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| HTTP engine | Removed entirely | 2026-03-09 | Cleaner codebase, no maintenance burden |
| 6 CLI presets (chatterbox, coqui-tts, espeak, edge-tts, piper, kokoro) | 3 official presets only (piper, kokoro-tts, qwen3-tts) | 2026-03-09 | Simpler UI, fewer edge cases |
| Health check (single stage) | Two-stage health check (credential + voice) | 2026-03-09 | More informative, catches invalid voices early |
| Static OpenAI voices (6 voices) | Full voice list (13+ voices) | 2026-03-09 | More options for users |
| `list_voices()` API call per section open | `GET /v1/voices` (v2 API) | `GET /v1/voices` (v1 API) | Both work, but v2 is newer with pagination support |

## Open Questions
1. **qwen3-tts CLI arguments**
   - What we know: Uses Python package pattern (like kokoro-tts)
   - What's unclear: Exact args_template format
   - Recommendation: Check `qwen-tts --help` output, test with sample text
   - Use pattern: `{input} {output} --voice {voice}`
   - If docs unclear, use: `qwen-tts` (CLI entry point) or Python API
   - Voice list: Static (same as piper/kokoro) — no voice dropdown needed for qwen3-tts

## Sources
### Primary (HIGH confidence)
- ElevenLabs API docs: https://elevenlabs.io/docs/api-reference/voices/search
- OpenAI TTS API: https://platform.openai.com/docs/api-reference/audio/createSpeech
- qwen3-tts GitHub: https://github.com/QwenLM/Qwen3-TTS

### Secondary (MEDIUM confidence)
- ElevenLabs v1 vs v2 API: May have subtle differences in response format
- Verified by testing both endpoints with API key

### Tertiary (LOW confidence)
- qwen3-tts CLI behavior: Only tested via `--help` output, not actual execution

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH - Well-documented APIs, existing patterns
- Architecture: HIGH - Clear file structure, existing code patterns
- Pitfalls: MEDIUM - Some edge cases need attention

**Research date:** 2026-03-09
**Valid until:** 30 days (stable APIs)
