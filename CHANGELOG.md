# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Voice profiles** — Added a profile layer (`VoiceProfile`/`ProfileEffects`) bundling `engine + voice + speed + pitch + effect` as one swappable unit, surfaced as a dedicated **Profiles** tab/route with a compact manager (select, rename, duplicate, delete, import/export JSON). Synthesis resolves an `EffectiveTtsRequest` from the active profile; the migrated `default` profile is a passthrough for the existing engine tabs.
- **Versioned TTS config + migration** — Added `schema_version` to `TtsConfig` and `migrate_tts_config`, which folds a legacy single-engine config into one `default` profile on load.
- **Google Gemini TTS backend** — Added `tts/google.rs` (`GoogleTtsBackend`) calling the Gemini `generateContent` AUDIO API and wrapping returned base64 PCM into WAV.
- **Microsoft MAI-Voice-2 backend** — Added `tts/microsoft.rs` (`MicrosoftTtsBackend`) with a user-configurable endpoint, auto-detecting raw-audio vs base64-JSON responses.
- **First-class HTTP TTS backend** — Added `tts/http.rs` (`HttpTtsBackend`) and `HttpTtsConfig` with templated URL/body (`{text}`, `{raw_text}`, `{voice}`, `{speed}`) for OpenAI-compatible/local TTS servers.
- **`{engine_dir}` CLI placeholder** — `CliTtsBackend` now expands `{engine_dir}` to `%LOCALAPPDATA%\CopySpeak\engines` for uv-managed local engines.
- **uv-based engine installers** — Added `scripts/install-uv.ps1`, `scripts/install-chatterbox.ps1`, `scripts/test-engine.ps1`, and shared `scripts/lib/copyspeak-engine-install.ps1`, plus the stable `scripts/chatterbox/copyspeak-chatterbox.py` wrapper. `uv` is a hard requirement; installers print a profile snippet instead of editing config.
- **Control server profiles** — `POST /speak` now accepts `"profile"` to select a voice profile per request (alongside the existing `engine`/`effect` shorthands).
- **Engine catalog** — Added `tts/catalog.rs` exposing per-engine labels, docs URLs, supported-option descriptors, and static fallback voice lists, surfaced via `list_tts_engines`/`list_tts_voices` IPC, the `/engines` and `/engines/{engine}/voices` control endpoints, and mirrored TypeScript types. The Profiles editor renders engine settings and a catalog/provider voice picker from it.
- **Profile engine options + text processing** — Profiles now carry `description`, `voice_label`, typed `text_processing` (inherit/disabled/enabled + bracketed-emote strategy), and `engine_options` that override global per-engine synthesis settings (model, output format, ElevenLabs voice settings, HTTP url/body/timeout, etc.) via `create_backend_from_effective`. Bracketed-emote handling (`[laughs]`) is deterministic and local.
- **Read/non-mutating control endpoints** — Added `GET /profiles`, `GET /profiles/{id}`, `POST /profiles/active`, `GET /engines`, `GET /engines/{engine}/voices`. `POST /speak` with a profile is request-local by default and only persists the active profile when `"persist_selection": true`. No response exposes API keys.
- **First-party CLI** — Added `scripts/copyspeak.mjs`, a thin Node wrapper over the localhost control server (`health`, `speak`, `profiles list|use|show`, `engines list`, `voices list --engine`).
- **Profile engine docs** — Added `docs/profile-engine-settings.md` documenting the profile-vs-global boundary, engine matrix, and HTTP/CLI semantics.
- **`set_active_profile` IPC** — Added a lightweight profile switch command that validates the profile id, updates the active-profile backend mirror, saves config, and emits `config-changed`.
- **LLM Post-Processing providers** — Added Groq-primary Post-Processing config and settings for OpenAI, Anthropic, Gemini, OpenRouter, Ollama, xAI, AWS Bedrock, Cerebras, and custom OpenAI-compatible endpoints before TTS generation.
- **Post-Processing prompt presets** — Added editable prompt labels, model refresh, concise developer, cleanup, professional, summarize, TTS-optimized, and revised caveman prompts.

### Changed

- **Voice profiles UI** — Moved profiles out of the Engine page into their own `/profiles` route/nav tab, and rebuilt the editor with the shared settings primitives (`SettingRow` + `Select`/`Slider`/`Input`) instead of raw `<select>`/`<input type="range">` controls.
- **`TtsEngine`** — Added `Http`, `Google`, and `Microsoft` variants. The HTTP engine is first-class again; the previous forced `http`→`local` downgrade on config load was removed.
- **Profile-first synthesis** — `speak_now` and `speak_queued` (the clipboard double-copy path) both build the backend from the resolved active profile, so profile `engine_options` are honored for short and paginated/long-text playback alike. The OpenAI backend now respects the requested `voice` instead of always using the global config voice.
- **Profiles drive synthesis end-to-end** — Footer switching now changes profiles instead of engines, `active_backend` is maintained as a derived compatibility mirror, and default profiles are named `Engine - Voice`.
- **Engine page repurposed** — Engine tabs no longer set the active synthesis engine; they are configuration panels for keys, fallback model/output settings, smoke tests, docs, and future installer guidance.

### Fixed

- **Profile-aware backend readers** — `test_tts_engine`, HUD provider/voice display, config validation, and control-server persisted profile selection now resolve from the active profile instead of loose `active_backend` state.
- **Tauri dev startup** — Added a preflight check in `scripts/tauri-dev.mjs` so `bun run tauri dev` now fails with a clear Rust/Cargo install message instead of Tauri's raw `cargo metadata` error when Cargo is missing.
- **Vercel landing page** — Updated the displayed version, screenshot asset, and removed the double-copy hero tagline.

## [0.1.4] - 2026-05-20

### Added

- **CopySpeak Claude Code hook** — Added `scripts/claude-copyspeak-hook.mjs` to speak Claude Code `Stop`/`SubagentStop` assistant responses through the CopySpeak control server.

### Changed

- **CopySpeak Pi extension** — Disabled speaking Pi thinking blocks by default and expanded status text to show only non-default assistant/thinking/activity modes.

### Fixed

- **CopySpeak Pi extension** — Removed the stale `.pi/extensions/copyspeak-voice` extension so only `/copyspeak` is registered.
- **Vercel deployments** — Added a repository `ignoreCommand` that runs production builds and skips preview builds.

## [0.1.3] - 2026-05-19

### Added

- **Update controls in settings** — Added the footer update status/check/install control below the automatic update-check setting.

### Fixed

- **CopySpeak Pi extension** — Renamed the Pi command/extension path to `copyspeak` and shortened its Pi status text to `on`/`off`.
- **Vercel landing page** — Re-enabled non-English locale registration and footer language switching, and restored page scrolling despite the desktop app's global hidden body overflow.
- **Windows audio wake-up** — Add a low-level preroll to desktop playback on Windows so the audio device wakes before speech or radio effects begin.
- **About settings layout** — Removed the stale import/export separator and aligned About rows with the shared `SettingRow` spacing.

## [0.1.2] - 2026-05-18

### Added

- **Audio Effects system** — Frontend-only post-processing applied to TTS playback
  - New `EffectsConfig` (Rust + TS) persisted in `AppConfig` with `enabled` and `active_effect`
  - New Effects settings tab and conditional main-menu Effects tab (gated by `effects.enabled`)
  - New `/effects` route with live effect selector and preview button
  - **Walkie-talkie effect** — Narrow radio EQ, subtle saturation, light AM wobble, normalized PTT clicks, and low static under the voice
  - **8-bit Game Boy effect** — 4-bit sample quantization resampled to 11025 Hz for crunchy retro voice
  - `Effect` interface and registry in `src/lib/stores/playback/effects/` for extensibility
  - Effects render inside `OfflineAudioContext` and integrate with existing pitch-shift pipeline; results cached per `{pitch, effect}` pair

### Changed

- **Unified web and desktop SvelteKit app** — Consolidated the former `src-web` landing page into the main `src` app
  - Added Vercel environment detection via `import.meta.env.VITE_IS_VERCEL`
  - Route layout now renders the marketing landing page on Vercel and the Tauri app shell locally/in desktop builds
  - Removed the redundant `src-web` SvelteKit project

### Fixed

- **CopySpeak Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak Pi extension** — Now speaks only once after an agent run completes and no longer auto-launches CopySpeak unless `COPYSPEAK_PI_LAUNCH=1` is set.
- **CopySpeak Pi extension** — Added a two-minute duplicate speech guard to avoid charging TTS credits for repeated final messages.
- **CopySpeak Pi extension** — Uses the running app's engine/effect settings by default and can include Pi thinking blocks in spoken assistant responses.
- **CopySpeak Pi extension** — Speaks Pi thinking blocks as soon as each thinking block finishes streaming, while avoiding replaying those blocks in the final response.
- **CopySpeak control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.
- **CopySpeak control server** — `/speak` now waits for speech generation to complete before responding, allowing Pi extension requests to queue synthesis instead of overlapping.
- **Playback queue** — Single `audio-ready` events now use the existing fragment queue so Pi-generated thinking and final responses play sequentially instead of interrupting each other.
- **Global playback settings** — Sync playback volume, speed, pitch, and effects during app startup so Pi control-server speech uses the configured walkie-talkie effect outside the Play page.

## [0.1.1] - 2026-05-15

### Added

- **Audio Effects system** — Frontend-only post-processing applied to TTS playback
  - New `EffectsConfig` (Rust + TS) persisted in `AppConfig` with `enabled` and `active_effect`
  - New Effects settings tab and conditional main-menu Effects tab (gated by `effects.enabled`)
  - New `/effects` route with live effect selector and preview button
  - **Walkie-talkie effect** — Narrow radio EQ, subtle saturation, light AM wobble, normalized PTT clicks, and low static under the voice
  - **8-bit Game Boy effect** — 4-bit sample quantization resampled to 11025 Hz for crunchy retro voice
  - `Effect` interface and registry in `src/lib/stores/playback/effects/` for extensibility
  - Effects render inside `OfflineAudioContext` and integrate with existing pitch-shift pipeline; results cached per `{pitch, effect}` pair

- **Cartesia onboarding verification** — Onboarding now accepts a Cartesia API key and validates it via `check_cartesia_credentials` without synthesis.

- **Cartesia TTS backend** — Added Cartesia Sonic 3.5 as a cloud TTS engine
  - Added `CartesiaConfig`, `TtsEngine::Cartesia`, and `CartesiaTtsBackend`
  - Added Cartesia engine settings UI with model, voice ID, and output format controls

### Changed

- **Unified web and desktop SvelteKit app** — Consolidated the former `src-web` landing page into the main `src` app
  - Added Vercel environment detection via `import.meta.env.VITE_IS_VERCEL`
  - Route layout now renders the marketing landing page on Vercel and the Tauri app shell locally/in desktop builds
  - Removed the redundant `src-web` SvelteKit project
- **Default TTS engine** — New configs now default to Cartesia Sonic 3.5 with the Katie voice
- **Default pagination fragment size** — New configs now use `fragment_size: 500`
- **Engine picker order** — Cartesia now appears first in engine settings and footer selector
- **Cartesia voice selection** — Cartesia settings now show resolved voice names with a manual voice ID fallback
- **Onboarding flow** — First-run setup now focuses on Cartesia Cloud instead of local Kitten TTS installation

### Fixed

- **CopySpeak Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.

## [0.1.0] - 2026-03-27

### Added

- **Global hotkey speak-from-clipboard** — Hotkey now triggers TTS directly from clipboard content
  - Added handler in global-shortcut plugin to call `speak_from_clipboard` on hotkey press
  - Logs hotkey trigger events for debugging

- **Dedicated History page** — New `/history` route for viewing all TTS generations
  - Moved history from play page to its own route
  - Conditionally shown in nav when history is enabled

- **SettingRow component** — Reusable settings row with label, tooltip, and consistent layout
  - Applied across all settings components for uniform UI

- **Live debug logs viewer** — Real-time log tail in About section when debug mode enabled
  - Shows last 20 lines, auto-refreshes every 2s

### Fixed

- **CopySpeak Pi extension** — Reworked clipboard triggering to serialize double-copy events and avoid repeated trigger loops; startup now avoids focusing an already-running CopySpeak instance.

- **Windows CLI backend PATH resolution** — Expanded PATH for finding Python/uv tools on Windows
  - Added `get_expanded_path()` to include common Python and uv installation paths
  - Fixes "executable not found" errors on clean Windows installations

### Changed

- **Settings page consolidation** — Major restructure from 8 sections to 3 tabs (General, Advanced, About)
  - Continuous scroll with scroll-spy navigation
  - Removed staggered loading (WebView2 crash workaround no longer needed)
  - HUD settings moved to General section as dropdown
  - Pagination/Sanitization moved to Advanced tab
- **Window size increased** — 675x540 → 775x640 for better content visibility
- **Hotkey capture redesign** — Cleaner UI with Kbd components and arrow key symbols (↑↓←→)
- **Quick-settings redesign** — Larger controls with clearer labels (Volume, Speed, Pitch)
- **App shell refactor** — Grid-based layout for better content distribution
- **Removed `show_notifications`** config field — Unused setting cleaned up
- **Default hotkey shortcut** — Changed from `Super+Shift+A` to `Win+Shift+A` for Windows clarity
- **Hotkey error messages** — Updated to use "Win" instead of "Win/Super" for consistency
- **Hotkey logging** — Added structured logging with `[Hotkey]` prefix for registration attempts and config changes
- **Border radius system** — Simplified radius variables for sharper brutalist aesthetic
  - `--radius-sm: 2px`, `--radius-md: var(--radius)`, `--radius-lg: 4px`, `--radius-xl: 6px`
  - Theme toggle and UI components updated to use `rounded-sm` instead of `rounded-none`
- **Logging noise reduction** — Suppressed verbose debug logs from tauri_plugin_updater and reqwest
- **Engine page layout refactor** — Moved badges to header section for cleaner UI
- **Progress bar animation** — Converted from JavaScript interval to CSS animation for smoother performance
- **Default Kokoro voice** — Changed from `af_heart` to `adam`
- **Internationalization** — Temporarily disabled language switcher, hardcoded to English during development

## [0.0.5] - 2026-03-24

### Added

- **Global hotkey configuration** — Configurable keyboard shortcut to trigger TTS
  - `hotkey` config field with modifier + key format (e.g., `"Ctrl+Space"`)
  - Hotkey capture component in settings UI
  - Backend IPC: `register_hotkey` with global-shortcut plugin
  - Hotkey re-registration on config change

- **Listening toggle** — Enable/disable clipboard monitoring via `listen_enabled` config
  - Toggle in quick-settings dropdown and app-footer
  - Backend IPC: `set_listening`, `get_listening` commands
  - Persisted to config, synced via `config-changed` event

### Fixed

- **HUD progress bar and marquee timing** — Accurate playback duration via cross-window event
  - HUD window and main window have separate JS contexts with separate `hudStore` instances
  - `playbackStore` in main window decodes audio via Web Audio API to get accurate duration
  - Emits `hud:audio-duration` event which HUD window receives and updates its `hudStore`
  - Progress now shows accurate percentage based on `AudioBuffer.duration`
  - Marquee animation timing now matches actual playback duration
  - ElevenLabs MP3 duration now accurately determined via Web Audio decode (not server estimate)

- **Audio playback on clean Windows 11** — AudioContext now resumes if suspended
  - Web Audio API requires user gesture to activate AudioContext on fresh profiles
  - Added `audioCtx.resume()` call when state is "suspended" in playback-store

## [0.0.3] - 2026-03-22

### Fixed

- **KittenTTS installer** now works on clean Windows 11 without Python pre-installed
  - Embeds installer scripts in binary and extracts to temp directory at runtime
  - Auto-detects any Python 3.x version, offers winget installation if not found
  - PowerShell window now visible with success/failure feedback before pause
  - Default config now uses `py -3.12` to ensure kittentts runs on same Python version used by installer
  - Health check detects `ModuleNotFoundError` with actionable error message
  - Fixed health check using invalid voice "test" instead of "Rosie"

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

[Unreleased]: https://github.com/ilyaizen/copyspeak/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/ilyaizen/copyspeak/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ilyaizen/copyspeak/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ilyaizen/copyspeak/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ilyaizen/copyspeak/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ilyaizen/copyspeak/compare/v0.0.5...v0.1.0
[0.0.5]: https://github.com/ilyaizen/copyspeak/compare/v0.0.3...v0.0.5
[0.0.3]: https://github.com/ilyaizen/copyspeak/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/ilyaizen/copyspeak/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/ilyaizen/copyspeak/releases/tag/v0.0.1
