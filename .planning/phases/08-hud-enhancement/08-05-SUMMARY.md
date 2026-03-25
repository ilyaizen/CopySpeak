---
phase: 08-hud-enhancement
plan: 05
subsystem: ui
tags: [web-audio-api, analyser-node, fft, svelte5, tauri-events, canvas, waveform]

# Dependency graph
requires:
  - phase: 08-hud-enhancement/08-01
    provides: HUD overlay component structure with isSynthesizing state and hud:start/hud:stop listeners
  - phase: 08-hud-enhancement/08-04
    provides: playbackStore with AudioContext, _audioEl, and setupListeners pattern

provides:
  - AnalyserNode pipeline in PlaybackStore tapping HTMLAudioElement via createMediaElementSource()
  - 30fps rAF loop emitting hud:amplitude events with 16-bar FFT data
  - buildBarValues() logarithmic bin-to-bar mapping (128 bins -> 16 bars)
  - Pure barValues-driven waveform.svelte canvas renderer (no internal animation loop)
  - hud-overlay.svelte hud:amplitude listener feeding live barValues to Waveform
  - HUD auto-hide fix: hud:stop emitted in onended and handleStop()

affects: [hud-overlay, waveform, playback-store, audio-playback]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Web Audio API AnalyserNode tapped from HTMLAudioElement via createMediaElementSource() with mandatory analyser.connect(destination) for audio routing
    - Cross-window amplitude data via Tauri global events (hud:amplitude with bars array payload)
    - Logarithmic FFT bin grouping for perceptual frequency balance (bass maps to first bars)
    - Pure prop-driven canvas renderer - no internal state or animation loop

key-files:
  created: []
  modified:
    - src/lib/stores/playback-store.svelte.ts
    - src/lib/components/waveform.svelte
    - src/lib/components/hud-overlay.svelte

key-decisions:
  - "Used createMediaElementSource() guard (!this._sourceNode) to prevent InvalidStateError on repeated calls"
  - "analyser.connect(audioCtx.destination) is mandatory - without it audio is silenced by Web Audio graph"
  - "AnalyserNode wired lazily in handleAudioReady() (not setAudioElement()) because _audioCtx is null during setAudioElement"
  - "Amplitude loop reads FFT via getByteFrequencyData and normalizes to 0.0-1.0 float range"
  - "barColor used for silent bars (amplitude <= minBarHeight), activeBarColor for live bars - visual distinction preserved"

patterns-established:
  - "Tauri event emission from frontend store: _emit field set from @tauri-apps/api/event in setupListeners()"
  - "Cross-window live data: rAF loop throttled at 33ms (~30fps) for performance"

requirements-completed: [HUD-02, HUD-03]

# Metrics
duration: 9min
completed: 2026-03-07
---

# Phase 08 Plan 05: Waveform Overhaul Summary

**AnalyserNode pipeline delivers real-time 16-bar FFT waveform in HUD via hud:amplitude Tauri events at 30fps, replacing pre-computed envelope oscillation entirely**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-07T14:31:06Z
- **Completed:** 2026-03-07T14:40:31Z
- **Tasks:** 2
- **Files modified:** 4 (3 source + CHANGELOG.md)

## Accomplishments
- AnalyserNode wired to HTMLAudioElement via createMediaElementSource() in PlaybackStore with correct routing to AudioContext destination (no silent audio regression)
- 30fps rAF amplitude loop emits 16-bar FFT data as hud:amplitude Tauri events; loop stops cleanly on playback end or stop
- HUD auto-hide bug fixed: hud:stop now emitted in both onended and handleStop()
- waveform.svelte fully rewritten as prop-driven renderer — zero internal animation state, reacts to barValues prop changes via $effect
- hud-overlay.svelte hud:amplitude listener updates barValues state; waveform hidden during synthesizing state via {#if !isSynthesizing}

## Task Commits

Each task was committed atomically:

1. **Task 1: Add AnalyserNode pipeline and 30fps amplitude emit loop to playbackStore** - `3bf919a` (feat)
2. **Task 2: Replace envelope-based waveform with barValues renderer, then wire hud-overlay** - `42e8861` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/lib/stores/playback-store.svelte.ts` - Added buildBarValues(), AnalyserNode pipeline, startAmplitudeLoop/stopAmplitudeLoop, _emit field, hud:stop emission
- `src/lib/components/waveform.svelte` - Full rewrite: barValues prop replaces envelope/isPlaying/animationSpeed, pure reactive canvas renderer
- `src/lib/components/hud-overlay.svelte` - Added barValues state, hud:amplitude listener, {#if !isSynthesizing} Waveform guard, removed envelope/isPlaying
- `CHANGELOG.md` - Documented real-time waveform and HUD auto-hide changes

## Decisions Made
- AnalyserNode created lazily in `handleAudioReady()` (not `setAudioElement()`) — AudioContext is null at setAudioElement time
- `createMediaElementSource()` guarded with `if (!this._sourceNode)` to prevent InvalidStateError on repeated audio-ready events
- `analyser.connect(audioCtx.destination)` explicitly added — without this the audio graph absorbs output silently
- `barColor` used for silent (minimum-height) bars; `activeBarColor` for live amplitude bars — subtle visual distinction
- `_emit` stored as instance field, populated from `@tauri-apps/api/event` in `setupListeners()` — consistent with existing store pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed stale isPlaying reference in handleSynthesizing()**
- **Found during:** Task 2 (hud-overlay.svelte update)
- **Issue:** After removing the `isPlaying` state variable, `handleSynthesizing()` still contained `isPlaying = false;` which would cause a TypeScript error
- **Fix:** Replaced with `barValues = []` to clear waveform data when synthesizing begins
- **Files modified:** src/lib/components/hud-overlay.svelte
- **Verification:** bun run check shows no errors in hud-overlay.svelte
- **Committed in:** 42e8861 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed unused props TypeScript errors in waveform.svelte**
- **Found during:** Task 2 verification (bun run check)
- **Issue:** `barColor` and `backgroundColor` props were declared but not referenced in the new drawWaveform() function, causing TypeScript "declared but never read" errors
- **Fix:** Used `backgroundColor` for canvas background fill (non-transparent case) and `barColor` for silent/minimum-height bars vs `activeBarColor` for live bars
- **Files modified:** src/lib/components/waveform.svelte
- **Verification:** bun run check shows no errors in waveform.svelte
- **Committed in:** 42e8861 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 - Bug)
**Impact on plan:** Both fixes required for TypeScript correctness; no scope creep, no behavioral change from plan intent.

## Issues Encountered
- Pre-existing TypeScript errors in 5 unrelated files (engine components and test files) — not introduced by this plan, deferred as out-of-scope per deviation rules

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Real-time waveform visualization fully implemented and HUD auto-hide bug fixed
- Phase 08 (HUD Enhancement) requirements HUD-02 and HUD-03 are complete
- Audio pipeline: AnalyserNode wired correctly with no silent audio regression
- Phase 08 can be marked complete if no remaining plans

## Self-Check: PASSED

- FOUND: src/lib/stores/playback-store.svelte.ts
- FOUND: src/lib/components/waveform.svelte
- FOUND: src/lib/components/hud-overlay.svelte
- FOUND: .planning/phases/08-hud-enhancement/08-05-SUMMARY.md
- FOUND commit: 3bf919a (Task 1)
- FOUND commit: 42e8861 (Task 2)
- FOUND commit: 90fb253 (metadata)

---
*Phase: 08-hud-enhancement*
*Completed: 2026-03-07*
