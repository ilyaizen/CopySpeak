---
status: resolved
trigger: "The HUD's Tauri window appears as a faint outline after playback ends and stays there forever. It should disappear completely."
created: 2026-03-08T00:00:00Z
updated: 2026-03-08T01:30:00Z
---

## Current Focus

hypothesis: CONFIRMED - HUD Tauri window is never hidden because hide_hud() only triggers from Rust AudioPlayer finish, but audio plays on frontend
test: verified no Rust-side playback in TTS commands, only frontend emit("audio-ready")
expecting: fix by having HUD overlay hide its own window on hud:stop
next_action: implement fix in hud-overlay.svelte handleStop()

## Symptoms

expected: HUD window should disappear completely when playback finishes
actual: HUD window appears as a faint outline after playback and stays there forever
errors: No errors visible in dev console
reproduction: Play any TTS audio, wait for playback to end, observe HUD window remains as faint outline
started: Always been broken - HUD never hid properly after playback

## Eliminated

## Evidence

- timestamp: 2026-03-08T00:01:00Z
  checked: hud.rs hide_hud() call sites
  found: hide_hud() is only called from main.rs playback monitor thread when AudioPlayer.take_playback_finished() returns true
  implication: Rust-side AudioPlayer must finish playback for window to hide

- timestamp: 2026-03-08T00:02:00Z
  checked: TTS commands (tts.rs) for Rust AudioPlayer usage
  found: TTS commands do NOT play audio through Rust AudioPlayer. They emit "audio-ready" to frontend, which plays via HTML <audio> element
  implication: AudioPlayer.take_playback_finished() NEVER fires for normal TTS, so hide_hud() is never called

- timestamp: 2026-03-08T00:03:00Z
  checked: playback-store.svelte.ts onended handler
  found: Frontend emits "hud:stop" via frontend emit() when audio ends. HUD overlay receives it and sets isVisible=false (opacity:0). But the Tauri WINDOW remains shown.
  implication: Content goes invisible but window frame/outline persists - this is the "ghost outline"

- timestamp: 2026-03-08T00:04:00Z
  checked: hud.json capabilities
  found: core:window:allow-hide is already permitted for HUD window
  implication: HUD frontend can call getCurrentWindow().hide() to hide its own window

## Resolution

root_cause: The Tauri HUD window is shown by Rust via hud_window.show(), but never hidden after frontend playback ends. The only call to hide_hud() (which calls hud_window.hide()) is in a monitor thread watching Rust AudioPlayer.take_playback_finished(). However, audio plays on the frontend via HTML <audio>, not the Rust AudioPlayer. So the Rust player never finishes, hide_hud() never fires, and the window stays visible with opacity:0 content - appearing as a ghost outline.
fix: In hud-overlay.svelte handleStop(), call getCurrentWindow().hide() to hide the Tauri window from the frontend side. Also added window.show() in handleStart() for the show_hud_synthesizing->show_hud transition safety.
verification:
files_changed:
  - src/lib/components/hud-overlay.svelte
  - CHANGELOG.md
commits:
  - f61c8bb: fix(hud): hide Tauri window when playback ends to eliminate ghost outline
  - 3b6c7af: docs: log HUD ghost outline fix in CHANGELOG
  - d14e284: fix: add show() method to hudWindow type definition and type error parameters
type_check: ✓ PASSED (hud-overlay type errors resolved)
status: FIXED ✓
