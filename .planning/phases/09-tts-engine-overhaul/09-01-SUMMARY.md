---
phase: 09-tts-engine-overhaul
plan: 01
subsystem: tts-backend
tags: [rust, tts, cleanup, credential-check, config-migration]
dependency_graph:
  requires: []
  provides:
    - TtsEngine enum with 3 variants (Local/OpenAI/ElevenLabs)
    - check_elevenlabs_credentials Tauri command
    - check_openai_credentials Tauri command
    - Http->Local config migration in load_or_default()
  affects:
    - src-tauri/src/config (AppConfig shape changed)
    - src-tauri/src/commands/tts (create_backend signature changed)
tech_stack:
  added: []
  patterns:
    - reqwest::blocking for sync Tauri credential check commands
    - serde_json::Value pre-parse for config migration detection
key_files:
  deleted:
    - src-tauri/src/tts/http.rs
  modified:
    - src-tauri/src/tts/mod.rs
    - src-tauri/src/config/tts.rs
    - src-tauri/src/config/mod.rs
    - src-tauri/src/commands/tts.rs
    - src-tauri/src/main.rs
    - CHANGELOG.md
decisions:
  - "Migration uses serde_json::Value pre-parse to detect old 'http' active_backend before AppConfig deserialization, since TtsEngine no longer has Http variant and would fail to deserialize"
  - "Credential check commands use reqwest::blocking (not async) to match the sync Tauri command pattern used throughout codebase"
  - "CredentialCheckResult added as a new distinct type (not reusing TtsHealthResult) because credential checks are a different surface — they return error_type for frontend routing but never do synthesis"
metrics:
  duration: 5min
  completed_date: "2026-03-09"
  tasks_completed: 2
  files_modified: 6
  files_deleted: 1
---

# Phase 9 Plan 01: Remove HTTP Engine and Add Credential Checks Summary

**One-liner:** HTTP TTS engine fully removed from Rust; two lightweight credential check commands added for ElevenLabs and OpenAI using reqwest blocking client.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Remove HTTP engine from Rust backend | f0e0178 | tts/mod.rs, config/tts.rs, config/mod.rs, commands/tts.rs |
| 2 | Add credential-only check commands | ad7e61f | commands/tts.rs, main.rs, CHANGELOG.md |

## What Was Built

### Task 1: HTTP Engine Removal

- Deleted `src-tauri/src/tts/http.rs` entirely (265 lines of dead code)
- `TtsEngine` enum reduced from 4 to 3 variants: `{ Local, OpenAI, ElevenLabs }`
- `TtsBackendType` enum removed (was marked `#[allow(dead_code)]` — truly dead)
- `HttpTtsConfig` struct and its `Default`/`validate` impls removed from `config/tts.rs`
- `AppConfig.http_tts: HttpTtsConfig` field removed from `config/mod.rs`
- `ValidationError::HttpUrlEmpty` and `HttpTimeoutTooSmall` variants removed
- `create_backend()` signature simplified: removed `http_tts_config` parameter
- All four synthesis functions (`test_tts_engine`, `speak_now`, `speak_queued`, `speak_history_entry`) updated to drop `http_tts_config` destructuring

**Config migration:** `load_or_default()` now pre-parses the raw JSON with `serde_json::Value` to detect the old `"active_backend": "http"` value before deserializing into `AppConfig`. If detected, `cfg.tts.active_backend` is reset to `TtsEngine::Local` with a `log::warn!`.

### Task 2: Credential Check Commands

Two new `#[tauri::command]` functions added to `commands/tts.rs`:

**`check_elevenlabs_credentials`** — hits `GET https://api.elevenlabs.io/v1/user` with `xi-api-key` header. Returns `CredentialCheckResult { success, message, error_type }`. No synthesis triggered.

**`check_openai_credentials`** — hits `GET https://api.openai.com/v1/models` with `Authorization: Bearer` header. Same result type.

Both handle:
- Empty API key → `error_type: "api_key_missing"`
- HTTP 401/403 → `error_type: "auth_failed"`
- Other HTTP status → `error_type: "http_error"`
- Network failure → `error_type: "http_error"`

Both registered in `main.rs` invoke_handler.

## Verification

- `src-tauri/src/tts/http.rs` does not exist: PASS
- `TtsEngine` enum has exactly 3 variants (Local, OpenAI, ElevenLabs): PASS
- `AppConfig` has no `http_tts` field: PASS
- Migration logic present in `load_or_default()`: PASS
- `check_elevenlabs_credentials` and `check_openai_credentials` registered in `main.rs`: PASS
- `cargo check` produces zero HTTP/TTS-related errors: PASS (remaining errors are pre-existing Windows API platform errors from building on Linux)

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `src-tauri/src/tts/http.rs` deleted
- [x] `f0e0178` commit exists
- [x] `ad7e61f` commit exists
- [x] `check_elevenlabs_credentials` in `commands/tts.rs` line 769
- [x] `check_openai_credentials` in `commands/tts.rs` line 819

## Self-Check: PASSED
