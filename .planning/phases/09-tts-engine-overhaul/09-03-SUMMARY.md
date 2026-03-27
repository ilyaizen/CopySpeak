---
phase: 09-tts-engine-overhaul
plan: "03"
subsystem: frontend/engine-ui
tags: [svelte, tts, cli-presets, voice-dropdown, health-check, credential-check]
dependency_graph:
  requires:
    - 09-01 (check_elevenlabs_credentials, check_openai_credentials Tauri commands)
    - 09-02 (TtsEngine type cleanup, engine tabs UI)
  provides:
    - "3-preset CLI engine UI (piper/kokoro-tts/qwen3-tts + custom) with per-preset voice dropdowns"
    - "Two-stage health check UI for ElevenLabs (Check API Key + Test Engine)"
    - "Two-stage health check UI for OpenAI (Check API Key + Test Engine)"
    - "Dropdown-only voice selection for ElevenLabs (no text input fallback)"
  affects:
    - src/lib/components/engine/local-engine.svelte
    - src/lib/components/engine/elevenlabs-engine.svelte
    - src/lib/components/engine/openai-engine.svelte
tech_stack:
  added: []
  patterns:
    - "Per-preset voice dropdown pattern: piper/kokoro-tts/qwen3-tts each have static voice lists"
    - "Two-stage health check pattern: credential check (no synthesis) then full synthesis test"
key_files:
  created: []
  modified:
    - src/lib/components/engine/local-engine.svelte
    - src/lib/components/engine/elevenlabs-engine.svelte
    - src/lib/components/engine/openai-engine.svelte
    - src/lib/components/engine/local-engine.test.ts
    - src/lib/components/engine/elevenlabs-engine.test.ts
    - src/lib/components/engine/openai-engine.test.ts
decisions:
  - "[09-03] CLI_PRESETS reduced to 3 official entries (kokoro-tts, piper, qwen3-tts) — chatterbox/coqui-tts/espeak/edge-tts hard deleted"
  - "[09-03] INSTALL_COMMANDS map and install guidance UI section removed entirely per user decision"
  - "[09-03] ElevenLabs voice section is dropdown-only — no text input fallback; empty state shows guidance text"
  - "[09-03] Two-stage health check: Check API Key resets testResult to null (fresh slate); Test Engine resets credCheckResult to null"
metrics:
  duration: "~15min"
  completed_date: "2026-03-09"
  tasks_completed: 2
  files_modified: 6
---

# Phase 9 Plan 03: CLI Preset Overhaul and Two-Stage Health Checks Summary

**One-liner:** CLI engine UI reduced to 3 official presets (piper/kokoro-tts/qwen3-tts) with per-preset voice dropdowns; ElevenLabs and OpenAI gain two-stage health checks (credential validation + synthesis test).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Overhaul local-engine — 3 official presets with voice dropdowns | 4704bf2 | local-engine.svelte, local-engine.test.ts |
| 2 | Two-stage health check UI for ElevenLabs and OpenAI | 2d71821 | elevenlabs-engine.svelte, openai-engine.svelte, 3 test files |

## What Was Built

### Task 1: Local Engine Overhaul (commit: 4704bf2)

**`CLI_PRESETS` map reduced from 6 entries to 3:**
- Kept: `kokoro-tts`, `piper`, `qwen3-tts`
- Hard deleted: `chatterbox`, `coqui-tts`, `espeak`, `edge-tts`

**New voice lists added:**
- `KOKORO_VOICES` — 11 voices (af_heart default through bm_lewis)
- `QWEN3_VOICES` — 2 voices (Chelsie default, Ethan)

**Preset Select updated to 4 options:** piper (default), kokoro-tts, qwen3-tts, custom

**Voice field now per-preset:**
- `piper` → existing `PIPER_EN_VOICES` dropdown (unchanged)
- `kokoro-tts` → `KOKORO_VOICES` dropdown
- `qwen3-tts` → `QWEN3_VOICES` dropdown
- `custom` → free text Input (unchanged behavior)

**Removed:**
- `INSTALL_COMMANDS` map (entire pip install guidance section)
- `getInstallCommand()` function
- `copyInstallCommand()` function
- `ERROR_MESSAGES` dead constant
- `Copy` icon import (no longer needed)

### Task 2: Two-Stage Health Checks (commit: 2d71821)

**ElevenLabs engine changes:**
- Added `credCheckResult` and `isCheckingCreds` state variables
- Added `checkCredentials()` → calls `invoke("check_elevenlabs_credentials")`
- Replaced single "Test Engine" button with two-button layout: "Check API Key" + "Test Engine"
- Each button shows its own Alert result (credCheckResult / testResult) independently
- Buttons mutually disable each other while the other is running
- Voice section: removed `Input` text fallback — now shows either `Select` dropdown, error message with guidance, or "Enter your API key" prompt
- Removed `ERROR_MESSAGES` dead constant

**OpenAI engine changes:**
- Added `credCheckResult` and `isCheckingCreds` state variables
- Added `checkCredentials()` → calls `invoke("check_openai_credentials")`
- Replaced single "Test Engine" button with same two-button layout
- Removed `ERROR_MESSAGES` dead constant

**Test fixes (Rule 1 — auto-fixed):**
- `local-engine.test.ts`: Removed 8 ENG-05 tests for install card (section removed); replaced with single negative assertion; fixed ENG-03 success test to use `getAllByText` (AlertTitle + AlertDescription both show message)
- `elevenlabs-engine.test.ts`: Fixed ENG-03 success test to use `getAllByText`
- `openai-engine.test.ts`: Fixed ENG-03 success test to use `getAllByText`

## Verification

- CLI preset dropdown: exactly 4 options (piper, kokoro-tts, qwen3-tts, custom): PASS
- No install guidance block in local-engine.svelte: PASS
- Piper voice dropdown unchanged: PASS
- kokoro-tts shows KOKORO_VOICES dropdown (11 voices): PASS
- qwen3-tts shows QWEN3_VOICES dropdown (2 voices): PASS
- ElevenLabs: "Check API Key" calls check_elevenlabs_credentials; "Test Engine" calls test_tts_engine: PASS
- OpenAI: "Check API Key" calls check_openai_credentials; "Test Engine" calls test_tts_engine: PASS
- ElevenLabs voice section: dropdown or empty-state guidance (no text input): PASS
- `bun run check`: 0 new errors (same pre-existing errors as before): PASS
- `bun run test`: 81 tests pass, 0 failures: PASS

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test failures caused by removed install card**
- **Found during:** Task 2 (test run after completion)
- **Issue:** 8 ENG-05 tests in local-engine.test.ts tested install card UI that was deleted in Task 1. Additionally, ENG-03 "success" tests in all 3 engine test files failed with "Found multiple elements with text: Engine is working" — AlertTitle and AlertDescription both show the message when it equals the title text.
- **Fix:** Replaced 8 ENG-05 install-card tests with single negative assertion (install card must NOT appear). Fixed 3 "success" tests to use `getAllByText` instead of `getByText`.
- **Files modified:** local-engine.test.ts, elevenlabs-engine.test.ts, openai-engine.test.ts
- **Commit:** 2d71821

## Self-Check

- [x] `src/lib/components/engine/local-engine.svelte` — has 4 preset options, 3 voice dropdowns
- [x] `src/lib/components/engine/elevenlabs-engine.svelte` — has Check API Key + Test Engine buttons
- [x] `src/lib/components/engine/openai-engine.svelte` — has Check API Key + Test Engine buttons
- [x] commit 4704bf2 exists
- [x] commit 2d71821 exists
- [x] 81 tests pass (PASS 81, FAIL 0)

## Self-Check: PASSED
