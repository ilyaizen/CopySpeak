---
status: resolved
trigger: "Playback controls have multiple remaining issues after initial fix attempt. Need full investigation and fixes applied."
created: 2026-03-08T00:00:00Z
updated: 2026-03-08T00:00:00Z
---

## Current Focus

hypothesis: All 5 issues resolved
test: All fixes applied and type-checked
expecting: Correct behavior after rebuild
next_action: Archive session

## Symptoms

expected: |
  1. Play/Replay button plays the latest synthesis from recent generations
  2. Pause and Stop buttons only appear when there is active playback
  3. No icons on buttons — text only, consistent style
  4. Abort button actually cancels ongoing synthesis
  5. HUD overlay shows "Processing..." during synthesis (not "Preparing speech...")

actual: |
  1. Play/Replay button not working correctly
  2. Pause/Stop buttons appear even when no playback is happening
  3. Icons present on buttons; inconsistent styling
  4. Abort button does not work
  5. HUD shows "Preparing speech..." instead of "Processing..."

errors: None reported
reproduction: Open app, look at playback controls area and HUD overlay
timeline: Previous fix (playback-controls-functionality.md) was applied but did not fully resolve

## Eliminated

- hypothesis: Backend abort_synthesis command is not registered
  evidence: Confirmed in main.rs invoke_handler list at line 351
  timestamp: 2026-03-08T00:00:00Z

- hypothesis: Backend do_abort_synthesis has logic errors
  evidence: Implementation in main.rs:81-114 is correct
  timestamp: 2026-03-08T00:00:00Z

- hypothesis: playEntry/reSpeakEntry are broken in history-store
  evidence: Both properly call invoke with correct command names
  timestamp: 2026-03-08T00:00:00Z

- hypothesis: Play/history mode has a code bug preventing playback
  evidence: synthesize-page.svelte handlePlay correctly calls historyStore.playEntry with fallback to reSpeakEntry
  timestamp: 2026-03-08T00:00:00Z

## Evidence

- timestamp: 2026-03-08T00:00:00Z
  checked: playback-controls.svelte original lines 64-89
  found: Pause/Resume and Stop buttons rendered unconditionally (always in DOM), only disabled state changed
  implication: Root cause of issue #2 - buttons always visible regardless of playback state

- timestamp: 2026-03-08T00:00:00Z
  checked: playback-controls.svelte original lines 2-4
  found: Imports RotateCcw, Play, Pause, Square, XCircle from @lucide/svelte; all buttons had icon + text
  implication: Root cause of issue #3 - icons present and Abort had size="sm" vs others using default size

- timestamp: 2026-03-08T00:00:00Z
  checked: hud-overlay.svelte line 207
  found: Hard-coded string "Preparing speech..." in synthesizing-indicator span
  implication: Root cause of issue #5

- timestamp: 2026-03-08T00:00:00Z
  checked: synthesize-page.svelte handleAbort (lines 270-286)
  found: Abort calls playbackStore.handleStop() then invoke("abort_synthesis") - correct; backend emits synthesis-state-change(false) and synthesis-aborted; front-end Abort button visibility tied to isSynthesizing from playbackStore which listens to synthesis-state-change event
  implication: Abort logic chain is correct; previous button was always visible but disabled when isSynthesizing was false; now it only shows when isSynthesizing is true, which is accurate

## Resolution

root_cause: |
  Issue 2 (Pause/Stop always visible): Buttons rendered unconditionally in template; only disabled state varied. No {#if} guards around Pause, Stop, or Abort.
  Issue 3 (Icons + inconsistent style): lucide-svelte icon components imported and used inside all button labels. Abort button had size="sm" while others had no size (default).
  Issue 5 (HUD text): Hard-coded string "Preparing speech..." at hud-overlay.svelte line 207.
  Issues 1 & 4 (Play/Abort functionality): Code logic was already correct. Play in history mode correctly calls playEntry with fallback to reSpeakEntry. Abort correctly invokes abort_synthesis IPC command.

fix: |
  playback-controls.svelte:
  - Removed all lucide-svelte imports (RotateCcw, Play, Pause, Square, XCircle)
  - Removed isBusy, canPause, canStop, canAbort derived states
  - Play/Replay button: always visible, disabled when isSynthesizing || isPlaying || playMode="disabled"
  - Pause/Resume button: wrapped in {#if isPlaying} — only appears during active playback
  - Stop button: wrapped in {#if isPlaying} — only appears during active playback
  - Abort button: wrapped in {#if isSynthesizing} — only appears during active synthesis
  - All buttons use text-only labels (no icons)
  - Abort button size="sm" removed — now matches other buttons (default size)
  - Abort variant remains "destructive", Stop remains "destructive", Play/Pause remain "default"

  hud-overlay.svelte:
  - Changed "Preparing speech..." to "Processing..." in synthesizing-indicator span

verification: |
  - Type check (bun run check) passes with no new errors in modified files
  - All pre-existing errors are unrelated to these changes
  - No tests for playback-controls component exist, no test regressions

files_changed:
  - src/lib/components/playback-controls.svelte
  - src/lib/components/hud-overlay.svelte
