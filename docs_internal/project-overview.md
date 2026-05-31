# CopySpeak Project Overview

**Last Updated:** 2026-03-25

## What This Is

CopySpeak is a lightweight Windows desktop application that wraps AI Text-to-Speech engines. It silently monitors the clipboard and reads text aloud when the user copies the same text twice in quick succession (double-copy trigger) or via a global hotkey. It runs in the system tray, staying out of the way until needed.

## Core Value

**Double-copy → instant speech must be flawless.** If the trigger misfires or the voice takes too long, the app is useless.

## Stack

- **Frontend**: Svelte 5 + SvelteKit + Tailwind CSS v4.2 + shadcn-svelte
- **Backend**: Rust (Tauri v2)
- **IPC**: `commands.rs` → `main.rs` → frontend via `@tauri-apps/api`
- **State**: `Mutex<T>` via Tauri's `app.manage()`

## Constraints

- **Platform**: Windows only — uses Win32 clipboard APIs; no cross-platform requirement
- **Tech Stack**: Must remain Tauri v2 + Svelte 5; no framework changes
- **Lightweight**: App should stay minimal in tray; no background CPU drain

## Current State

The app is at a pre-production v0.0.x state — core clipboard-to-speech flow is complete and working. Phase 9 (TTS Engine Overhaul) was recently completed, consolidating engines and removing HTTP backend.

## Key Decisions

| Decision                         | Rationale                                                                       | Status               |
| -------------------------------- | ------------------------------------------------------------------------------- | -------------------- |
| Double-copy trigger (not hotkey) | Zero-friction; no shortcut to memorize                                          | ✓ Good               |
| HUD overlay with waveform        | Real-time visual feedback during playback and clipboard operations              | ✓ Implemented        |
| Brutalist UI design              | Distinctive aesthetic, hard edges, muted palette                                | ✓ Good               |
| Dedicated Engine route           | Engine config too complex for Settings; deserves own page                       | ✓ Complete           |
| Remove HTTP TTS backend          | Simplify engine abstraction; focus on proven services (CLI, OpenAI, ElevenLabs) | ✓ Complete (Phase 9) |
| Consolidate CLI engines          | Reduce maintenance burden; standardize on piper, kokoro-tts, qwen3-tts          | ✓ Complete (Phase 9) |

## Existing Documentation

- **[Architecture](architecture.md)**
- **[Requirements & Traceability](requirements.md)**
- **[Development Guide](development_guide.md)**
- **[TTS Backends](tts_backends.md)**
- **[Brutalist Design](brutalist_design.md)**
- **[Roadmap](roadmap.md)**

## Deferred Features (`features-extras` branch)

- Global hotkeys
- Voice presets manager
- Batch TTS processing
- Application-specific whitelist/blacklist
