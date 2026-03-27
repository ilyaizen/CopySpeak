---
phase: 09-tts-engine-overhaul
plan: "04"
subsystem: settings-ui, documentation
tags: [gap-closure, dead-code-removal, requirements, phase-9]
dependency_graph:
  requires: [09-01, 09-02, 09-03]
  provides: [ENG-09-HTTP-REMOVE, ENG-09-CLI-CONSOLIDATE, ENG-09-TWO-STAGE, ENG-09-OPENAI-VOICE, ENG-06]
  affects: [REQUIREMENTS.md, tts-settings.svelte]
tech_stack:
  added: []
  patterns: [dead-code-purge, requirements-traceability]
key_files:
  created: []
  modified:
    - src/lib/components/settings/tts-settings.svelte
    - .planning/REQUIREMENTS.md
decisions:
  - "tts-settings.svelte retained (not deleted) — file may be re-imported in a future phase"
  - "REQUIREMENTS.md v0.1 total updated from 13 to 17 to reflect Phase 9 additions"
metrics:
  duration: "4min"
  completed: "2026-03-09"
  tasks: 2
  files: 2
---

# Phase 9 Plan 04: Gap Closure — HTTP Dead Code Purge and Phase 9 Requirement Traceability Summary

**One-liner:** HTTP engine dead code purged from orphaned tts-settings.svelte and four ENG-09-* requirements with full traceability added to REQUIREMENTS.md, closing all Phase 9 verification gaps.

## Objective

Close two documentation and dead-code gaps identified by the Phase 9 verifier: strip all HTTP engine references from the orphaned tts-settings.svelte file, and add the missing ENG-09-* requirement definitions plus Phase 9 traceability rows to REQUIREMENTS.md.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Strip HTTP engine dead code from tts-settings.svelte | b437e4e | src/lib/components/settings/tts-settings.svelte |
| 2 | Add Phase 9 requirement definitions and traceability to REQUIREMENTS.md | 5c889e5 | .planning/REQUIREMENTS.md |

## What Was Built

### Task 1: tts-settings.svelte HTTP purge

Removed all HTTP engine dead code from the orphaned settings component:

- Removed `{ value: "http", label: "Local HTTP Server" }` from `backendOptions` array — component now only exposes `local`, `openai`, and `elevenlabs`
- Deleted the entire `HTTP_PRESETS` map (5 server presets: kokoro-server, fish-speech, coqui-server, chatterbox-server, openai-compatible)
- Deleted `httpPresetOptions` array, `selectedHttpPreset` reactive state, and `applyHttpPreset()` function
- Deleted the entire `{:else if active_backend === "http"}` UI block (server preset selector, URL template input, body template input, response format select, timeout input, voice input — all binding to `localConfig.http_tts.*` fields that no longer exist on `AppConfig`)

The file is now safe to re-import without runtime errors.

### Task 2: REQUIREMENTS.md Phase 9 traceability

Added a new "Engine Overhaul" section under v2 Requirements with four requirement definitions:
- **ENG-09-HTTP-REMOVE**: HTTP engine removal (type narrowed, http.rs deleted, migration added)
- **ENG-09-CLI-CONSOLIDATE**: CLI preset consolidation to piper, kokoro-tts, qwen3-tts
- **ENG-09-TWO-STAGE**: Two-stage health check for cloud engines
- **ENG-09-OPENAI-VOICE**: OpenAI voice static dropdown with all 9 official voices

Marked **ENG-06** as `[x]` complete (ElevenLabs voice dropdown implemented in Phase 9).

Added 5 Phase 9 traceability rows to the Traceability table. Updated coverage count from 13 to 17 total requirements. Removed ENG-06 from pending list. Updated datestamp to 2026-03-09.

## Verification

| Check | Result |
|-------|--------|
| `grep -c "http_tts\|HTTP_PRESETS" tts-settings.svelte` | 0 |
| `grep -c '"http"' tts-settings.svelte` | 0 |
| `grep -c "ENG-09-HTTP-REMOVE" REQUIREMENTS.md` | 2 (definition + traceability) |
| `grep -c "ENG-06.*Phase 9" REQUIREMENTS.md` | 1 |
| `bun run check` errors in tts-settings.svelte | 0 |

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- tts-settings.svelte: FOUND
- REQUIREMENTS.md: FOUND
- SUMMARY.md: FOUND
- Commit b437e4e (Task 1): FOUND
- Commit 5c889e5 (Task 2): FOUND
