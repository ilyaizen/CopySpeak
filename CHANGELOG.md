# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Web deployment support** — Vercel configuration for src-web standalone SvelteKit site
  - `.vercelignore` to exclude Tauri/Rust build artifacts from deployment uploads
  - `.vercel` added to `.gitignore`
  - Language switcher component for English/Spanish in footer
  - Locale persistence in localStorage for web version

### Fixed

- svelte-i18n initialization error on static site build - wait for locale loading before render

## [0.0.2] - 2026-03-21

### Added

- **HUD playback enhancements**
  - Progress bar animation synced to audio duration
  - Marquee scrolling text for long speech content
  - `duration_ms` field in `HudSynthesizingPayload` for synthesis duration tracking

### Fixed

- Removed duplicate `$effect` in hud-playback-content component
- Removed debug `console.log` statement from production code

## [0.0.1] - 2026-03-20

### Added

- **Core TTS functionality** — Clipboard-triggered text-to-speech with multiple engine support
  - Double-copy trigger: copy twice within 1.5s to speak selected text
  - Hotkey trigger: configurable keyboard shortcut
  - Manual trigger: paste/play from UI

- **Multiple TTS engines**
  - **Kitten TTS** (default): Ultra-lightweight CPU-optimized ONNX inference, 8 built-in voices
  - **Piper TTS**: Local CLI engine with 20+ EN US voices
  - **Kokoro TTS**: Local CLI engine with multiple voices
  - **OpenAI TTS**: Cloud API with 9 voices (alloy, ash, coral, echo, fable, onyx, nova, shimmer, verse)
  - **ElevenLabs TTS**: Cloud API with voice library support

- **HUD overlay** — Floating heads-up display showing playback status, waveform visualization, and engine info
  - Real-time waveform visualization with 16-bar equalizer
  - Progress tracking for paginated synthesis
  - Click-through transparent overlay

- **History management** — Persistent history of TTS generations with playback
  - Audio files saved in native format (WAV/MP3/OGG/FLAC)
  - Fragmented copy grouping for paginated text
  - Batch playback and deletion

- **Settings system**
  - General: auto-start, debug mode, language (EN/ES with full i18n support)
  - Playback: speed (0.25x–4x), pitch (0.5x–2x), volume
  - Triggers: double-copy window, hotkey configuration
  - Sanitization: markdown stripping, text normalization

- **Auto-updater** — Check and install updates from GitHub Releases

- **Internationalization (i18n)** — Full localization with English and Spanish support, RTL layout ready

### Breaking Changes

- **HTTP TTS engine removed** — HTTP endpoint backend removed in favor of CLI and cloud engines
- **SSML support removed** — SSML markup passthrough feature removed
- **Streaming TTS mode removed** — Simplified to paginated synthesis only

[Unreleased]: https://github.com/ilyaizen/copyspeak/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/ilyaizen/copyspeak/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/ilyaizen/copyspeak/releases/tag/v0.0.1
