# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

General desktop reader on Windows, macOS (Intel), or Linux. They hit long text — articles, documentation, emails — and want to listen while multitasking. Success: text becomes speech without them thinking about the app.

## Product Purpose

CopySpeech reads clipboard text aloud. Copy the same text twice and it speaks, with a floating HUD, word-synced captions, and configurable playback. It orchestrates 12 TTS engines, offline by default. The next release targets low time-to-first-audio (TTFA) and a small VRAM/RAM footprint, so local engines run on low-VRAM GPUs and CPU-only machines.

## Positioning

The double-copy trigger. Copy any text twice within 1.5 s and it speaks — no hotkey, no window focus, no selection dance. A neighboring product cannot truthfully copy this friction-free trigger. Local-by-default voices (Kokoro in one click, no API key) support it but are not the core claim.

## Operating Context

- Desktop app: tray icon, global hotkey, HUD overlay window (XWayland on Wayland; hotkeys are protocol-impossible there).
- Main window resizable from 360 × 400 (initial 775 × 580); HUD is fixed-size. Content scrolls when the window is short.
- Browser companion (Chrome/Edge) reads selections through the app and highlights the passage on the page.
- Local HTTP control server (`/health`, `/speak`) for scripts and external tools.
- 9 interface languages; Hebrew and Arabic mirror right-to-left; tray follows app language.
- Dev loop: `bun run tauri dev`. AGENTS.md governs all work.

## Capabilities and Constraints

- Engines: local — Kokoro (default), Piper, Kitten, Qwen3-TTS, Pocket, tiny-voca, generic CLI; cloud — OpenAI, ElevenLabs, Cartesia, Google, Azure, generic HTTP. New engines are judged by TTFA and VRAM footprint first. Chatterbox was dropped 2026-10-20; do not reintroduce it.
- Streaming playback starts after ~150 ms of buffering; Stop aborts in-flight synthesis.
- Speed 0.25×–4×, pitch 0.5×–2×, volume; output device selection; MP3/OGG/FLAC export; per-app audio ducking on Windows.
- Local engine daemons exit after idle (default 10 min) to free RAM/VRAM.
- Voice profiles bundle engine, voice, speed, pitch, and effects.
- Text pipeline sanitizes markdown, URLs, and unicode without corrupting non-English scripts; optional LLM rewrite pass.
- History can be switched off; a small recent-clips cache still skips re-synthesis.
- Decisions: no formal accessibility target recorded (2026-11-13); current practice is keyboard nav, focus rings, and theme contrast.

## Brand Commitments

- Name: CopySpeech. Tagline in use: "Your clipboard, read aloud."
- Logo: `static/app-logo.svg`.
- License: MIT.

## Evidence on Hand

- README feature checklist and platform table (repo root).
- App screenshot: `static/screen-0.3.1-light.png` and `static/screen-0.3.1-dark.png`.
- Incumbent visual spec: `DESIGN.md` (muted-slate brutalist; `src/routes/+layout.css` is the token source of truth).
- Absence: no testimonials, press, pricing, or user studies. Future work must not fabricate them.

## Product Principles

1. Speech before setup — zero configuration to first audio; depth lives behind the surface.
2. TTFA is a product metric — the reading must start while attention still holds.
3. Local by default — no API key to hear text; cloud engines are opt-in.
4. Trust the text — sanitization never corrupts non-English scripts or reading order.
5. Small footprint — low-VRAM GPUs and CPU-only machines are first-class targets.

## Accessibility & Inclusion

No formal standard recorded. Confirmed commitments: RTL mirroring for Hebrew and Arabic, 9-language interface, keyboard navigation, and dark/light themes.
