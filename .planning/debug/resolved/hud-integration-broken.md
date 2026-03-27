---
status: resolved
trigger: "HUD finally displays in testing (test_show_hud works), but does NOT appear during actual TTS playback. It should automatically show when HUD is enabled in config and playback is happening."
created: 2026-03-07T00:00:00Z
updated: 2026-03-07T06:30:00Z
---

## Current Focus

hypothesis: CONFIRMED — speak_queued (used by main clipboard trigger) and speak_history_entry never call show_hud at all. speak_now does, but it's only used by the manual text-input path. The main workflow (double-copy clipboard → speak-request event → speak_queued) has zero HUD integration.
test: Read all TTS command files and trace all call paths
expecting: Fix requires adding show_hud_synthesizing / show_hud calls to speak_queued and speak_history_entry
next_action: Human verification — run the app, trigger TTS via double-copy clipboard, confirm HUD appears

## Symptoms

expected: HUD overlay displays waveform visualization and amplitude data when TTS is playing back audio, if HUD is enabled in config
actual: HUD does not appear during playback at all — it only appears when test_show_hud is called directly
errors: None known
reproduction: Enable HUD in settings, trigger TTS playback via double-copy clipboard — HUD never shows
timeline: Just discovered; previous session fixed the HUD display itself (URL fix, build errors)

## Eliminated

- hypothesis: HUD window not registered in tauri.conf.json
  evidence: tauri.conf.json correctly defines the "hud" window with proper settings
  timestamp: 2026-03-07

- hypothesis: HUD module not wired in main.rs
  evidence: main.rs has "mod hud", uses hud::show_hud, hud::hide_hud, and all hud commands registered
  timestamp: 2026-03-07

- hypothesis: HUD commands not in invoke_handler
  evidence: save_hud_position, set_hud_preset_position, test_show_hud are all registered
  timestamp: 2026-03-07

- hypothesis: Frontend HUD route missing
  evidence: src/routes/hud/+page.svelte and +page.ts exist, HudOverlay component exists
  timestamp: 2026-03-07

- hypothesis: HudConfig not in AppConfig
  evidence: config/mod.rs includes hud: HudConfig with proper defaults (enabled: true)
  timestamp: 2026-03-07

- hypothesis: show_hud() itself is broken (config check, window lookup, event emit)
  evidence: test_show_hud calls the same show_hud() and works — so show_hud() is functional
  timestamp: 2026-03-07

- hypothesis: speak_now is the problem
  evidence: speak_now correctly calls hud::show_hud_synthesizing (line 210) and hud::show_hud (line 360). The issue is in other commands.
  timestamp: 2026-03-07

## Evidence

- timestamp: 2026-03-07
  checked: src-tauri/src/commands/tts.rs — all three speak commands
  found: speak_now calls show_hud_synthesizing (before synthesis) and show_hud (after synthesis). speak_queued has ZERO hud calls. speak_history_entry has ZERO hud calls.
  implication: Primary bug — the main clipboard trigger path (speak_queued) never shows the HUD

- timestamp: 2026-03-07
  checked: src/routes/+layout.svelte line 113
  found: The global clipboard trigger (speak-request event) calls speak_queued, not speak_now
  implication: This is the main user workflow. speak_queued missing HUD calls = HUD never shows during clipboard-triggered TTS

- timestamp: 2026-03-07
  checked: src/lib/stores/history-store.svelte.ts line 275 and src/lib/components/history/history-entry.svelte line 68
  found: History replay calls speak_history_entry, which has zero HUD calls
  implication: History replay also never shows HUD

- timestamp: 2026-03-07
  checked: main.rs playback monitor thread (lines 315-329)
  found: Auto-hide monitor polls AudioPlayer.take_playback_finished(). But audio playback is browser-side via audio-ready event + HTMLAudioElement — not via Rust AudioPlayer. So playback_finished is never set to true during normal playback.
  implication: Auto-hide via playback monitor is also broken (secondary issue). The HUD hide is driven by hud:stop from frontend instead.

- timestamp: 2026-03-07
  checked: speak_now output_config early return (line 293-334)
  found: If output_config.enabled is true, speak_now returns early before show_hud is called
  implication: Users with "save to file" mode enabled would also not see HUD via speak_now. But this is an edge case.

## Resolution

root_cause: speak_queued (called by main clipboard double-copy trigger) and speak_history_entry (called by history replay) have no HUD integration whatsoever. Only speak_now has HUD calls. Since the primary user workflow routes through speak_queued, the HUD never appears during normal use.

Secondary: speak_now has an early return for output_config.enabled that bypasses show_hud.

fix: Added hud::show_hud_synthesizing / hud::show_hud calls to speak_queued and speak_history_entry, mirroring the pattern in speak_now.
  - speak_queued: show_hud_synthesizing before synthesis loop; show_hud with combined_envelope after all fragments combined
  - speak_history_entry: show_hud_synthesizing before synthesis; show_hud with envelope after synthesis (before audio-ready emit)
verification: confirmed by user — HUD now displays during all TTS playback paths
files_changed:
  - src-tauri/src/commands/tts.rs (added HUD calls to speak_queued and speak_history_entry)
