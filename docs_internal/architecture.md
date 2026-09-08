# CopySpeak Architecture

> **Version:** v0.1.13
> **Last Updated:** 2026-08-18
> **Status:** Production — actively maintained
> **Diagram:** [`architecture-diagram.html`](architecture-diagram.html) (visual overview, generated from the same tree)

---

## Overview

CopySpeak is a Windows 11 desktop application that monitors the system clipboard and triggers text-to-speech (TTS) when the same text is copied twice within a configurable window (double-copy trigger, default 1500 ms), or via a global hotkey, tray action, or the local control server.

### Design Philosophy

CopySpeak is an orchestrator, not a self-contained TTS solution:

- Users install their own TTS engine (piper, kokoro, kitten-tts, chatterbox, or any CLI) or use cloud APIs (Edge, OpenAI, ElevenLabs, Cartesia, Google, Microsoft, generic HTTP).
- CopySpeak calls the engine via tokio subprocess or reqwest HTTP.
- 11 engine backends share one `TtsBackend` trait; the catalog (`tts/catalog.rs`) drives the Engines UI, installers, and tests.

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Windows 11 System                           │
│                                                                     │
│  Win32 clipboard listener      global shortcuts      tray + autostart│
└──────────┬──────────────────────────┬──────────────────────┬────────┘
           │                          │                      │
           ▼                          ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Tauri v2 Application (copyspeak.exe)           │
│                                                                     │
│  ┌───────────────────── Rust backend (src-tauri) ─────────────────┐ │
│  │ clipboard.rs → sanitize/ → post_process/ → tts/ → audio/       │ │
│  │                                   (Groq, opt.)  (11 engines)    │ │
│  │                                                                 │ │
│  │ fragment_queue + pagination   hud.rs   config/   history/       │ │
│  │ control_server.rs (HTTP)      cli.rs   secrets.rs telemetry.rs  │ │
│  │ main.rs + commands/ (IPC handlers, plugins, tray)               │ │
│  └────────────────────────┬────────────────────────────────────────┘ │
│                     IPC: invoke ↓  ·  events ↑                       │
│  ┌───────────────────── Svelte 5 frontend (src) ───────────────────┐ │
│  │ services/tauri.ts → stores/ (runes)                             │ │
│  │ main window: settings·history·engines·voices·effects·onboarding │ │
│  │ hud window: /hud route, transparent, always-on-top, click-thru  │ │
│  └─────────────────────────────────────────────────────────────────┘ │
└──────────┬──────────────────────────┬────────────────────────────────┘
           │ subprocess                │ HTTPS
           ▼                           ▼
┌────────────────────────┐  ┌──────────────────────────────────────────┐
│  Local CLI engines     │  │  Cloud: edge·openai·elevenlabs·cartesia  │
│  piper (daemon mode)   │  │         google·microsoft·generic http    │
│  kokoro·kitten·any CLI │  │  Groq (LLM rewrite) · GitHub Releases    │
└────────────────────────┘  └──────────────────────────────────────────┘
```

---

## Multi-Window Design

Two `WebviewWindow`s are declared in `src-tauri/tauri.conf.json`:

| Window | Label  | Size     | Properties                                             | Route / URL |
| ------ | ------ | -------- | ------------------------------------------------------ | ----------- |
| Main   | `main` | 775×580  | Centered, fixed size, visible                          | SvelteKit routes |
| HUD    | `hud`  | 300×140  | `transparent`, `alwaysOnTop`, `skipTaskbar`, no decorations, click-through (`set_ignore_cursor_events(true)`) | `/hud` |

The HUD is moved offscreen at startup (`hud::move_hud_offscreen`) and shown on playback. Onboarding, settings, engines, voices, effects, and history are all routes inside the main window, not separate windows.

---

## Backend Module Structure

```
src-tauri/src/
├── main.rs              # App setup, state manage(), 79 IPC handlers, tray, plugins
├── clipboard.rs         # Double-copy detection state machine (Win32 listener)
├── autostart.rs         # Windows startup registration (registry)
├── cli.rs               # Drives the control server; auto-launches GUI if none
├── control_server.rs    # Local HTTP server (127.0.0.1:43117)
├── fragment_queue.rs    # Sequential fragment playback queue
├── pagination.rs        # Splits long text into fragments
├── history.rs           # Speech history persistence
├── history_manager.rs   # History entry lifecycle + cleanup
├── hud.rs               # HUD window positioning / show / hide
├── logging.rs           # flexi_logger wiring
├── post_processing.rs   # LLM post-processing (Groq) pipeline entry
├── secrets.rs           # .env overlay next to exe (env wins over config.json)
├── telemetry.rs         # Synthesis timing per backend/voice → ETA estimates
├── audio/               # rodio playback
│   ├── player.rs        # AudioPlayer
│   ├── wav.rs           # WAV parsing
│   ├── stream.rs        # Streaming utilities
│   └── format.rs        # Format conversion (mp3/ogg/flac)
├── commands/            # IPC handlers
│   ├── config.rs        # get/set/reset/validate config
│   ├── playback.rs      # play/stop/pause/skip/volume + playback events
│   ├── tts/             # synthesis, profiles, helpers (synthesis-state events)
│   ├── history.rs       # history CRUD, search, batch, export, file tracking
│   ├── queue.rs         # fragment queue commands
│   ├── install.rs       # engine install/uninstall/status
│   ├── update.rs        # update check trigger
│   └── logging.rs       # log retrieval
├── config/              # Typed AppConfig sections
│   ├── mod.rs           # AppConfig, load/save to %APPDATA%/CopySpeak/config.json
│   ├── tts.rs, playback.rs, trigger.rs, general.rs, output.rs
│   ├── hotkey.rs, hud.rs, sanitization.rs, effects.rs
│   ├── post_process.rs, post_processing.rs   # Groq/LLM config
│   └── tests.rs
├── post_process/        # Groq client for optional text rewrite
├── sanitize/            # 3-pass text normalization
│   ├── markdown.rs      # Pass 1: markdown stripping
│   ├── tts_normalize.rs # Pass 2: TTS normalization
│   └── cleanup.rs       # Pass 3: artifact cleanup (always runs)
└── tts/                 # Engine backends
    ├── mod.rs           # TtsBackend trait
    ├── catalog.rs       # Engine metadata → drives Engines UI
    ├── cli.rs           # Any local CLI engine (template-driven)
    ├── local_daemon.rs  # Persistent daemons for Piper/Kitten/Kokoro/Pocket (models resident in RAM)
    ├── edge.rs, openai.rs, elevenlabs.rs, cartesia.rs
    ├── google.rs, microsoft.rs, http.rs
    └── ...
```

### Module Responsibilities

#### `clipboard.rs` — Clipboard State Machine

```
IDLE ──(clipboard change)──► ARMED ──(same text within window)──► SPEAK
  ▲                            │
  └────(different text)────────┘
  └────(timeout)───────────────┘
```

Emits `clipboard-change` on every change, `speak-request` on trigger, `text-truncated` when the max length clamps input.

#### `sanitize/` — Text Normalization Pipeline

Three passes, all in `src-tauri/src/sanitize/`:

**Pass 1 — Markdown stripping** (`markdown.rs`, optional): code blocks/inline code removed, `[text](url)` → `text`, `# Heading` → `Heading.` (period for sentence boundary unless it already ends in `.?!:;`), bold/italic/list/blockquote markers removed.

**Pass 2 — TTS normalization** (`tts_normalize.rs`, optional), priority order: emoji removal → URL removal → citation removal (`[1]`) → slash lookups (`w/o` → `without`) → slash options (`true/false` → `true or false`) → slash ratios (`km/h` → `km per h`) → Latin abbreviations (`e.g.` → `for example`) → title abbreviations (`Dr.` → `Doctor`) → number suffixes (`5m` → `5 million`) → metric units (`10km` → `10 kilometers`) → symbols (`&` → `and`, `$50` → `50 dollars`) → punctuation normalization → artifact cleanup → newline stripping.

**Pass 3 — Cleanup** (`cleanup.rs`, always): collapse spaces/blank lines, fix punctuation spacing, trim.

#### `tts/` — Backend Abstraction

```rust
pub trait TtsBackend: Send + Sync {
    fn name(&self) -> &str;
    fn synthesize(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>, TtsError>;
    fn health_check(&self) -> Result<(), TtsError>;
    fn supports_streaming(&self) -> bool { false }
}
```

| Backend | Type | Notes |
| ------- | ---- | ----- |
| CLI | tokio subprocess | piper, kokoro, kitten, chatterbox, any command; template args |
| Local daemons | persistent subprocess per engine | Model stays resident; PCM streams back over the pipe; falls back to one-shot |
| Edge | free cloud | Default engine (Microsoft Edge Read Aloud) |
| OpenAI | cloud API | 9 voices |
| ElevenLabs | cloud API | Dynamic voice listing, output formats, voice settings |
| Cartesia | cloud API | Sonic 3.5 |
| Google | cloud API | |
| Microsoft | cloud API | Azure Cognitive Services |
| HTTP | generic REST | Self-hosted / custom servers |

#### `control_server.rs` + `cli.rs` — External Control Plane

- Local HTTP server on `127.0.0.1:43117` (thread-spawned at startup).
- Endpoints: `GET /health`, `POST /speak` (text, engine, effect params). This is how the **Pi** and **Claude Code** extensions (`scripts/claude-copyspeak-hook.mjs`, `scripts/copyspeak.mjs`) make CopySpeak talk.
- `cli.rs`: when the `.exe` is launched with a subcommand, it connects to the control server; if none is running it auto-launches a detached GUI instance, waits for `/health`, then runs the command.

#### `secrets.rs` — API Key Resolution

A `.env` file next to the executable overlays `config.json` values. Env wins when set and non-empty; resolved values are never written back to disk. Keys typed in the UI remain the fallback.

#### `telemetry.rs` — ETA Estimation

Tracks synthesis duration per backend/voice/character-bucket to predict job times (shown in the HUD synthesis progress).

#### `history/` — Speech History

Persistent JSON log with audio file tracking: search, batch ops, export, statistics, orphaned/missing file detection, auto-cleanup by age/count.

---

## Frontend Architecture

```
src/
├── lib/
│   ├── components/
│   │   ├── engine/       # engine-panel, engine-setup, install-dialog,
│   │   │                 # profile-manager, voice-picker, profile-export
│   │   ├── history/      # entry, search, bulk-actions, export-dialog
│   │   ├── hud/          # clipboard-notification, playback-content,
│   │   │                 # synthesis-progress, status
│   │   ├── settings/     # general, appearance, playback, hotkey, batch,
│   │   │                 # pagination, sanitization, history, import-export,
│   │   │                 # post-process(ing), about
│   │   ├── landing/      # marketing page
│   │   ├── layout/       # app-header, app-footer
│   │   ├── ui/           # shadcn-svelte primitives
│   │   ├── hud-overlay.svelte, global-player.svelte, waveform.svelte
│   │   ├── play-page / settings-page / history-page / effects-page
│   │   ├── playback-controls, quick-settings, recent-history
│   │   └── update-checker, theme-toggle, hotkey-capture, virtual-list
│   ├── composables/      # use-hud-events.ts
│   ├── services/tauri.ts # invoke bridge + event listeners (single entry point)
│   ├── stores/           # Svelte 5 runes: playback, synthesis, hud, history,
│   │                     # listening, install, save-bar
│   ├── i18n/, locales/   # svelte-i18n
│   ├── models/, types.ts, utils/, utils.ts, version.ts
├── routes/
│   ├── /, settings/, engines/, voices/, effects/, history/,
│   ├── onboarding/
│   └── hud/              # separate webview window mounts this route
└── app.html
```

### Technology Stack

- **Svelte 5** with runes (`$state`, `$effect`, `$derived`, `$props`)
- **SvelteKit 2** (static output) + **Vite 8** multi-page build
- **Tailwind CSS v4** + **shadcn-svelte** (brutalist design system)
- **mode-watcher** (dark/light), **svelte-i18n**, **svelte-sonner**
- **Bun** package manager

---

## IPC Commands

79 handlers registered in `main.rs` via `generate_handler![]`, grouped by `commands/` domain:

| Domain | Commands |
| ------ | -------- |
| Config | `get_config`, `set_config`, `reset_config`, `config_exists`, `validate_config` |
| Trigger / clipboard | `speak_now`, `speak_now_with_profile`, `speak_selected_text`, `speak_queued`, `replay_cached`, `abort_synthesis`, `set_listening`, `get_listening`, `get_clipboard_content` |
| Playback | `stop_speaking`, `toggle_pause`, `skip_forward`, `skip_backward`, `set_playback_speed`, `get_playback_state`, `set_volume`, `set_debug_mode` |
| Queue / pagination | `get_queue_state`, `get_queue_fragments`, `skip_to_fragment`, `stop_queue`, `clear_queue` |
| History | `get_history`, `list_history`, `search_history`, `get_history_batch`, `get_history_with_metadata`, `get_history_statistics`, `clear_history`, `delete_history_entry`, `delete_history_batch`, `speak_history_entry`, `play_history_entry`, `play_history_batch`, `copy_history_entry_text`, `export_history`, `get_history_unique_engines`, `get_history_unique_voices`, `get_history_unique_tags`, `get_history_date_range`, `run_history_cleanup` |
| History file tracking | `get_file_tracking`, `get_entry_by_file_path`, `verify_file_exists`, `verify_all_files`, `get_orphaned_files`, `get_missing_files`, `unlink_file`, `get_file_metadata`, `is_file_tracked` |
| HUD | `show_hud_for_playback`, `test_show_hud` |
| Engines & credentials | `list_tts_engines`, `list_tts_voices`, `install_engine`, `uninstall_engine`, `engine_status`, `test_tts_engine`, `test_tts_engine_config`, `test_local_engine`, `check_command_exists`, `check_elevenlabs_credentials`, `check_cartesia_credentials`, `check_openai_credentials`, `check_groq_credentials`, `has_engine_credentials`, `list_elevenlabs_voices`, `get_elevenlabs_voice_by_id`, `get_elevenlabs_output_formats` |
| Profiles | `set_active_profile` |
| Post-processing | `list_post_processing_models` |
| System | `get_logs`, `get_logs_path`, `trigger_update_check` |

### IPC Events (Rust → Frontend)

Verified against `emit()` calls in the source:

| Event | Emitted when |
| ----- | ------------ |
| `clipboard-change` | Clipboard content changes |
| `speak-request` | Double-copy trigger detected |
| `text-truncated` | Input clamped by max length |
| `synthesis-state-change` | Synthesis starts / ends |
| `synthesis-aborted` | Synthesis cancelled |
| `audio-ready` | Synthesized audio available to play |
| `playback-stop` | Playback stopped |
| `playback-toggle-pause` | Pause / resume toggled |
| `config-changed` | Config or profile updated |
| `history-updated` | History entry added / changed |
| `hud` | HUD show / hide / update |
| `pagination:started` | Multi-fragment synthesis begins |
| `pagination:fragment-started` | Fragment synthesis starts |
| `pagination:fragment-ready` | Fragment audio ready |
| `check-for-updates` | Updater triggered from backend |

---

## State Management

### Backend (Rust)

`Mutex`-wrapped structs registered with `app.manage()`: config, audio player, history, history manager, cached audio, fragment queue, telemetry, listening flag, job status, and a tokio async lock serializing synthesis.

### Frontend (Svelte)

Rune-based stores in `src/lib/stores/` (`playback-store`, `synthesis-store`, `hud-store`, `history-store`, `listening-store`, `install-store`, `save-bar`), fed exclusively through `services/tauri.ts`.

---

## Data Flow: Speech Trigger

```
1. User copies text (Ctrl+C)
    └─► Win32 AddClipboardFormatListener fires
2. clipboard.rs state machine
    └─► Double-copy within window → emit speak-request
3. Sanitize (3 passes: markdown → normalize → cleanup)
4. Optional LLM post-processing (Groq rewrite for listening)
5. Pagination (if enabled): split into fragments → fragment_queue
6. tts/ synthesizes (subprocess or HTTPS) → bytes
7. WebView plays the audio (volume; speed and pitch via `TimeStretcher`, independent knobs)
8. Effects applied per-profile (OfflineAudioContext in the WebView)
9. history/ logs entry; HUD shows waveform; telemetry records duration
10. Frontend refreshes via history-updated / audio-ready events
```

## Voice Profiles

Named presets (`engine + voice + speed + pitch + effects`) managed by `profile-manager.svelte`, applied via `set_active_profile` / `speak_now_with_profile`. Effects live on the profile (`VoiceProfile.effects`), not in global config. Profiles export/import via `profile-export-dialog`.

`speed` (0.5-2.0) and `pitch` (0.75-1.35) are independent: speed time-stretches without shifting pitch, pitch shifts without changing duration. Both ranges live in `config/tts.rs` (`SPEED_RANGE`, `PITCH_RANGE`) and are clamped on config load. The streaming PCM path stretches each chunk in `playback/time-stretch.ts` before scheduling; the `<audio>` path bakes the pitch shift into the blob and leaves speed to `preservesPitch` + `playbackRate`.

---

## Global Hotkey

- **Plugin**: `tauri-plugin-global-shortcut` registers a system-wide combo (default `Super+Shift+A`).
- **Config**: `HotkeyConfig` (`enabled`, `shortcut`) in `config/hotkey.rs`; `set_config` re-registers on change.
- **Flow**: plugin handler → `speak_now()` with current clipboard text.

---

## Control Server & CLI

External integrations talk to `127.0.0.1:43117`:

```
Pi agent ─┐
Claude Code hook ─┤─► POST /speak { text, engine?, effect? } ─► synthesis pipeline
curl ─────┘
GET /health ─► liveness probe
```

`copyspeak.exe speak "text"` uses the same path: `cli.rs` → control server (auto-launching the GUI if needed).

---

## Configuration Structure

`%APPDATA%/CopySpeak/config.json`, typed by `AppConfig` (`config/mod.rs`): `version`, `general`, `trigger`, `tts`, `playback`, `hud`, `output`, `sanitization`, `pagination`, `history`, `hotkey`, `post_process`. Effects are per-profile (see Voice Profiles). Shape (abbreviated):

```json
{
  "trigger":     { "listen_enabled": true, "double_copy_window_ms": 1500, "max_text_length": 100000 },
  "tts":         { "active_backend": "cartesia", "...per-engine blocks": "api_key, voice, model" },
  "playback":    { "on_retrigger": "queue", "volume": 100, "playback_speed": 1.35, "pitch": 1.15 },
  "hud":         { "enabled": true, "position": "bottom-center", "width": 300, "height": 140, "opacity": 0.85 },
  "hotkey":      { "enabled": false, "shortcut": "Super+Shift+A" },
  "general":     { "start_with_windows": false, "start_minimized": true, "close_behavior": "minimize-to-tray" },
  "output":      { "enabled": false, "directory": "", "format_config": { "format": "wav" } },
  "sanitization":{ "markdown_enabled": true, "tts_normalize_enabled": true },
  "pagination":  { "enabled": false, "fragment_size": 500 },
  "history":     { "enabled": true, "max_entries": 1000, "max_age_days": 30, "save_audio": true },
  "post_process":{ "enabled": false, "provider": "groq", "model": "..." }
}
```

---

## Security Considerations

### Tauri Capabilities

Two capability files: `capabilities/default.json` (main window: core, window management, events, dialog, opener, updater, process restart, global-shortcut) and `capabilities/hud.json` (HUD window: events + window basics only — no updater, no dialog).

### CLI Execution

- User configures which engine command runs; templates stored locally; no remote execution.
- Subprocess spawning is tokio-managed; local engine daemons stay resident for latency.

### API Keys

- Typed in the UI (stored in `config.json`) **or** supplied via `.env` next to the exe (`secrets.rs`); env wins, and env values are never persisted.
- Keys only travel to their configured endpoints.

---

## Performance Considerations

1. **Event-driven clipboard** — Win32 listener, no polling.
2. **Local daemons** — model resident in RAM for Piper/Kitten/Kokoro/Pocket; synthesis is a pipe round-trip instead of a full process start + model load, and PCM streams back as it is produced.
3. **rodio buffering** — playback double-buffering handled by the crate.
4. **Fragment queue** — long texts synthesize/play sequentially without blocking the UI.
5. **Telemetry-based ETA** — per-backend/voice timing buckets drive progress estimates.
6. **Fine-grained reactivity** — Svelte 5 runes limit re-renders; virtual list for long history.

---

## Deferred Features

Preserved on the `features-extras` branch (different repository):

1. **Language Detection** — auto voice selection by text language
2. **Content Filtering** — regex rules to avoid speaking sensitive data
3. **Application Filter** — per-app whitelist/blacklist
4. **Batch Processing** — multi-text queue with dedicated UI

## Implemented Features

- HUD overlay (waveform, clipboard notifications, synthesis progress, click-through)
- Global hotkey · tray · autostart · single-instance
- Voice profiles (create/switch/export) · audio effects per profile
- History: search, batch, export, statistics, file tracking, auto-cleanup
- Engine catalog with per-engine setup pages, installers, credential checks
- Control server (HTTP) + CLI + Pi/Claude Code extensions
- LLM post-processing (Groq) · audio save mode (wav/mp3/ogg/flac)
- Auto-updater (GitHub Releases) · i18n · dark/light · telemetry ETA

## Future Considerations

- Pronunciation dictionary (custom word pronunciations)
- Cross-platform (macOS/Linux) — requires abstracting the Win32 clipboard/tray/autostart layer
- Usage statistics dashboard (telemetry already collects the data)
