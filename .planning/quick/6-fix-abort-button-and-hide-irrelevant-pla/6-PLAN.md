# Quick Task Plan: Fix Abort Button and Hide Irrelevant Playback Controls

**Created:** 2026-03-08
**Task:** #6

## Goal
Fix the Abort button functionality and hide playback control buttons when they are irrelevant/non-functional.

## Current Issues
1. **Abort button**: Present but may not be working properly - needs to abort synthesis and stop playback
2. **Button visibility**: All buttons always visible even when non-functional:
   - Pause button shown when not playing
   - Stop button shown when not playing  
   - Abort button shown when neither synthesizing nor playing

## Implementation Tasks

### Task 1: Fix Abort Button Functionality
**Files:** `src/lib/components/synthesize-page.svelte`

The Abort handler exists but let's verify it's working correctly:
- It calls `invoke("abort_synthesis")` which kills CLI TTS processes
- It calls `playbackStore.handleStop()` to stop audio playback
- It sets `abortRequested = true` flag to suppress error messages

**Action:** Verify the abort flow works end-to-end. The code looks correct - abort_synthesis command exists in backend and is registered.

**Verification:** Test that clicking Abort during synthesis stops the process and clears state.

### Task 2: Hide Irrelevant Playback Buttons
**Files:** `src/lib/components/playback-controls.svelte`

Update button visibility logic:
- **Pause/Resume button**: Only show when `isPlaying` is true
- **Stop button**: Only show when `isPlaying` is true
- **Abort button**: Only show when `isSynthesizing || isPlaying` is true

Keep Play button always visible (shows as disabled when unavailable).

**Action:** Wrap Pause, Stop, and Abort buttons in `{#if}` conditional blocks based on state.

**Verification:** 
- When idle: Only Play button visible
- When synthesizing: Play (disabled), Abort visible
- When playing: Play (disabled), Pause, Stop, Abort all visible
- When paused: Play (disabled), Resume, Stop, Abort all visible

## Files to Modify

1. `src/lib/components/playback-controls.svelte` - Hide irrelevant buttons based on state

## Success Criteria
- [ ] Abort button works during synthesis (stops process, no errors)
- [ ] Pause button hidden when not playing
- [ ] Stop button hidden when not playing
- [ ] Abort button hidden when neither synthesizing nor playing
- [ ] Play button always visible (disabled state works correctly)
