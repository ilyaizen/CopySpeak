---
phase: 02-engine-page
plan: 03
subsystem: frontend, backend
tags: [cleanup, synthesis-speed]

# Dependency graph
requires:
  - phase: 02-engine-page
    provides: Engine page implementation complete
provides:
  - All synthesis speed placeholders removed from CLI presets
  - Placeholder hint text updated to remove speed references
  - Rust defaults cleaned up to remove length_scale
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - src/lib/components/settings/tts-settings.svelte
    - src-tauri/src/config/tts.rs

key-decisions:
  - "Synthesis speed is client-side playback control, not TTS engine synthesis parameter"

patterns-established: []

requirements-completed: []

# Metrics
duration: 5min
completed: 2026-03-05T14:00:00Z
---

# Phase 2: Plan 3 - Synthesis Speed Cleanup Summary

**Removed all synthesis speed placeholders from CLI presets and Rust defaults**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-05T13:55:00Z
- **Completed:** 2026-03-05T14:00:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Removed `{speed}` from kokoro-tts CLI preset in tts-settings.svelte
- Removed `{speed}` from placeholder hints (two occurrences)
- Removed `{length_scale}` from Rust default TtsConfig args_template

## Task Commits

Each task was committed atomically:

1. **Task 1: Remove speed from kokoro-tts preset** - Removed `--speed {speed}` args
2. **Task 2: Update placeholder hints** - Removed `{speed}` references from hint text
3. **Task 3: Remove length_scale from Rust** - Removed `--length-scale {length_scale}` from defaults

**Plan metadata:** `[plan-id-placeholder]` (cleanup: remove synthesis speed placeholders)

## Files Created/Modified

- `src/lib/components/settings/tts-settings.svelte`
  - Line 69: Removed `--speed` and `{speed}` from kokoro-tts preset
  - Line 307: Removed `{speed}` from CLI placeholder hint
  - Line 362: Removed `{speed}` from HTTP placeholder hint

- `src-tauri/src/config/tts.rs`
  - Lines 127-128: Removed `--length-scale` and `{length_scale}` from default args_template

## Decisions Made

- Synthesis speed is a client-side playback parameter, not a TTS engine synthesis parameter
- Speed placeholders should not exist in CLI presets or Rust default configurations
- Placeholder hints must reflect actual available placeholders

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Synthesis speed cleanup complete. Phase 2 can now be considered complete (all 6 plans done). Ready for Phase 3 (Health Check UI) planning.

---

*Phase: 02-engine-page*
*Completed: 2026-03-05*

## Self-Check: PASSED

- ✓ No `{speed}` references found in tts-settings.svelte
- ✓ No `{length_scale}` references found in tts.rs
- ✓ Placeholder hints updated to remove speed references
- ✓ CLI presets cleaned up
