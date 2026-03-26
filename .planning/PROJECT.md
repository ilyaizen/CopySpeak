# CopySpeak

## What This Is

CopySpeak is a lightweight Windows desktop application that wraps AI Text-to-Speech engines. It silently monitors the clipboard and reads text aloud when the user copies the same text twice in quick succession (double-copy trigger). It runs in the system tray, staying out of the way until needed.

**Shipped:** v0.1 TTS Engine Overhaul (2026-03-26) — HTTP engine removed, CLI consolidated to 3 presets, ElevenLabs/OpenAI two-stage health checks, OpenAI 9-voice dropdown.

## Current Milestone: v0.2 Settings Consolidation

**Goal:** Reduce settings tabs from 8 to 4, implement 2-column layout, and minify redundant UI controls.

**Target features:**

- Consolidate 8 tabs → 3 tabs (General, Advanced, About)
- 2-column layout (Label+InfoTip | Control) across all settings
- Minify Pagination to single select dropdown (Disabled/200/400.../2000)
- Minify HUD position to single select dropdown (Disabled/positions)
- Move Import/Export to About tab
- Cards within tabs to differentiate sections (Startup, Appearance, Playback, HUD, History, Triggers, Hotkeys in General; Pagination, Sanitization in Advanced)

## Core Value

Double-copy → instant speech must be flawless. If the trigger misfires or the voice takes too long, the app is useless.

## Requirements

### Validated

- ✓ Double-copy clipboard trigger using Win32 AddClipboardFormatListener — existing
- ✓ TTS backends: CLI (piper, kokoro-tts, qwen3-tts), ElevenLabs, OpenAI — v0.1
- ✓ Streaming TTS playback (audio starts before synthesis completes) — existing
- ✓ Audio playback via rodio with output device selection — existing
- ✓ Re-trigger modes: Interrupt or Queue — existing
- ✓ JSON config persistence to %APPDATA%/CopySpeak/config.json — existing
- ✓ Configuration validation (ranges, hotkey format, command templates) — existing
- ✓ System tray: Toggle Listening, Speak Now, Settings, Quit — existing
- ✓ Main window: Status view (listening state, last spoken, Pause/Speak Now) — existing
- ✓ Main window: Settings view (trigger, TTS, playback, HUD, general) — existing
- ✓ Speech history persistence (circular buffer, 1000 entries) — existing
- ✓ Brutalist UI design with dark/light mode toggle — existing
- ✓ Windows auto-start via registry — existing
- ✓ Audio controls: volume, speed hotkeys, skip forward/backward — existing
- ✓ Save audio to file with date/time/hash filename patterns — existing
- ✓ SSML markup support — existing
- ✓ Max text length enforcement (default 50,000 chars) — existing
- ✓ Comprehensive error logging with rotation (10MB, 5 files) — existing
- ✓ Debug mode with verbose IPC/clipboard/TTS logging — existing
- ✓ Engine route: health check UI — Phase 3
- ✓ Inline install guidance for broken CLI engines — Phase 3
- ✓ Navigation: Play, Engine, Settings tabs — Phase 1
- ✓ Engine page: backend selector, credentials, voice selection — Phase 2
- ✓ Two-stage health checks for API engines — v0.1
- ✓ ElevenLabs voice dropdown from API — v0.1
- ✓ OpenAI 9-voice dropdown — v0.1

### Active

- [ ] Startup engine health check — detect if TTS engine is configured/working on launch; show setup prompt if broken
- [ ] First-run onboarding with config detection — redirect to setup if no config exists
- [ ] Non-blocking onboarding — user can skip and use app immediately
- [ ] CLI preset auto-apply — selecting preset should populate command/args

### Out of Scope

- HUD overlay / waveform visualization — deferred to features-extras branch
- Global hotkeys — deferred to features-extras branch
- Voice presets manager — not in this milestone
- Speech history viewer UI — log exists but no browse UI yet
- Batch TTS processing — deferred
- Application-specific whitelist/blacklist — deferred

## Context

- Stack: Tauri v2 (Rust backend) + Svelte 5 + SvelteKit + Tailwind CSS v4 + shadcn-svelte
- IPC: commands.rs → main.rs → frontend via @tauri-apps/api
- State: Mutex<T> via Tauri's app.manage()
- ~23,000 LOC across Rust, TypeScript, and Svelte
- v0.1 shipped March 2026: HTTP engine removed, CLI consolidated, two-stage health checks

## Constraints

- **Platform**: Windows only — uses Win32 clipboard APIs; no cross-platform requirement
- **Tech Stack**: Must remain Tauri v2 + Svelte 5; no framework changes
- **Lightweight**: App should stay minimal in tray; no background CPU drain

## Key Decisions

| Decision                                | Rationale                                                 | Outcome   |
| --------------------------------------- | --------------------------------------------------------- | --------- |
| Double-copy trigger (not hotkey)        | Zero-friction; no shortcut to memorize                    | ✓ Good    |
| Deferred HUD/hotkeys to features-extras | Keep core clean for stable release                        | — Pending |
| Brutalist UI design                     | Distinctive aesthetic, hard edges, muted palette          | ✓ Good    |
| Dedicated Engine route (v0.1)           | Engine config too complex for Settings; deserves own page | ✓ Good    |
| Remove HTTP engine                      | Minimal usage, maintenance burden                         | ✓ Good    |
| Consolidate CLI to 3 presets            | Reduce confusion, focus support                           | ✓ Good    |
| Two-stage health checks                 | Separate credential validation from synthesis             | ✓ Good    |
| ElevenLabs voice dropdown only          | API fetch is reliable; raw ID is error-prone              | ✓ Good    |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):

1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):

1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---

_Last updated: 2026-03-26 after v0.2 milestone planning started_
