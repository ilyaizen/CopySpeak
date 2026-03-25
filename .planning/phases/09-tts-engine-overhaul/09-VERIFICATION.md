---
phase: 09-tts-engine-overhaul
verified: 2026-03-09T12:00:00Z
status: passed
score: 12/12 must-haves verified
re_verification:
  previous_status: gaps_found
  previous_score: 10/12
  gaps_closed:
    - "tts-settings.svelte fully cleaned of HTTP engine references (HTTP_PRESETS map deleted, http backend option removed from backendOptions, all localConfig.http_tts.* bindings deleted)"
    - "ENG-09-HTTP-REMOVE, ENG-09-CLI-CONSOLIDATE, ENG-09-TWO-STAGE, ENG-09-OPENAI-VOICE, and ENG-06 now defined and traceable in REQUIREMENTS.md with Phase 9 traceability rows"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Load engine page with HTTP backend configured in saved config.json"
    expected: "Engine tabs shows 3 tabs (Local/OpenAI/ElevenLabs); toast 'HTTP engine has been removed. Switched to Local engine.' appears once"
    why_human: "Requires Tauri app launch with modified config file; toast rendering is runtime UI behavior"
  - test: "Click 'Check API Key' in ElevenLabs section with a valid API key"
    expected: "Button shows 'Checking...' while in-flight; Alert appears with 'API key valid' message; Test Engine result cleared"
    why_human: "Requires live ElevenLabs API call"
  - test: "Click 'Check API Key' in OpenAI section with an invalid API key"
    expected: "Alert appears with 'API key check failed' / auth_failed message"
    why_human: "Requires live OpenAI API call with invalid credentials"
  - test: "Select kokoro-tts preset, then qwen3-tts preset, then custom preset in CLI engine"
    expected: "kokoro-tts shows 11 voice options (af_heart default); qwen3-tts shows 2 options (Chelsie default); custom shows free text input"
    why_human: "Requires interactive UI rendering to verify conditional voice field switching"
---

# Phase 9: TTS Engine Overhaul Verification Report

**Phase Goal:** Overhaul the TTS engine layer — remove the HTTP engine entirely, consolidate CLI presets, add cloud engine health checks, and clean up all related frontend/backend dead code.
**Verified:** 2026-03-09
**Status:** passed
**Re-verification:** Yes — after gap closure (Plans 09-04)

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HTTP engine fully removed from Rust — no TtsEngine::Http variant, no HttpTtsConfig, no http.rs | VERIFIED | `src-tauri/src/tts/http.rs` absent; `config/tts.rs` TtsEngine enum has 3 variants (Local/OpenAI/ElevenLabs) only |
| 2 | Config migration: active_backend == http migrated to local in Rust | VERIFIED | `load_or_default()` in `config/mod.rs` pre-parses JSON, detects old `"http"` value, resets to `TtsEngine::Local` |
| 3 | ElevenLabs credential check command exists: fast API key validation without synthesis | VERIFIED | `check_elevenlabs_credentials` in `commands/tts.rs`; calls `https://api.elevenlabs.io/v1/user`; registered in `main.rs` line 366 |
| 4 | OpenAI credential check command exists: fast API key validation without synthesis | VERIFIED | `check_openai_credentials` in `commands/tts.rs`; calls `https://api.openai.com/v1/models`; registered in `main.rs` line 367 |
| 5 | Frontend types cleaned — TtsEngine is 'local' \| 'openai' \| 'elevenlabs' only | VERIFIED | `src/lib/types.ts` line 45: 3-engine union type; HttpTtsConfig, TtsBackendType, http_tts field all absent |
| 6 | HTTP engine UI removed — engine-tabs has 3 tabs, http-engine files deleted | VERIFIED | `http-engine.svelte` and `http-engine.test.ts` absent; `engine-tabs.svelte` has no HttpEngine import; migration toast wired |
| 7 | CLI preset dropdown shows only piper, kokoro-tts, qwen3-tts, custom | VERIFIED | `local-engine.svelte` CLI_PRESETS has 3 entries; preset Select has 4 options; INSTALL_COMMANDS absent |
| 8 | kokoro-tts and qwen3-tts presets show static voice dropdowns | VERIFIED | KOKORO_VOICES (11 voices) and QWEN3_VOICES (2 voices) defined; conditional rendering at lines 204-219 |
| 9 | ElevenLabs shows two-stage health check — Check API Key + Test Engine | VERIFIED | `checkCredentials()` calls `check_elevenlabs_credentials`; `credCheckResult` state present; two buttons rendered |
| 10 | OpenAI shows two-stage health check — Check API Key + Test Engine | VERIFIED | `checkCredentials()` calls `check_openai_credentials`; `credCheckResult` state present; two buttons rendered |
| 11 | Health checks are manual button only — never auto-run on page load | VERIFIED | Neither `elevenlabs-engine.svelte` nor `openai-engine.svelte` use onMount or $effect to trigger credential/test checks |
| 12 | tts-settings.svelte fully cleaned of HTTP engine references | VERIFIED | `backendOptions` has 3 entries (local/openai/elevenlabs); no http_tts or HTTP_PRESETS references; zero grep hits for http_tts or HTTP_PRESETS |

**Score:** 12/12 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src-tauri/src/tts/http.rs` | DELETED — must not exist | VERIFIED | File absent |
| `src-tauri/src/config/tts.rs` | TtsEngine with Local/OpenAI/ElevenLabs only; HttpTtsConfig deleted | VERIFIED | 3-variant enum; no HttpTtsConfig |
| `src-tauri/src/commands/tts.rs` | check_elevenlabs_credentials and check_openai_credentials Tauri commands | VERIFIED | Both present with `#[tauri::command]`; handle empty key, 401/403, network errors |
| `src-tauri/src/config/mod.rs` | load_or_default migrates http->local; AppConfig without http_tts field | VERIFIED | Migration logic present; no http_tts field |
| `src/lib/types.ts` | Clean types — TtsEngine without http, AppConfig without http_tts | VERIFIED | 3-engine type; no HttpTtsConfig; no http_tts |
| `src/lib/components/engine/engine-tabs.svelte` | 3-tab UI with HTTP migration toast | VERIFIED | Migration guard + toast.info wired |
| `src/lib/components/engine/openai-engine.svelte` | Full 9-voice OpenAI dropdown | VERIFIED | All 9 voices: alloy, ash, coral, echo, fable, nova, onyx, shimmer, verse |
| `src/lib/components/engine/local-engine.svelte` | 3-preset CLI engine with per-preset voice dropdowns | VERIFIED | CLI_PRESETS has 3 entries; KOKORO_VOICES and QWEN3_VOICES arrays present |
| `src/lib/components/engine/elevenlabs-engine.svelte` | Two-stage health check + dropdown-first voice selection | VERIFIED | checkCredentials() wired; voice section is dropdown-only |
| `src/lib/components/engine/http-engine.svelte` | DELETED — must not exist | VERIFIED | File absent |
| `src/lib/components/engine/http-engine.test.ts` | DELETED — must not exist | VERIFIED | File absent |
| `src/lib/components/settings/tts-settings.svelte` | HTTP references purged — safe to re-import | VERIFIED | backendOptions has 3 entries; zero hits for http_tts or HTTP_PRESETS |
| `.planning/REQUIREMENTS.md` | All 5 Phase 9 requirement IDs defined and traceable | VERIFIED | ENG-09-* section added; 5 traceability rows present; ENG-06 marked complete |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `src-tauri/src/commands/tts.rs` | `https://api.elevenlabs.io/v1/user` | check_elevenlabs_credentials calls lightweight API | WIRED | Function exists; reqwest GET to ElevenLabs user endpoint |
| `src-tauri/src/commands/tts.rs` | `https://api.openai.com/v1/models` | check_openai_credentials calls lightweight API | WIRED | Function exists; reqwest GET with bearer auth |
| `src-tauri/src/config/mod.rs` | `src-tauri/src/config/tts.rs` | load_or_default migration resets Http variant to Local | WIRED | Migration logic references TtsEngine::Local via serde pre-parse |
| `engine-tabs.svelte` | `svelte-sonner` | toast.info() on HTTP migration detection | WIRED | toast.info("HTTP engine has been removed. Switched to Local engine.") |
| `elevenlabs-engine.svelte` | `invoke('check_elevenlabs_credentials')` | Stage 1 Check API Key button | WIRED | checkCredentials() invokes check_elevenlabs_credentials |
| `openai-engine.svelte` | `invoke('check_openai_credentials')` | Stage 1 Check API Key button | WIRED | checkCredentials() invokes check_openai_credentials |
| `local-engine.svelte` | `CLI_PRESETS` map | onchange applies preset command+args | WIRED | `const cfg = CLI_PRESETS[preset]` in onchange handler |
| `.planning/ROADMAP.md` | `.planning/REQUIREMENTS.md` | All ENG-09-* IDs defined and traceable | WIRED | ENG-09-HTTP-REMOVE, ENG-09-CLI-CONSOLIDATE, ENG-09-TWO-STAGE, ENG-09-OPENAI-VOICE each appear in definition section and traceability table |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| ENG-09-HTTP-REMOVE | 09-01, 09-02, 09-04 | HTTP engine removed from Rust + frontend + settings dead code | SATISFIED | http.rs deleted; TtsEngine has 3 variants; http-engine.svelte deleted; types.ts clean; tts-settings.svelte clean |
| ENG-09-CLI-CONSOLIDATE | 09-03 | CLI presets consolidated to piper/kokoro-tts/qwen3-tts | SATISFIED | CLI_PRESETS has 3 entries; chatterbox/coqui-tts/espeak/edge-tts hard-deleted; install guidance absent |
| ENG-09-TWO-STAGE | 09-01, 09-03 | Two-stage health checks for ElevenLabs + OpenAI | SATISFIED | check_elevenlabs/openai_credentials Tauri commands wired; UI has Check API Key + Test Engine buttons |
| ENG-09-OPENAI-VOICE | 09-02 | OpenAI complete 9-voice static dropdown | SATISFIED | openai-engine.svelte: alloy, ash, coral, echo, fable, nova, onyx, shimmer, verse |
| ENG-06 | 09-03 | ElevenLabs voice list from API (dropdown not raw ID) | SATISFIED | elevenlabs-engine.svelte: dropdown-only from list_elevenlabs_voices invoke; no text Input fallback |

All five requirement IDs are defined in REQUIREMENTS.md with Phase 9 traceability rows. ENG-06 marked complete.

---

## Anti-Patterns Found

None. The previously-flagged HTTP dead code in `tts-settings.svelte` was resolved by Plan 09-04.

---

## Human Verification Required

### 1. HTTP Migration Toast on Config Load

**Test:** Set `config.json` active_backend to `"http"`, launch app, navigate to Engine page
**Expected:** Engine tabs show Local engine selected; toast notification "HTTP engine has been removed. Switched to Local engine." appears once
**Why human:** Requires Tauri app launch with modified config file; toast rendering is runtime UI behavior

### 2. ElevenLabs Check API Key — Valid Key

**Test:** Enter a valid ElevenLabs API key, click "Check API Key"
**Expected:** Button shows "Checking..." while in-flight; Alert appears with "API key valid" message; Test Engine result cleared
**Why human:** Requires live ElevenLabs API call

### 3. OpenAI Check API Key — Invalid Key

**Test:** Enter an incorrect OpenAI API key, click "Check API Key"
**Expected:** Alert appears with "API key check failed" / auth error message
**Why human:** Requires live OpenAI API call with intentionally invalid credentials

### 4. CLI Preset Voice Dropdowns

**Test:** Select kokoro-tts preset, then qwen3-tts, then custom in CLI engine
**Expected:** kokoro-tts shows 11 kokoro voice options (af_heart default); qwen3-tts shows 2 options (Chelsie default); custom shows free text input
**Why human:** Requires interactive UI rendering to verify conditional voice field switching

---

## Re-Verification Summary

**Gaps closed (2/2):**

- Gap 1 resolved: `tts-settings.svelte` is clean. Commit `b437e4e` deleted the `HTTP_PRESETS` map, removed the `{ value: "http" }` backendOptions entry, and stripped all `localConfig.http_tts.*` bindings. `grep -c "http_tts|HTTP_PRESETS"` returns 0. The file can be safely re-imported without runtime errors.

- Gap 2 resolved: `REQUIREMENTS.md` now contains a dedicated "Engine Overhaul" section with all four ENG-09-* requirement definitions (commit `5c889e5`). The traceability table has all five Phase 9 rows (ENG-09-HTTP-REMOVE, ENG-09-CLI-CONSOLIDATE, ENG-09-TWO-STAGE, ENG-09-OPENAI-VOICE, ENG-06). ENG-06 is marked complete. Coverage count updated to 17.

**No regressions detected:**

All 10 previously-verified truths remain intact. Spot checks confirm http.rs absent, TtsEngine enum has 3 variants, both credential-check Tauri commands registered, HTTP frontend files absent, 9-voice OpenAI dropdown present, CLI_PRESETS has 3 entries, and two-stage health check UI wired in both cloud engine components.

---

*Verified: 2026-03-09*
*Verifier: Claude (gsd-verifier)*
