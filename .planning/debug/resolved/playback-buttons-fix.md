---
status: resolved
trigger: "the abort button still won't appear correctly and the play button should replay (without resynthesis) the last item in recent history"
created: 2026-03-08T00:00:00.000Z
updated: 2026-03-08T00:00:00.000Z
---

## Current Focus
hypothesis: N/A - Fixed
test: N/A
expecting: N/A
next_action: Archive

## Symptoms
expected: 
  - Abort button always visible, disabled when idle
  - Play button replays last history item without resynthesis if output_path exists
actual:
  - Abort button only visible when synthesizing
  - Play button uses unsorted history items
errors: None
reproduction: Click Play when text input is empty
started: Always been this way

## Evidence
- timestamp: 2026-03-08
  checked: playback-controls.svelte lines 76-84
  found: `{#if isSynthesizing}` conditional hides Abort button
  implication: Remove conditional, use disabled prop instead

- timestamp: 2026-03-08
  checked: synthesize-page.svelte line 208
  found: `historyStore.items[0]` used without sorting
  implication: Not getting the most recent item

- timestamp: 2026-03-08
  checked: recent-history.svelte lines 31-35
  found: Correct pattern: sort by timestamp descending
  implication: Use same pattern in synthesize-page

## Resolution
root_cause: Two issues - conditional rendering of Abort, unsorted history access
fix: 
  1. Removed `{#if}` conditionals for Pause/Stop/Abort buttons - all now always visible with proper disabled states
  2. Abort disabled when `!isSynthesizing && !isPlaying`
  3. History play mode now sorts by timestamp and checks `output_path` before playing
verification: Visual inspection of both files shows correct implementation
files_changed:
  - src/lib/components/playback-controls.svelte
  - src/lib/components/synthesize-page.svelte
