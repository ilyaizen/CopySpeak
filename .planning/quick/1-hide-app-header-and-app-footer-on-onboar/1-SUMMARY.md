---
phase: quick-1
plan: 01
subsystem: ui
tags: [svelte, onboarding, layout, health-check, tauri]

# Dependency graph
requires:
  - phase: 04-startup-onboarding
    provides: onboarding page and config_exists check
provides:
  - Fullscreen onboarding route (AppHeader/AppFooter hidden on /onboarding)
  - Automatic color-coded TTS health check on onboarding mount
  - Beautified onboarding page with gradient background and shadow card
affects: [onboarding, layout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "page.url.pathname from $app/state for reactive route detection in layout"
    - "Color-coded status pill with CSS box-shadow glow for engine health"

key-files:
  created: []
  modified:
    - src/routes/+layout.svelte
    - src/routes/onboarding/+page.svelte

key-decisions:
  - "Use $derived(page.url.pathname) in layout to reactively gate AppHeader/AppFooter — avoids window.location (non-reactive) pattern"
  - "runHealthCheck() fires after loadDefaultConfig() resolves but without await so config UI renders immediately"

patterns-established:
  - "Conditional chrome pattern: {#if !isOnboarding} guards in root layout for fullscreen routes"

requirements-completed: []

# Metrics
duration: 4min
completed: 2026-03-06
---

# Quick Task 1: Hide App Header and Footer on Onboarding Summary

**Fullscreen onboarding via reactive pathname guard in root layout, plus auto-running color-coded TTS health check pill with glow effect**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-06T05:20:09Z
- **Completed:** 2026-03-06T05:23:32Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Root layout now conditionally hides AppHeader and AppFooter on `/onboarding` using a `$derived` reactive signal from `$app/state`
- Main element padding is removed on `/onboarding` so the page fills full viewport height
- Onboarding page invokes `test_tts_engine` automatically on mount (after config loads), showing a pulsing indicator then a green/red status pill with glow shadow
- Onboarding page visually improved: gradient background, shadow card, gradient h1 text

## Task Commits

Each task was committed atomically:

1. **Task 1: Hide AppHeader/AppFooter on /onboarding route** - `a8edd87` (feat)
2. **Task 2: Beautify onboarding page and add color-coded health check** - `e0b613a` (feat)

**Plan metadata:** (see final docs commit)

## Files Created/Modified
- `src/routes/+layout.svelte` - Added `page` import from `$app/state`, `isOnboarding` derived, conditional header/footer guards, conditional main padding
- `src/routes/onboarding/+page.svelte` - Added `runHealthCheck()`, health state variables, health status UI block, gradient background, shadow card, gradient h1

## Decisions Made
- Used `$derived(page.url.pathname === "/onboarding")` — `page` from `$app/state` is reactive in Svelte 5 SvelteKit, `window.location` is not
- `runHealthCheck()` is called without `await` after `loadDefaultConfig()` so the config UI renders immediately while the health check runs concurrently in background

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `bun run check` exits with 8 pre-existing errors in unrelated files (`elevenlabs-engine.svelte`, `http-engine.svelte`, `openai-engine.svelte`, `engine-tabs.test.ts`, `app-header.test.ts`). None introduced by this task.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Onboarding fullscreen experience is complete and ready for user testing
- Health check wires directly to existing `test_tts_engine` Tauri command — no backend changes needed

---
*Phase: quick-1*
*Completed: 2026-03-06*

## Self-Check: PASSED
- src/routes/+layout.svelte: FOUND
- src/routes/onboarding/+page.svelte: FOUND
- .planning/quick/1-hide-app-header-and-app-footer-on-onboar/1-SUMMARY.md: FOUND
- Commit a8edd87: FOUND
- Commit e0b613a: FOUND
