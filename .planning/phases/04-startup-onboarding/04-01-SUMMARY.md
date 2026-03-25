---
phase: 04-startup-onboarding
plan: 01
subsystem: onboarding
tags: [first-run, config-detection, svelte, tauri-ipc, routing]

# Dependency graph
requires: []
provides:
  - First-run detection via config_exists IPC command
  - Full-screen onboarding page with TTS configuration
  - Automatic redirect to onboarding for fresh installs
affects: [startup, user-experience, configuration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - First-run detection via config file existence (not defaults)
    - Full-screen modal pages without app chrome
    - onMount-based redirect to avoid IPC race conditions

key-files:
  created:
    - src/routes/onboarding/+page.svelte
  modified:
    - src-tauri/src/commands/config.rs
    - src-tauri/src/main.rs
    - src/routes/+layout.svelte

key-decisions:
  - "Use config file existence check instead of default config detection"
  - "Full-screen onboarding without AppHeader/AppFooter for focused setup experience"
  - "onMount redirect pattern prevents IPC race condition mentioned in roadmap"
  - "Reuse LocalEngine component to avoid duplicating engine configuration logic"
  - "Skip button saves minimal config, allowing immediate app use"

patterns-established:
  - "First-run detection: Check config file existence, not default values"
  - "Full-screen modals: Omit AppHeader/AppFooter, centered card layout"
  - "Redirect timing: Use onMount, not load function, for Tauri IPC calls"

requirements-completed: [OBD-01, OBD-02, OBD-03]

# Metrics
duration: 3 min
completed: 2026-03-05
---

# Phase 4 Plan 01: First-Run Onboarding Summary

**First-run detection with full-screen onboarding flow using config_exists IPC, automatic redirect, and LocalEngine component reuse**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-05T17:47:51Z
- **Completed:** 2026-03-05T17:50:56Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- First-run detection distinguishes fresh installs from existing users
- Full-screen onboarding page guides new users through TTS setup
- Automatic redirect ensures new users see onboarding on first launch
- Skip option respects user choice while preventing re-redirect
- Component reuse avoids duplicating engine configuration logic

## Task Commits

Each task was committed atomically:

1. **Task 1: config_exists IPC command** - `f0ce630` (feat)
2. **Task 2: onboarding route with full-screen layout** - `eb82bba` (feat)
3. **Task 3: first-run redirect in root layout** - `cf0d980` (feat)

**Requirements update:** `fe3bdc2` (docs)

## Files Created/Modified
- `src-tauri/src/commands/config.rs` - Added config_exists command
- `src-tauri/src/main.rs` - Registered config_exists in invoke_handler
- `src/routes/onboarding/+page.svelte` - Full-screen onboarding page (created)
- `src/routes/+layout.svelte` - Added first-run redirect logic
- `.planning/REQUIREMENTS.md` - Updated OBD requirements

## Decisions Made

1. **Config file existence check**: Using `config::config_path().exists()` directly tells us if this is a fresh install. The existing `get_config` returns defaults when missing, which doesn't distinguish "new user" from "user with default config".

2. **Full-screen modal design**: Onboarding page omits AppHeader and AppFooter for a focused, distraction-free setup experience. Uses centered card layout matching brutalist theme.

3. **onMount redirect pattern**: Checking in onMount (not +layout.ts load) avoids IPC race condition mentioned in roadmap. The redirect happens after Tauri is ready, not during SSR/load.

4. **Component reuse**: LocalEngine component reused for TTS configuration, avoiding duplication of engine config logic and maintaining consistency with Engine tab.

5. **Skip behavior**: Skip button saves minimal valid config (default AppConfig), allowing immediate app use. User can configure engine later from Engine tab.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all tasks completed smoothly without errors or blockers.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

First-run onboarding complete. New users will see onboarding on first launch, existing users skip directly to Play tab. Ready for next phase or project completion.

---
*Phase: 04-startup-onboarding*
*Completed: 2026-03-05*

## Self-Check: PASSED

- ✅ src/routes/onboarding/+page.svelte exists
- ✅ src/routes/+layout.svelte contains config_exists check
- ✅ src-tauri/src/commands/config.rs contains config_exists command
- ✅ All commits present in git history
