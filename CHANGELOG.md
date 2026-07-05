# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Engines page restored (`/engines`)** — dedicated surface for per-engine setup, decoupled from voice profiles. Hosts API key entry (+ endpoint for Microsoft), local-engine installers (kitten, piper, kokoro, pocket, chatterbox, uv), engine **Test** buttons (via `test_tts_engine_config`), and docs links. Sidebar groups Cloud and Local engines; uv-missing banner shown when the local prerequisite is absent.
  - New `routes/engines/+page.svelte`, `components/engine/engine-setup.svelte` (orchestrator), `components/engine/engine-panel.svelte` (presentational card), and `components/engine/engine-meta.ts` (single source of truth for setup metadata — credentials, installer ids, docs).
  - "Engines" nav item restored in `app-header.svelte`.

- **Screenshot capture script** — `scripts/capture-screenshot.mjs` reads version from `tauri.conf.json`, captures the Tauri window via `screenshot-window.ps1`, saves to `static/screen-v{version}.png`, and patches `screenshots.svelte` to reference the new file. One-command screenshot refresh: `node scripts/capture-screenshot.mjs`.

### Changed

- **Voices page (`/voices`) redesigned** — `profile-manager.svelte` restructured from a flat list into grouped cards (Identity, Engine & Voice, Sound, Advanced). Credentials no longer live here; the Engine row shows a passive "Set up engine credentials" hint linking to `/engines` when the active profile's engine key is missing.

- **Credential persistence fixed** — the per-engine config structs (`OpenAIConfig`, `ElevenLabsConfig`, `CartesiaConfig`, `GoogleTtsConfig`, `MicrosoftTtsConfig`) were `#[serde(skip_serializing)]` at the field level in `TtsConfig`, so API keys vanished on restart. Now only `api_key`/`endpoint` persist; profile-owned knobs (model, voice, format, etc.) remain skip-serialized per the profile/global boundary in `docs/profile-engine-settings.md`.

- **Landing screenshot updated** — `screenshots.svelte` now references `screen-v0.1.7.png` (was stale `screen-v0.1.4.png`). Fresh screenshot captured from the v0.1.7 Play page.

### Removed

- **`voice-credentials.svelte` deleted** — its contextual-credential UX is replaced by the Engines page. Setup metadata consolidated into `engine-meta.ts` (DRY).
- **Empty `/engine` and `/profiles` route directories removed** (left behind by the prior consolidation).

### Fixed

- **Screenshot script window title** — `screenshot-window.ps1` defaulted to `"CopySpeak TTS"` but the actual Tauri window title is `"CopySpeak"`. Capture would always fail unless the title was passed manually. Fixed default.

- **Local CLI engine wrapper paths** — pre-v0.1.8 local profiles (kitten, piper, chatterbox) stored the engine wrapper as a CWD-relative path (`scripts/copyspeak-<engine>.py`), which broke because the Tauri process CWD is `src-tauri/` (dev) or the install dir (packaged), not the engine install dir. Migration in `config/tts.rs::migrate_tts_config` now rewrites bare legacy wrapper paths to `{engine_dir}/<engine>/scripts/...` on every load (idempotent). `install-chatterbox.ps1`'s emitted `profileJson` was also missing `command`/`args_template`, so the printed snippet was non-functional — now baked in with the correct absolute path.

## [0.1.7] - 2026-07-05

### Added

- **Voice profiles system** — Create, edit, and switch between named voice profiles, each with its own engine, voice, speed, pitch, and effects settings.
  - New `VoiceProfile` and `ProfileEffects` types; `TtsConfig` now carries `active_profile_id` and `profiles`.
  - New Profiles page (`/profiles`) with inline profile manager.
  - New `speak_now_with_profile` Tauri command registered in `main.rs`.
  - Profiles nav item added to app header.

- **Expanded TTS engine types** — Added `EdgeTtsConfig`, `GoogleTtsConfig`, `MicrosoftTtsConfig`, and `HttpTtsConfig` to `TtsConfig`; `TtsEngine` union extended with `"edge"`, `"google"`, `"microsoft"`, and `"http"`.

- **Engine catalog types** — `EngineCatalogEntry`, `VoiceCatalogEntry`, `EngineOptionDescriptor` interfaces for server-driven engine metadata.

- **Centralized save bar** — Shared `save-bar.svelte.ts` store replaces per-page save bar markup in settings, profiles, and engine pages. Single save bar rendered in `+layout.svelte`.

- **Page motion transitions** — `MotionWrapper` component with fade+slide-up entrance animation on route changes; respects `prefers-reduced-motion` and a `motion-disabled` class.

- **Portal utility** — `portal()` action in `utils.ts` teleports a node to `<body>` so `position:fixed` escapes transformed ancestors.

- **Granular markdown sanitization toggles** — Each markdown strip feature (code blocks, inline code, headers, links, bold/italic, lists, blockquotes) can now be individually enabled/disabled in Settings → Sanitization. Inline code stripping defaults to off to preserve backtick-wrapped terms in technical text.

- **Post-processing providers expanded** — Added `xai`, `aws`, and `cerebras` to `PostProcessingProvider`; new `PostProcessingPromptPreset` type and `selected_prompt_label` / `prompt_presets` fields in `PostProcessingConfig`.

### Changed

- **Markdown stripping respects config** — `strip_markdown()` now accepts `MarkdownSanitizationConfig` and skips disabled features instead of always stripping everything.
- **Import/export settings refactored** — Internal cleanup of dialog state and validation flow.
- **Pi extension** — Removed unused `prepareText` wrapper; switched from custom `parseJson` to `JSON.parse`.

### Fixed

- **`speak_now_with_profile`** — Now exposed as a `#[tauri::command]` (was `pub(crate)`) so the frontend can invoke it.
- **Rust compiler warnings** — Added `#[allow(dead_code)]` on post-processing structs/functions not yet wired to the UI.
- **Devtools and browser flags** — Main window now enables devtools and sets `--force-prefers-no-reduced-motion`, `--enable-smooth-scrolling`, and file-access flags for local dev.

## [0.1.6] - 2026-07-04

### Changed

- **Product naming reverted to `CopySpeak`** — Dropped the `-tts` suffix and lowercasing introduced in the 0.1.5 rename. User-facing strings, package identity, and bundle metadata now use `CopySpeak` consistently.
  - `package.json` and `src-tauri/Cargo.toml` package name restored to the `CopySpeak`/`copyspeak` identity.
  - `src-tauri/tauri.conf.json`: `productName`, `publisher`, and main window `title` → `CopySpeak`.
  - Landing page (hero, footer, screenshots), in-app header, browser title, onboarding, settings tooltips, and HTML history-export titles now read `CopySpeak`.
  - Locale strings in `en.json` updated (`landing.hero.title`, `screenshots.*`, onboarding/welcome, about, app title, OpenAI engine detail).
  - `scripts/claude-copyspeak-hook.mjs` running-exe matcher updated for the new `CopySpeak.exe` bundle name.
- **Tagline now visible** — `Modern AI TTS Orchestrator` (`header.tagline`) is rendered under the in-app header title (was commented out) and added as the landing hero subtitle.

### Fixed

- **CI release build (Rust compile)** — Resolved 24 compile errors blocking `tauri build`:
  - Wired up `config::post_processing` module (declared + re-exported in `config/mod.rs`) so `LlmProviderConfig` and `PostProcessingProvider` resolve from `commands/config.rs`.
  - Removed 4 dead command registrations in `main.rs` (`get_data_dir`, `get_home_dir`, `get_installer_script_path`, `run_kittentts_installer`) — no definitions, no frontend callers.
  - Added missing `let eff = resolve_effective(&tts_config)` + `voice` bindings in `speak_now` and `speak_queued` (`commands/tts/synthesis.rs`) where `eff`/`voice` were referenced but never bound.

### Fixed

- **CI release build** — Removed stale `bundle.resources` entries in `tauri.conf.json` (`../install-kittentts.ps1`, `../kittentts-cli.py`) that referenced non-existent repo-root files and aborted the bundler. Engine installer scripts are resolved at runtime under `scripts/` via `install_engine`.

### Changed

- **Engine page refactor** — Consolidated `engine-page.svelte` from per-engine subcomponents into a single data-driven panel driven by `ENGINE_TABS`. Removed the cloud-TTS API-key dialog, credential check/test helpers, and unused category metadata; engine settings are now edited inline with a single save flow.
  - Added `placeholderKey` per engine tab and `install_engine`/`check_command_exists` (uv) wiring for local engines.
  - Added English locale strings for engine API-key placeholders.

## [0.1.5] - 2026-05-20

### Added

- **LLM post-processing (Groq Cloud)** — Optional pass between sanitize and TTS synthesis that rewrites copied text into concise, listener-friendly speech tailored for software developers. Off by default. Configure under Settings → Advanced → LLM Post-Processing.
  - New `PostProcessConfig` (`enabled`, `api_key`, `model`, `prompt`) in `AppConfig`; config schema version bumped to `0.1.5`.
  - New Rust module `post_process` (`process`, `try_process`) wraps Groq's OpenAI-compatible `/chat/completions`.
  - New IPC command `check_groq_credentials` validates the key via `GET /models`.
  - Hooked into `speak_now` and `speak_queued` after the cfg snapshot, before pagination. LLM failures fall back to the original text and never block synthesis.
  - Hardcoded model dropdown: `openai/gpt-oss-20b`, `llama-3.3-70b-versatile`, `llama-3.1-8b-instant`.

### Changed

- **LLM post-processing default prompt** — Switched to a terse caveman-style rewrite prompt with a 3 bullet/point maximum.

### Fixed

- **CopySpeak TTS Pi extension** — Routes final Pi responses through the running app's sanitization, max-length, LLM post-processing, effects, and TTS pipeline instead of filtering/truncating in the extension.
- **Vercel landing page** — Updated the displayed version, screenshot asset, and removed the double-copy hero tagline.

## [0.1.4] - 2026-05-20

### Added

- **CopySpeak TTS Claude Code hook** — Added `scripts/claude-copyspeak-hook.mjs` to speak Claude Code `Stop`/`SubagentStop` assistant responses through the CopySpeak TTS control server.

### Changed

- **CopySpeak TTS Pi extension** — Disabled speaking Pi thinking blocks by default and expanded status text to show only non-default assistant/thinking/activity modes.

### Fixed

- **CopySpeak TTS Pi extension** — Removed the stale `.pi/extensions/copyspeak-voice` extension so only `/copyspeak` is registered.
- **Vercel deployments** — Added a repository `ignoreCommand` that runs production builds and skips preview builds.

## [0.1.3] - 2026-05-19

### Added

- **Update controls in settings** — Added the footer update status/check/install control below the automatic update-check setting.

### Fixed

- **CopySpeak TTS Pi extension** — Renamed the Pi command/extension path to `copyspeak` and shortened its Pi status text to `on`/`off`.
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

- **CopySpeak TTS Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak TTS control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak TTS Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak TTS Pi extension** — Now speaks only once after an agent run completes and no longer auto-launches CopySpeak TTS unless `COPYSPEAK_PI_LAUNCH=1` is set.
- **CopySpeak TTS Pi extension** — Added a two-minute duplicate speech guard to avoid charging TTS credits for repeated final messages.
- **CopySpeak TTS Pi extension** — Uses the running app's engine/effect settings by default and can include Pi thinking blocks in spoken assistant responses.
- **CopySpeak TTS Pi extension** — Speaks Pi thinking blocks as soon as each thinking block finishes streaming, while avoiding replaying those blocks in the final response.
- **CopySpeak TTS control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.
- **CopySpeak TTS control server** — `/speak` now waits for speech generation to complete before responding, allowing Pi extension requests to queue synthesis instead of overlapping.
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

- **CopySpeak TTS Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak TTS control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak TTS Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak TTS control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.

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

- **CopySpeak TTS Pi extension** — Reworked clipboard triggering to serialize double-copy events and avoid repeated trigger loops; startup now avoids focusing an already-running CopySpeak TTS instance.

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

[Unreleased]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.7...HEAD
[0.1.7]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.5...v0.1.0
[0.0.5]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.3...v0.0.5
[0.0.3]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/ilyaizen/CopySpeak/releases/tag/v0.0.1
