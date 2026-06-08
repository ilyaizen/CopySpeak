# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Persistent Piper TTS RAM caching** — Keeps the Piper voice model loaded in RAM using a background HTTP server process (`piper.http_server`) on a free localhost port. Speeds up consecutive synthesis triggers to near-instantaneous.
- **CUDA/GPU acceleration for Piper** — Added a CUDA option to the UI and configuration, enabling GPU-accelerated local inference if available.
- **PowerShell setup scripts** — Added `setup-piper-cpu.ps1` and `setup-piper-cuda.ps1` scripts to automate installing `piper-tts[http]` (for persistent RAM caching) along with their corresponding CPU or GPU/NVIDIA library dependencies.
- **Automatic CUDA DLL path discovery** — Rust backend now queries Python on startup to locate pip-installed NVIDIA runtime paths (like `cublas`, `cudnn`, etc.) and adds them to the environment `PATH` when spawning the server, making GPU acceleration work fully out-of-the-box on Windows.
- **Dynamic local Piper voice discovery** — Automatically scans the `piper-voices` folder and lists all downloaded quality variations (low, medium, high) dynamically in the dropdown.
- **"Unload Model" system tray action** — Added a menu item to the tray context menu to manually terminate the background server and unload the model from RAM.
- **New Rust IPC commands** — Added `get_local_piper_voices` to discover available models and `unload_piper_model` to allow manual unloading of the cached model.
- **Server teardown hooks** — Hooked server termination into Tauri's `RunEvent::Exit` so the background python server is always cleaned up when the app is closed.
- **History bulk operations** — Added multi-select checkboxes to every history entry, a "Clear All" button, and a bulk actions toolbar (Select All, Clear Selection, Export Selected, Delete Selected with double-click confirmation) wired through the existing `HistoryBulkActions` component.
- **History "Clear All" confirmation dialog** — Added an `AlertDialog` confirmation before wiping the entire history, with new i18n keys (`history.clearAll`, `history.clearAllDialogTitle`, `history.clearAllDescription`, `history.confirmClearAll`).
- **Reusable HTTP connection pooling for all cloud TTS backends** — Each backend (`ElevenLabsTtsBackend`, `OpenAiTtsBackend`, `CartesiaTtsBackend`, and Groq post-process) now holds a single `reqwest::Client` with `tcp_nodelay`, `tcp_keepalive(60s)`, and `pool_max_idle_per_host(2)` configured in `new()`, eliminating TLS handshake + TCP connection setup per synthesis request.
- **Precomputed `Bearer` header in OpenAI backend** — The `Authorization: Bearer <key>` header is now computed once in `OpenAiTtsBackend::new()` and reused across all requests, avoiding per-request `format!()` allocations.
- **NVIDIA DLL paths cached via `OnceLock`** — The expensive `python -c "import nvidia..."` subprocess call that discovers GPU library paths now runs only once per app lifetime, cached in a static `OnceLock<Option<String>>`.
- **Adaptive fragment sizing from telemetry** — `pagination::adaptive_fragment_size()` uses per-engine `chars_per_ms` telemetry (3+ samples required) to dynamically scale the pagination fragment size: fast engines (>20 chars/ms) get 3× the base size (capped at 2000), moderate engines (5–20 chars/ms) get 2× (capped at 1500), slow or unknown engines keep the default.
- **Parallel cloud fragment synthesis** — For cloud backends (OpenAI, ElevenLabs, Cartesia) with multiple paginated fragments, `speak_queued` now synthesizes up to 3 fragments concurrently via `tokio::task::JoinSet`, then emits results in index order. CLI backends continue to use the existing sequential loop.
- **Pre-decode next fragment during playback** — When a fragment starts playing, the `PlaybackStore` asynchronously base64-decodes and `decodeAudioData()` the next queued fragment in the background, so when the current fragment ends the next one is ready to play instantly with zero decode gap.
- **Piper server pre-warm at app startup** — When the active backend is Local with the `piper` preset, `prewarm_piper_server()` spawns the HTTP server in a background thread at app launch, loading the voice model into RAM before the user ever triggers synthesis.
- **Piper warm-up synthesis** — After the server reports ready, `prewarm_piper_server()` sends a hidden synthesis to force ONNX Runtime JIT compilation and GPU kernel init, eliminating a ~1.6s cold-start penalty on the first real request.
- **Piper server auto-restart on config change** — `restart_piper_server()` kills and respawns the Piper server when voice or CUDA settings change, with a `PIPER_WARMING` atomic guard preventing duplicate servers.
- **Piper server status endpoint** — `get_piper_server_status()` returns `PiperServerStatus` (running, model, port, cuda, ready) for the control server `/piper-status` health check.
- **Piper performance test script** — `test-piper-perf.ps1` automates synthesis timing measurements via the control server API.

### Changed

- **Frontend Dependency Upgrades** — Upgraded Svelte to `5.56.1`, `@sveltejs/kit` to `2.63.0`, Vite to `8.0.16`, Vitest to `4.1.8`, and all Tauri frontend modules to their latest v2 releases.
- **Backend Cargo Upgrades** — Upgraded Rust crate dependencies (`dirs` to v6, `flexi_logger` to v0.31, `winreg` to v0.56, `chrono`, and `log` to their latest patch versions).
- **CLI synthesis engine** — Intercepts synthesis calls for Piper to route them via the running local HTTP server instead of spawning a new process for every synthesis. Adds fallback to standard CLI execution if server synthesis fails.
- **Piper HTTP server health-check poll interval** — Reduced from every 100ms to every 50ms for the first 2 seconds, then 200ms thereafter, improving server readiness detection speed.
- **Piper speed control** — The `_speed` parameter is now passed as `length_scale` in the Piper HTTP API JSON body, allowing playback speed adjustments at the synthesis level.
- **Piper server `reqwest::blocking::Client` reuse** — The health-check poll client is now stored in `PiperServerState` and reused for all subsequent synthesis HTTP requests instead of creating a fresh `Client::new()` each time.
- **Backend created once per `speak_queued`** — The `create_backend()` call was hoisted outside the pagination loop so that the same `Arc<Box<dyn TtsBackend>>` (with its shared connection pool) is reused for all fragments in a paginated synthesis.
- **base64 encoding moved to `spawn_blocking`** — `emit_audio_ready()` and `emit_audio_fragment()` are now async functions that run base64 encoding on tokio's blocking thread pool, preventing CPU-intensive encoding from stalling the async worker thread.
- **`history-updated` event batched** — The `history-updated` Tauri event is now emitted once at the end of all fragment synthesis instead of per-fragment, avoiding repeated frontend re-renders.
- **HUD event emission** — Removed `std::thread::spawn` + 50ms `thread::sleep` pattern from `show_hud`, `show_hud_synthesizing`, and `show_hud_playback`. Events are now emitted synchronously with direct `app.emit()`, eliminating OS thread creation overhead per HUD display.
- **Windows audio preroll** — Reduced from 1200ms to 200ms of near-silent sine wave prepended to audio playback on Windows, reducing dead air before speech starts by 1 second.
- **`AudioContext` pre-warmed at startup** — `PlaybackStore.setupListeners()` now creates the `AudioContext` and calls `resume()` immediately at app startup instead of lazily on first playback, avoiding a 50–200ms cold-start delay.
- **Clipboard config read** — Combined two separate `AppConfig` mutex locks (for `trigger_window_ms`/`max_text_length` and for `sanitization_config`) into a single lock acquisition in `clipboard.rs:on_change`, reducing mutex contention.
- **Cleanup pass conditional** — `cleanup_artifacts()` in the sanitization pipeline now runs a single pass and only re-runs if the text actually changed, instead of unconditionally running twice.
- **Piper server consolidated single-lock pattern** — `synthesize_via_server()` acquires the `PIPER_SERVER` mutex in one cohesive critical section (check → start → extract port), eliminating the triple lock/unlock cycle and race windows for duplicate server processes.
- **Piper HTTP client simplified** — Replaced custom `build_keepalive_client()` builder with bare `reqwest::blocking::Client::new()`, which already enables connection pooling by default without unnecessary TCP keepalive syscalls on localhost.
- **Piper synthesis timing instrumentation** — Every synthesis call logs `[Piper] Synthesis — total:Xms (req:Yms read:Zms) size:N B chars:N voice:X cuda:bool` for visibility into HTTP vs data transfer time.
- **TTS pipeline timing breakdown** — `handle_playback_output()` logs `[TTS] Pipeline — synth:Xms env:Xms hist:Xms emit:Xms total_post:Xms` showing exactly where post-synthesis time goes.
- **Synthesis timing always visible** — Changed synthesis completion from `log::debug!` (debug-mode only) to `log::info!` so millisecond timings are always in console output.
- **Audio thread poll interval** — Increased the audio playback monitor's channel receive timeout from 50ms to 200ms, reducing idle CPU wake-ups by 4×.
- **Piper log prefix standardized** — All Piper-related log messages use `[Piper]` prefix for consistency across prewarm, synthesis, restart, and health-check paths.
- **Piper server port+client extracted in a single Mutex lock** — `synthesize_via_server()` now returns a `(port, client)` tuple from one lock acquisition, eliminating a second lock/unlock cycle that cloned the HTTP client handle separately.
- **Piper parallel paginated synthesis** — `speak_queued` now routes the Piper preset (`Local + preset "piper"`) through `synthesize_queued_parallel` alongside cloud engines, enabling concurrent fragment synthesis for multi-fragment texts via the persistent server.

### Fixed

- **Parallel fragment synthesis processes all fragments** — `synthesize_queued_parallel` now loops with a max-concurrency cap of 3 until every fragment is synthesized, fixing a bug where `fragments.len().min(3)` silently dropped fragments beyond the first three. Emit order remains sequential.
- **Piper prewarm/synthesis race condition** — Added `PIPER_WARMING` atomic flag with `ClearWarming` RAII drop guard. `synthesize_via_server()` polls this flag before starting a new server, eliminating a race where `restart_piper_server`'s background prewarm thread and a foreground synthesis call would start two separate server processes simultaneously.
- **System Tray and Configuration Sync** — Fixed the listening state toggle sync by making `set_listening` IPC command update and persist the `AppConfig` to disk and emit the `config-changed` event.
- **Listening State Initialization** — Initialized the tray listening menu item label dynamically on startup using the user's persistent config instead of hardcoding `"● Listening"`.
- **Chrono timestamp allocation** — Moved `chrono::Local::now().format(...)` inside the `is_debug_mode()` guard in `clipboard.rs`, avoiding a heap allocation on every clipboard change in release builds.
- **Pagination Configuration Bypass** — Fixed `synthesize_paginated` to respect the user's settings by passing `pagination_config` from active configuration instead of hardcoding `PaginationConfig::default()`.
- **Config Validation Unit Tests** — Explicitly set `active_backend = TtsEngine::Local` in local validation tests in `tests.rs` to prevent failure when the default project engine is configured to a non-local engine.
- **CLI TTS Engine Health Check** — Fixed the pre-existing health check to dynamically find and use any downloaded local `.onnx` voice in the user's voice folder, resolving failure errors complaining about a missing `"Rosie"` voice.
- **Clippy and Panic Fixes in CLI TTS backend** — Resolved option unwrap panic vector in Piper server port logic and simplified file extension checks using `is_some_and`.

### Performance

- **Piper unified HTTP client** — Replaced ad-hoc `build_client()` with a `OnceLock<reqwest::Client>` singleton configured with `tcp_nodelay(true)` and `pool_max_idle_per_host(2)`. All Piper call sites (prewarm warm-up, server storage, synthesis) share the same connection pool via `.clone()`.
- **Piper HTTP response double-buffer eliminated** — `synthesize_via_server()` now calls `response.bytes()?.to_vec()` directly for zero-copy `Bytes`→`Vec` conversion, avoiding pre-allocation + `extend_from_slice()` copy.
- **Piper health-poll exponential backoff** — Replaced fixed 50ms/200ms delays with 100→200→400→800→1600ms exponential backoff in both `prewarm_piper_server()` and `synthesize_via_server()`, reducing CPU wake-ups during server startup.
- **Windows PATH expansion cached** — Wrapped `get_expanded_path()` in a `OnceLock<String>`; the 20-path iteration now computes once per process lifetime instead of on every CLI synthesis call.
- **Removed dead `AudioCommand::Play/Pause/Resume`** — Removed 3 variants, their match arms in the audio thread loop, and the `AudioPlayer::play()/pause()/resume()/is_paused()` methods (never called externally; playback uses `TogglePause`/`Stop` only).
- **Removed dead `SynthesisProgressEvent`** — 12-line struct never constructed; frontend uses a separate `SynthesisProgressPayload` type for the `hud:synthesis-progress` event.
- **Removed dead `FragmentQueue` methods** — Removed `add_fragment`, `is_empty`, `current_fragment`, `get_fragment`, `get_audio`, `has_audio`, `next`, `previous`, `clear_stop_flag`, `pause`, `resume`, `start` (~120 lines of untested-in-production queue navigation API) plus their test suite, reducing from 17 test cases to 7 focused tests.
- **Removed unused `Cursor`/`Decoder` imports** — Cleaned up imports in `audio/player.rs` after `play()` method removal.

- **base64 decode optimization** — Replaced the manual `for` loop over `charCodeAt(i)` with `Uint8Array.from(binary, c => c.charCodeAt(0))` in both `handleAudioReady` and `predecodeNextFragment`, leveraging V8's internal typed-array fast path for ~2M fewer JavaScript VM bytecode iterations per fragment.
- **WAV conversion optimization** — Rewrote `audioBufferToWavBlob()` to use a single `Int16Array` view over the output buffer with interleaved channel writes, replacing the inner-per-sample `DataView.setInt16()` loop for ~10× faster PCM sample writing.
- **Reduced `ArrayBuffer` copies** — Removed the redundant `arrayBuffer.slice(0)` copy in `handleAudioReady`; `decodeAudioData` now reads directly from the original buffer and `_originalBytes` is set once (not twice).
- **Analyser `Uint8Array` reuse** — `AudioAnalyser.start()` now allocates the frequency data `Uint8Array` once at `setup()` and reuses it every frame in the rAF loop instead of calling `new Uint8Array(...)` 60 times per second.
- **OpenAI header precomputation** — `format!("Bearer {}", api_key)` is now computed once in `new()` and stored as `auth_header: String`, avoiding a per-request allocation.
- **Tauri Plugin Registration dedup** — Removed a duplicate registration of `tauri-plugin-global-shortcut` builder in `main.rs`.
- **WAV envelope extraction: streaming RMS** — `extract_envelope()` now computes RMS in a single pass over raw PCM data without allocating an intermediate `Vec<f32>`. For short audio (<0.5s), processes every frame; for longer audio, decimates to at most `num_bars × 256` samples per bar. Eliminates ~880KB allocation for typical TTS outputs.
- **PCM frame decoder inlined** — `decode_frame_mono()` marked `#[inline(always)]` with per-bit-depth fast paths, eliminating function call overhead in the hot envelope extraction loop.
- **Pagination: no `Vec<char>` allocation** — `paginate_text()`, `detect_sentence_boundaries()`, and `force_split()` now operate on `&str` byte offsets via `char_indices()` instead of materializing the entire text as `Vec<char>`, saving ~400KB for 100k-char inputs.
- **Sequential fragment encoding pipelined** — `synthesize_queued_sequential()` spawns fragment N's base64 encoding in a background `tokio::task` while synthesizing fragment N+1, overlapping CPU-bound encoding with I/O-bound synthesis on the blocking thread pool.
- **Removed dead `WavStreamSource`** — Deleted `src-tauri/src/audio/stream.rs` (254 lines), `AudioCommand::PlayStreaming` variant, and `play_streaming()` method — all dead code from a pre-HTTP stdout-streaming approach.
- **Removed dead `read_pcm_samples`/`compute_rms`** — Deleted two functions replaced by the streaming `extract_envelope` + inline `decode_frame_mono`.
- **Cargo release profile** — Added `opt-level = 3`, `lto = true`, `codegen-units = 1`, and `strip = true` to `[profile.release]` in `Cargo.toml`.
- **Cleaned `#[allow(dead_code)]` annotations** — Removed file-level `#![allow(dead_code)]` from `fragment_queue.rs` and unnecessary annotations from `TtsError`, `Voice`, `TtsBackend` trait methods, `SynthesisProgressEvent`, and `AudioPlayer` fields/methods.

- **Piper removed from parallel synthesis** — `is_parallel_capable` no longer includes the Local+piper preset. Piper's HTTP server is single-threaded Python; concurrent requests cause head-of-line blocking with no speed gain. Sequential synthesis reduces server contention.
- **Piper health-check poll client reused** — `synthesize_via_server()` now uses the global `get_piper_client()` singleton for health-check polling instead of building a new `reqwest::blocking::Client` per server start, avoiding a redundant TCP handshake.
- **Piper server mutex lock scope minimized** — The `PIPER_SERVER` mutex is released before the HTTP synthesis request. `synthesize_via_server()` extracts `(port, client)` under lock, then performs the synthesis outside the critical section. Verified via new `lock:0ms` log metric on warm-server calls.
- **Envelope extraction offloaded to blocking thread** — `extract_envelope_async()` wraps WAV parsing in `spawn_blocking`, preventing large audio files from blocking the tokio async worker thread. All 4 call sites updated.
- **History saves batched in paginated synthesis** — `add_entry_with_batch()` accepts `skip_save: bool`; `synthesize_queued_sequential()` and `synthesize_queued_parallel()` set `skip_save=true` per-fragment and call `history::save()` once at the end. N fragments → 1 disk write instead of N.
- **Telemetry saves debounced** — `record_sample()` now persists to disk every 10 samples via an `AtomicU32` counter instead of on every synthesis call, reducing disk I/O by 90%.
- **Double-spawn eliminated in fragment emit** — `spawn_fragment_emit()` now uses a single `spawn_blocking` call (encoding + emit) instead of nested `spawn(async { spawn_blocking(...) })`, saving one task allocation per fragment.
- **Hot-path functions inlined** — Added `#[inline]` to `create_backend()`, `voice_for_backend()`, and `engine_str()`.
- **Piper warmup text extended** — Warm-up synthesis text increased from 5 chars (`"Hello"`) to 80 chars for more thorough ONNX Runtime JIT/GPU kernel warmup.

### Changed

- **Speed parameter threaded through synthesis** — `synthesize_async()` now accepts `speed: f32` and all call sites pass `config.playback.playback_speed`. Piper HTTP receives it as `length_scale`; previously hardcoded to `1.0`.
- **Piper synthesis timing expanded** — Log format now includes `lock_ms` (mutex hold time), `poll_attempts` (health-check retries), and `spawn_ms` (process spawn time) in addition to the existing `req_ms`/`read_ms` breakdown.
- **Duplicate tray/hotkey speak code consolidated** — Extracted a shared `spawn_speak(&AppHandle)` helper in `main.rs`, eliminating 28 lines of duplicate state extraction across the tray menu handler and global hotkey handler.
- **History cleanup deferred on startup** — The background cleanup service now sleeps 30s before its first run instead of executing immediately, avoiding disk I/O during app launch.
- **Synthesis calls always pass speed** — `synthesize_async()`, `synthesize_paginated()`, `synthesize_queued_sequential()`, and `synthesize_queued_parallel()` all accept and propagate the `speed` parameter.

### Removed

- **`history_manager.rs`** — Deleted the entire 385-line file (`HistoryManager` struct with `#![allow(dead_code)]`). The managed state was created in `main.rs` but never consumed by any Tauri command (all history operations use `HistoryLog` directly).
- **Dead functions in `history.rs`** — Removed `create_entry()`, `add_entry()`, `add_entry_complete()`, `update_file_size()`, `format_file_size()`, and `get_total_file_size_human()` (~80 lines of never-called API).
- **Dead methods in `pagination.rs`** — Removed `TextFragment::is_first()`, `is_last()`, and `label()` (~20 lines, all `#[allow(dead_code)]`).
- **Dead function in `telemetry.rs`** — Removed `get_bucket_label()` (debug formatting, never called).
- **Dead method in `config/output.rs`** — Removed `AudioFormat::from_extension()` (`#[allow(dead_code)]`).
- **Unused macro in `logging.rs`** — Removed `debug_log!` macro (never invoked).
- **Dead import in `main.rs`** — Removed `mod history_manager;` declaration.

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

[Unreleased]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.0.5...v0.1.0
[0.0.5]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.0.3...v0.0.5
[0.0.3]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/ilyaizen/copyspeak-tts/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/ilyaizen/copyspeak-tts/releases/tag/v0.0.1
