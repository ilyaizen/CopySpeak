# Quick Task Summary #6

**Task:** Fix Abort button and hide irrelevant playback controls  
**Date:** 2026-03-08  
**Status:** Complete

## Changes Made

### Modified Files

1. **`src/lib/components/playback-controls.svelte`**
   - Wrapped Pause/Resume button in `{#if isPlaying}` conditional
   - Wrapped Stop button in `{#if isPlaying}` conditional  
   - Wrapped Abort button in `{#if isSynthesizing || isPlaying}` conditional
   - Removed `disabled` props from conditional buttons (no longer needed since they're hidden instead)
   - Play button remains always visible with proper disabled state

2. **`src-tauri/src/commands/tts.rs`**
   - Added `SynthesisGuard` to `speak_queued` function (line ~527)
   - This fixes the bug where `isSynthesizing` was never set to `true` during CLI synthesis triggered by double-copy
   - The `SynthesisGuard` RAII pattern ensures synthesis state is properly emitted to the frontend

## Behavior Changes

**Before:**
- All buttons always visible
- Pause, Stop, Abort buttons disabled but shown when irrelevant
- Cluttered UI with non-functional buttons

**After:**
- **Idle state:** Only Play button visible (disabled until text entered)
- **Synthesizing:** Play (disabled) + Abort visible
- **Playing:** Play (disabled) + Pause + Stop + Abort all visible
- **Paused:** Play (disabled) + Resume + Stop + Abort all visible

## Root Cause of Missing Abort Button During Synthesis

**The Bug:** `speak_queued` (used for double-copy synthesis) did not use `SynthesisGuard`, so `isSynthesizing` was never set to `true` when CLI synthesis was triggered via double-copy. The Abort button relies on `isSynthesizing || isPlaying` to determine visibility.

**The Fix:** Added `SynthesisGuard::new(&app)` to `speak_queued` at line ~527, right before the fragment synthesis loop. This RAII guard:
- Emits `synthesis-state-change` event with `true` when created
- Emits `synthesis-state-change` event with `false` when dropped (function ends)
- Updates the `JobStatus.is_synthesizing` atomic flag
- Updates the tray icon to show busy state

This matches the existing pattern in `speak_now` which already had the guard.

## Abort Button Functionality

The Abort button implementation:
- Calls `invoke("abort_synthesis")` to kill CLI TTS processes (via `ACTIVE_CLI_PID`)
- Calls `playbackStore.handleStop()` to stop audio playback
- Sets `abortRequested` flag to suppress error toasts
- Shows success toast when abort completes

## Testing Notes

- Buttons appear/disappear correctly based on playback state
- Abort button works during synthesis (kills process, no errors shown)
- UI is cleaner with only relevant controls visible
- No breaking changes to existing functionality
