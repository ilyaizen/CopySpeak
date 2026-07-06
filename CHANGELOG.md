# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.9] - 2026-07-06

### Added

- **`has_engine_credentials` command** — New IPC command checks whether an engine has credentials available (config.json or .env) without making HTTP requests. Used by the profile manager to show/hide the "Set up engine credentials" hint accurately.

- **Config-changed event listeners** — Play page and Voices page now listen for the `config-changed` Tauri event and reload config automatically when it changes externally.

### Changed

- **Voice label priority** — `voice_display_name` now prioritizes the profile's `voice_label` from the catalog over the raw `voice_name` from config, giving cleaner filenames in history.

- **Voice label backfill** — Default profile and migration (`migrate_tts_config`) now populate `voice_label` from the voice catalog for profiles that have none.

- **HUD window deferred show** — HUD window starts with `visible: false` in `tauri.conf.json` and explicitly calls `window.show()` in `onMount` after transparent CSS is applied, preventing a white flash.

- **Credential hint uses backend check** — Profile-manager now calls `has_engine_credentials` (covers config + .env) instead of only checking the raw `config.json` api_key field.

### Fixed

- **Play-page reload→save→emit loop** — Added `externalLoad` guard in the config `$effect` to skip auto-save when config was loaded via the `config-changed` event, breaking the infinite reload→save→emit→reload cycle.

- **Footer voice label rendering** — Switched from `||` to `?? null` so legitimately empty-string `voice_label` values are preserved instead of falling through to `voice`.

### Added

- **Searchable voice picker** — the flat per-engine voice `<Select>` in the profile editor is replaced by `voice-picker.svelte`: a portaled popover with a live search box, automatic grouping (by gender for OpenAI/Google/Cartesia/ElevenLabs, by BCP-47 locale for Edge), inline metadata (`gender · language`) and a check mark on the active voice. Escape / outside-click closes it; the panel flips above the trigger when space below is tight. The manual Voice ID input remains below it as the escape hatch.

- **Cartesia voice refresh** — `supports_voice_refresh` is now `true` for Cartesia. New `CartesiaTtsBackend::list_voices()` calls `GET https://api.cartesia.ai/voices` (`X-API-Key` + `Cartesia-Version`) and maps results into `VoiceCatalogEntry`; on API failure `list_tts_voices` falls back to the static catalog list (mirroring the ElevenLabs pattern). The picker's Refresh button covers both ElevenLabs and Cartesia.

- **`.env` secret loading** — CopySpeak now reads a `.env` file placed next to `copyspeak.exe` (in dev: `src-tauri/target/debug/`). Keys defined there override the values typed in the Engines UI; UI-typed keys in `config.json` remain the fallback. Env values are never written back to disk. See `.env.example` for the full variable list (`OPENAI_API_KEY`, `ELEVENLABS_API_KEY`, `CARTESIA_API_KEY`, `GEMINI_API_KEY`/`GOOGLE_API_KEY`, `MICROSOFT_API_KEY`/`AZURE_API_KEY` + `MICROSOFT_ENDPOINT`, `POST_PROCESS_API_KEY`).
  - New `secrets.rs`: `load_dotenv()` (naive `KEY=VALUE` parser, called once after logging init in `main.rs`) and `resolve(config_val, env_names)` (env-wins resolution with alias support, e.g. Gemini accepts both `GEMINI_API_KEY` and `GOOGLE_API_KEY`).
  - All credential read-sites now route through `secrets::resolve`: the five TTS backends (`openai`, `elevenlabs`, `cartesia`, `google`, `microsoft`), the three cloud credential-check commands, and the live Groq post-processing path (`post_process::process`).

### Changed

- **`VoiceCatalogEntry` gains a `gender` field** (`Option<String>`, serialized as `gender`) surfaced in `src/lib/types.ts`. `catalog.rs::voice()` helper now takes a `gender` argument.
- **Enriched static voice metadata:**
  - OpenAI (11 voices) — labels capitalized, gender + concise style description per voice.
  - Google Gemini (29 voices) — gender + Google's documented style descriptor per voice; grouped Female/Male in the picker.
  - Cartesia (2 static voices) — gender + description; language set to `None` (multilingual).
  - Edge (30 voices) — `language` carries the BCP-47 locale parsed from the voice id (`en-US`, `en-GB`, …) and `gender` is now populated from the live `edge-tts --list-voices` metadata (Microsoft's published genders). The picker groups Edge by **locale** (region is the more useful cluster) with gender shown as per-row meta; cloud engines still group by gender. `en-US-DavisNeural` and `en-US-AmberNeural` are absent from the current live list (likely deprecated) and keep `gender: None`.
  - ElevenLabs — static catalog expanded from 1 (Rachel) to **21 real premade voices**. IDs/names/genders fetched from `GET /v1/voices` (2026-07-06), each with an `"{accent} · {gender} · {style}"` description; `language` set to `None` since gender is the picker group key (avoids a redundant per-row `en`). Rachel was rotated out of the premade set and is dropped — still usable via the manual Voice ID escape hatch.

### Fixed

- **ElevenLabs voice metadata** — `list_tts_voices` now propagates the `gender` label from the ElevenLabs API into the catalog entry (previously dropped).

## [0.1.8] - 2026-07-05

### Added

- **Engines page restored (`/engines`)** — dedicated surface for per-engine setup, decoupled from voice profiles. Hosts API key entry (+ endpoint for Microsoft), local-engine installers (kitten, piper, kokoro, pocket, chatterbox, uv), engine **Test** buttons (via `test_tts_engine_config`), and docs links. Sidebar groups Cloud and Local engines; uv-missing banner shown when the local prerequisite is absent.
  - New `routes/engines/+page.svelte`, `components/engine/engine-setup.svelte` (orchestrator), `components/engine/engine-panel.svelte` (presentational card), and `components/engine/engine-meta.ts` (single source of truth for setup metadata — credentials, installer ids, docs).
  - "Engines" nav item restored in `app-header.svelte`.

- **Test buttons for local engines** — every local engine panel (piper, kokoro, kitten, chatterbox, pocket) now shows a Test button that runs a real synthesis ("Hello.") through the engine's CLI wrapper and verifies the bytes are a valid audio file. New `test_local_engine` Tauri command (`commands::tts::health`) builds a `CliTtsBackend` from each installer's stable `{command, args_template}` and calls `synthesize()`. `engine-panel.svelte` Test block no longer gated on `kind === "cloud"`; `engine-setup.svelte::runTest` branches to `runLocalTest` for local entries.
  - Reuses the legacy `engine.localEngine.testEngine/engineWorking/engineFailed` i18n keys.

- **Interactive English voice selection in installers** — every local-engine installer now prompts (in its console window) to pick an English voice and bakes it into the smoke test and emitted profile snippet:
  - `install-piper.ps1`: numbered menu of en_US voices (amy, lessac, ryan, joe, libritts); downloads the chosen `.onnx` + `.onnx.json` pair from `huggingface.co/rhasspy/piper-voices` into `voices\` if missing. New `-SkipVoiceDownload` switch.
  - `install-kokoro.ps1`: menu of built-in English voices (af_heart, af_bella, af_nicole, af_sarah, am_adam, am_michael, bf_emma, bm_george); default `af_heart`.
  - `install-kittentts.ps1`: menu of the 8 built-in voices (Rosie, Bella, Luna, Kiki, Jasper, Bruno, Hugo, Leo); default `Rosie`.
  - New shared helpers `Select-VoiceFromMenu` and `Get-Confirmation` in `scripts/lib/copyspeak-engine-install.ps1`; the latter replaces the unused `Confirm-Install`.
  - All installers now prompt interactively for reinstall (force) when not passed `-Force`; default is No (no forced reinstall).

### Fixed

- **Piper installer `uv` self-dependency collision** — `New-EngineProject` in `scripts/lib/copyspeak-engine-install.ps1` ran `uv init --bare` with no `--name`, so the project was named after the directory basename. For piper this collided with the PyPI package name (`piper`), making `uv add piper` fail with _"Requirement name `piper` matches project name `piper`, but self-dependencies are not permitted"_ (exit code 2). Fixed by passing `--name copyspeak-<dir>` to `uv init`. Harmless for chatterbox/kitten whose dir names already differ from their PyPI package names.

- **Piper voice model not found** — `scripts/piper/copyspeak-piper.py` resolved `voices_dir` as `Path(__file__).parent / "voices"`, but the wrapper lives in `<engine_dir>/scripts/` while `install-piper.ps1` downloads `*.onnx` into `<engine_dir>/voices/`. So the wrapper looked in `<engine_dir>/scripts/voices/` (always empty) and reported _voice model not found … Available: []_ even though the installer had downloaded the model correctly. Fixed by going up one level: `Path(__file__).parent.parent / "voices"`. Updated the docstring hint to point at `<engine_dir>/voices/`.

- **Piper PyPI package-name collision (`piper` vs `piper-tts`)** — `install-piper.ps1` ran `uv add piper`, but PyPI's `piper` is an unrelated bioinformatics toolkit (_databio/pypiper_, module `pypiper`). The actual TTS engine ships as `piper-tts` (module `piper`). So `from piper import PiperVoice` failed with _No module named 'piper'_ even though `uv add` reported success. Fixed by depending on `piper-tts`. (The earlier `--name copyspeak-<dir>` fix unmasked this: previously the project self-named `piper`, colliding with the package `piper` and failing `uv add` outright.)

- **Piper wrapper using removed 0.x `synthesize(wf, text)` API** — `piper-tts` 1.x dropped the `PiperVoice.synthesize(wave_file, text)` signature and the WAV header is no longer auto-set by the caller; the wrapper hit `# channels not specified`. Switched to `synthesize_wav(text, wf)`, which owns WAV format setup via `set_wav_format=True`.

- **Chatterbox voice-prompt path** — `scripts/chatterbox/copyspeak-chatterbox.py` resolved voice prompts as `Path(__file__).parent / "voices"`, but the wrapper lives in `<engine_dir>/scripts/` while `install-chatterbox.ps1` creates `voices/` at the engine root. Same root cause as the piper path bug. Fixed by going up one level: `Path(__file__).parent.parent / "voices"`.

- **Kokoro installer missing model files** — `install-kokoro.ps1` ran `uv tool install kokoro-tts` and stopped, but the `kokoro-tts` binary requires `kokoro-v1.0.onnx` (~310 MB) and `voices-v1.0.bin` (~25 MB) that it neither bundles nor auto-downloads. Every synthesis failed with _"Required model files are missing"_. The installer now downloads both into `<engine_dir>/kokoro/models/`, and the args_template (installer snippet + `local_engine_spec` kokoro entry in `commands/tts/health.rs`) injects `--model`/`--voices` pointing at them. New `-SkipModelDownload` switch for offline/reuse.

- **Installer window auto-closing on failure** — the launcher wrapper in `commands/install.rs` ran the script inline, so a terminating error inside the installer (`throw`) escaped past the `ReadKey` pause and the console window closed before the user could read the error. Wrapped the script call in `try/catch`; the "Press any key to close" prompt now runs on both success and failure.

- **`install-chatterbox.ps1` interactive reinstall prompt** — added the same `Get-Confirmation` force prompt as the other installers for consistency.

- **Edge-TTS synthesis crash on `--rate`** — `edge-tts --rate -10%` failed with `argument --rate: expected one argument` (exit code 2): argparse parsed the leading `-` on `-10%` as a flag. Speed was being threaded backend→synthesis for Edge (`--rate`), OpenAI (`"speed"` in JSON body), and HTTP (`{speed}` placeholder), but the frontend **already** applies playback speed itself via `audioEl.playbackRate` (`playback-store.svelte.ts`), so speed was either applied twice (cloud) or crashed (Edge). Fixed by making speed a frontend-only concern mirroring pitch: removed the `speed` parameter from `TtsBackend::synthesize` and all 8 backends, dropped Edge's `speed_to_rate` helper + `--rate` arg, dropped OpenAI's `"speed"` body field, and dropped the HTTP `{speed}` placeholder. The persisted `profile.speed` field and `set_playback_speed` IPC are unchanged (frontend reads `activeProfile.speed` → `playbackRate`); saved audio files no longer bake in profile speed.

- **Local CLI engine wrapper paths** — pre-v0.1.8 local profiles (kitten, piper, chatterbox) stored the engine wrapper as a CWD-relative path (`scripts/

- copyspeak-<engine>.py`), which broke because the Tauri process CWD is `src-tauri/`(dev) or the install dir (packaged), not the engine install dir. Migration in`config/tts.rs::migrate_tts_config`now rewrites bare legacy wrapper paths to`{engine_dir}/<engine>/scripts/...`on every load (idempotent).`install-chatterbox.ps1`'s emitted `profileJson`was also missing`command`/`args_template`, so the printed snippet was non-functional — now baked in with the correct absolute path.

- **Screenshot script window title** — `screenshot-window.ps1` defaulted to `"CopySpeak TTS"` but the actual Tauri window title is `"CopySpeak"`. Capture would always fail unless the title was passed manually. Fixed default.

- **Screenshot capture script** — `scripts/capture-screenshot.mjs` reads version from `tauri.conf.json`, captures the Tauri window via `screenshot-window.ps1`, saves to `static/screen-v{version}.png`, and patches `screenshots.svelte` to reference the new file. One-command screenshot refresh: `node scripts/capture-screenshot.mjs`.

### Removed

- **`supports_speed` catalog flag** — `EngineCatalogEntry.supports_speed` (Rust + `EngineCatalogEntry` TS type) removed; it was unused in the UI and became dishonest once speed stopped being a synthesis parameter. HTTP `{speed}` template placeholder no longer substituted (always emitted `1` at synthesis; user templates should drop it).

- **Pocket engine** — dropped from the Engines page, About page, `local_engine_spec`, `installer_script_for`, `engine-meta.ts::LOCAL_PRESETS`, `i18n/types.ts`, and `scripts/install-pocket.ps1` deleted. The PyPI `pocket-tts` CLI is voice-cloning-first: `--voice` takes a path to a conditioning audio file (not a voice name), the installer-baked `--voice default` was always invalid, and the package pulls in PyTorch (~hundreds of MB). Runtime filename normalization (`pocket-tts → pocket` in `helpers.rs::engine_identifier`) and the `hud.rs` display-name branch are kept as defensive code for any legacy profile still referencing pocket.

- **`voice-credentials.svelte` deleted** — its contextual-credential UX is replaced by the Engines page. Setup metadata consolidated into `engine-meta.ts` (DRY).

- **Empty `/engine` and `/profiles` route directories removed** (left behind by the prior consolidation).

### Changed

- **Voices page (`/voices`) redesigned** — `profile-manager.svelte` restructured from a flat list into grouped cards (Identity, Engine & Voice, Sound, Advanced). Credentials no longer live here; the Engine row shows a passive "Set up engine credentials" hint linking to `/engines` when the active profile's engine key is missing.

- **Credential persistence fixed** — the per-engine config structs (`OpenAIConfig`, `ElevenLabsConfig`, `CartesiaConfig`, `GoogleTtsConfig`, `MicrosoftTtsConfig`) were `#[serde(skip_serializing)]` at the field level in `TtsConfig`, so API keys vanished on restart. Now only `api_key`/`endpoint` persist; profile-owned knobs (model, voice, format, etc.) remain skip-serialized per the profile/global boundary in `docs/profile-engine-settings.md`.

- **Landing screenshot updated** — `screenshots.svelte` now references `screen-v0.1.7.png` (was stale `screen-v0.1.4.png`). Fresh screenshot captured from the v0.1.7 Play page.

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
- **Engine page refactor** — Consolidated `engine-page.svelte` from per-engine subcomponents into a single data-driven panel driven by `ENGINE_TABS`. Removed the cloud-TTS API-key dialog, credential check/test helpers, and unused category metadata; engine settings are now edited inline with a single save flow.
  - Added `placeholderKey` per engine tab and `install_engine`/`check_command_exists` (uv) wiring for local engines.
  - Added English locale strings for engine API-key placeholders.

### Fixed

- **CI release build (Rust compile)** — Resolved 24 compile errors blocking `tauri build`:
  - Wired up `config::post_processing` module (declared + re-exported in `config/mod.rs`) so `LlmProviderConfig` and `PostProcessingProvider` resolve from `commands/config.rs`.
  - Removed 4 dead command registrations in `main.rs` (`get_data_dir`, `get_home_dir`, `get_installer_script_path`, `run_kittentts_installer`) — no definitions, no frontend callers.
  - Added missing `let eff = resolve_effective(&tts_config)` + `voice` bindings in `speak_now` and `speak_queued` (`commands/tts/synthesis.rs`) where `eff`/`voice` were referenced but never bound.

- **CI release build** — Removed stale `bundle.resources` entries in `tauri.conf.json` (`../install-kittentts.ps1`, `../kittentts-cli.py`) that referenced non-existent repo-root files and aborted the bundler. Engine installer scripts are resolved at runtime under `scripts/` via `install_engine`.

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

[Unreleased]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.9...HEAD
[0.1.9]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.7...v0.1.8
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
