# CopySpeak Architecture

> **Version:** v0.0.5
> **Last Updated:** 2026-03-25
> **Status:** Pre-Production / In Active Development
> **Note:** Five features have been deferred and are preserved on the `features-extras` branch on a differant repository: Language Detection, Content Filtering, Application Filter, Keyboard Shortcuts, and Batch Processing. HUD Overlay has been reimplemented, but needs UI improvements.

---

## Table of Contents

- [CopySpeak Architecture](#copyspeak-architecture)
  - [Table of Contents](#table-of-contents)
  - [Overview](#overview)
    - [Design Philosophy](#design-philosophy)
  - [System Architecture](#system-architecture)
  - [Multi-Window Design](#multi-window-design)
  - [Backend Module Structure](#backend-module-structure)
    - [Module Responsibilities](#module-responsibilities)
  - [Frontend Architecture](#frontend-architecture)
    - [Technology Stack](#technology-stack)
  - [IPC Commands](#ipc-commands)
  - [State Management](#state-management)
    - [Backend State (Rust)](#backend-state-rust)
    - [Frontend State (Svelte)](#frontend-state-svelte)
  - [Data Flow: Speech Trigger](#data-flow-speech-trigger)
  - [Configuration Structure](#configuration-structure)
  - [Security Considerations](#security-considerations)
    - [Tauri Capabilities](#tauri-capabilities)
    - [CLI Execution](#cli-execution)
    - [API Keys](#api-keys)
  - [Performance Considerations](#performance-considerations)
  - [Multi-Window Design](#multi-window-design-1)
  - [Backend Module Structure](#backend-module-structure-1)
    - [Module Responsibilities](#module-responsibilities-1)
      - [`clipboard.rs` - Clipboard State Machine](#clipboardrs---clipboard-state-machine)
      - [`config.rs` - Configuration Persistence](#configrs---configuration-persistence)
      - [`history.rs` - Speech History Logging](#historyrs---speech-history-logging)
      - [`autostart.rs` - Windows Startup Integration](#autostartrs---windows-startup-integration)
      - [`tts/` - Backend Abstraction](#tts---backend-abstraction)
  - [Frontend Architecture](#frontend-architecture-1)
    - [Technology Stack](#technology-stack-1)
  - [IPC Commands](#ipc-commands-1)
    - [IPC Events (Rust → Frontend)](#ipc-events-rust--frontend)
  - [State Management](#state-management-1)
    - [Backend State (Rust)](#backend-state-rust-1)
    - [Frontend State (Svelte)](#frontend-state-svelte-1)
  - [Data Flow: Speech Trigger](#data-flow-speech-trigger-1)
  - [Configuration Structure](#configuration-structure-1)
  - [Global Hotkey](#global-hotkey)
    - [Architecture](#architecture)
    - [Data Flow](#data-flow)
    - [Configuration](#configuration)
  - [Security Considerations](#security-considerations-1)
    - [Tauri Capabilities](#tauri-capabilities-1)
    - [CLI Execution](#cli-execution-1)
    - [API Keys](#api-keys-1)
  - [Performance Considerations](#performance-considerations-1)
  - [Deferred Features](#deferred-features)
  - [Implemented Features](#implemented-features)
  - [Future Considerations](#future-considerations)

---

## Overview

CopySpeak is a Windows 11 desktop application designed to monitor the system clipboard and trigger text-to-speech (TTS) when the same text is copied twice within a configurable time window (double-copy trigger), or via a global hotkey.

### Design Philosophy

CopySpeak is designed as an orchestrator, not a self-contained TTS solution:

- Users install their own TTS engine (e.g., kokoro-tts, piper, espeak) or use cloud APIs (OpenAI, ElevenLabs).
- CopySpeak calls the engine via CLI or HTTP.
- This approach enables flexibility and allows users to leverage the best TTS technology available.

---

## System Architecture

CopySpeak is built as a Tauri v2 application with a Rust backend and a Svelte 5 frontend. The system architecture is designed to be modular and extensible.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Windows 11 System                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────────────────────────────────┐   │
│  │   Clipboard  │◄───│  Win32 AddClipboardFormatListener        │   │
│  └──────────────┘    └──────────────────────────────────────────┘   │
│         │                                                           │
│         ▼                                                           │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    Tauri v2 Application                      │   │
│  │  ┌────────────────────────────────────────────────────────┐  │   │
│  │  │                   Rust Backend                         │  │   │
│  │  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │  │   │
│  │  │  │ clipboard.rs │ │  config.rs   │ │  commands.rs │    │  │   │
│  │  │  │ State Machine│ │ Persistence  │ │ IPC Handlers │    │  │   │
│  │  │  └──────────────┘ └──────────────┘ └──────────────┘    │  │   │
│  │  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │  │   │
│  │  │  │   audio.rs   │ │ sanitize.rs  │ │  history.rs  │    │  │   │
│  │  │  │ rodio + WAV  │ │  Text Norm   │ │ Speech Log   │    │  │   │
│  │  │  └──────────────┘ └──────────────┘ └──────────────┘    │  │   │
│  │  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │  │   │
│  │  │  │   tts/       │ │  autostart   │ │   (deferred) │    │  │   │
│  │  │  │ CLI/HTTP     │ │  Windows     │ │ hud, filter, │    │  │   │
│  │  │  └──────────────┘ └──────────────┘ │language,     │    │  │   │
│  │  │                                    │app_source    │    │  │   │
│  │  │                                    └──────────────┘    │  │   │
│  │  └────────────────────────────────────────────────────────┘  │   │
│  │                           │ IPC                              │   │
│  │  ┌────────────────────────▼───────────────────────────────┐  │   │
│  │  │                 Svelte 5 Frontend                      │  │   │
│  │  │  ┌────────────────────────────────────────────────┐    │  │   │
│  │  │  │         Main Window                            │    │  │   │
│  │  │  │      Settings & Status UI                      │    │  │   │
│  │  │  │   HUD overlay with waveform and clipboard      │    │  │   │
│  │  │  └────────────────────────────────────────────────┘    │  │   │
│  │  └────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │ Wrapped around                       │
│  ┌───────────────────────────▼──────────────────────────────────┐   │
│  │              External TTS Engine (CLI or API)                │   │
│  │  kitten-tts, kokoro-tts, piper, OpenAI, ElevenLabs (etc...)  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Multi-Window Design

CopySpeak employs a multi-window design to separate concerns and improve user experience. The main window handles settings and status, while the HUD overlay window provides real-time visual feedback during playback and clipboard operations.

---

## Backend Module Structure

The backend is structured into several modules, each with specific responsibilities:

### Module Responsibilities

- **`clipboard.rs` - Clipboard State Machine**: Manages the clipboard monitoring and state transitions.
- **`config.rs` - Configuration Persistence**: Handles the loading and saving of user configurations.
- **`sanitize.rs` - Text Normalization**: Normalizes text before it is sent to the TTS engine.
- **`history.rs` - Speech History Logging**: Logs the history of speech triggers.
- **`autostart.rs` - Windows Startup Integration**: Manages the application's startup with Windows.
- **`tts/` - Backend Abstraction**: Provides an abstraction layer for different TTS engines.

---

## Frontend Architecture

### Technology Stack

The frontend is built with Svelte 5, providing a reactive and efficient user interface. The main window includes settings and status information, while the HUD overlay window provides clipboard feedback and an overview of the copied text overlayed over a waveform visualization of the audio being played, .

---

## IPC Commands

Inter-process communication (IPC) is used to facilitate communication between the Rust backend and the Svelte frontend. This includes commands for clipboard monitoring, configuration updates, and TTS triggers.

---

## State Management

### Backend State (Rust)

The backend state is managed using Rust's state management facilities, ensuring efficient and safe state transitions.

### Frontend State (Svelte)

The frontend state is managed using Svelte's reactive state management, providing a seamless user experience.

---

## Data Flow: Speech Trigger

1. Clipboard monitoring detects a double-copy event.
2. The text is normalized and checked against the configuration.
3. The appropriate TTS engine is called via CLI or HTTP.
4. The speech is played back to the user.

---

## Configuration Structure

The configuration is structured to allow users to customize the double-copy time window, TTS engine settings, and other preferences.

---

## Security Considerations

### Tauri Capabilities

CopySpeak uses Tauri's capabilities to ensure secure interactions with the system clipboard and TTS engines.

### CLI Execution

CLI commands for TTS engines are executed with appropriate permissions and validations.

### API Keys

API keys for cloud-based TTS services are stored securely and managed through the application's configuration.

---

## Performance Considerations

CopySpeak is designed to be lightweight and efficient, with minimal impact on system performance. Clipboard monitoring is optimized to reduce CPU usage.

## Multi-Window Design

CopySpeak uses Tauri's multi-window architecture:

| Window | File         | Purpose                           | Properties                                               |
| ------ | ------------ | --------------------------------- | -------------------------------------------------------- |
| Main   | `index.html` | Settings and status UI            | 1280x720, centered, visible                              |
| HUD    | `hud.html`   | Waveform visualization & feedback | Variable size, always-on-top, transparent, click-through |

The HUD overlay provides real-time visual feedback including waveform visualization during playback and "Clipboard Copied" notifications during double-copy detection.

---

## Backend Module Structure

```
src-tauri/src/
├── main.rs              # App setup, tray icon, IPC command registration
├── clipboard.rs         # Double-copy detection state machine
├── autostart.rs         # Windows startup registration
├── fragment_queue.rs    # Text pagination queue management
├── pagination.rs        # Text splitting for long content
├── logging.rs           # Application logging
├── history.rs           # Speech history logging
├── history_manager.rs   # History entry management
├── audio/               # Audio playback (directory-based)
│   ├── mod.rs           # Module exports
│   ├── player.rs        # AudioPlayer implementation
│   ├── wav.rs           # WAV parsing
│   ├── stream.rs        # Streaming utilities
│   └── format.rs        # Audio format handling
├── commands/            # Tauri IPC commands (directory-based)
│   ├── mod.rs           # Module exports, command registration
│   ├── config.rs        # Config get/set commands
│   ├── tts.rs           # TTS synthesis commands
│   ├── playback.rs      # Playback control commands
│   ├── history.rs       # History management commands
│   └── queue.rs         # Queue management commands
├── config/              # Configuration (directory-based)
│   ├── mod.rs           # AppConfig, load/save
│   ├── tts.rs           # TTS config
│   ├── playback.rs      # Playback config
│   ├── trigger.rs       # Trigger config
│   ├── general.rs       # General config
│   ├── output.rs        # Output config
│   ├── hotkey.rs        # Global hotkey config
│   ├── sanitization.rs  # Sanitization config
│   └── tests.rs         # Config tests
├── sanitize/            # Text normalization (directory-based)
│   ├── mod.rs           # Module exports
│   ├── markdown.rs      # Markdown stripping
│   ├── tts_normalize.rs # TTS text normalization
│   └── cleanup.rs       # General cleanup
└── tts/                 # TTS backends (directory-based)
    ├── mod.rs           # TtsBackend trait
    ├── cli.rs           # CLI TTS (piper, kokoro, etc.)
    ├── http.rs          # HTTP TTS (REST API)
    ├── openai.rs        # OpenAI TTS
    └── elevenlabs.rs    # ElevenLabs TTS
```

**Deferred modules** (available on `features-extras` branch):

- `filter.rs` — Content sanitization and regex-based filtering
- `language.rs` — Language detection and voice auto-selection
- `app_source.rs` — Application-specific whitelist/blacklist filtering

### Module Responsibilities

#### `clipboard.rs` - Clipboard State Machine

The double-copy detection follows a state machine pattern:

```
IDLE ──(clipboard change)──► ARMED ──(same text within window)──► SPEAK
  ▲                            │
  └────(different text)────────┘
  └────(timeout)───────────────┘
```

#### `config.rs` - Configuration Persistence

- Loads/saves to `%APPDATA%/CopySpeak/config.json`
- Provides default values for all settings
- Auto-creates config directory on first run

.#### `sanitize/` - Text Normalization Pipeline

Three-pass multi-stage pipeline in `src-tauri/src/sanitize/`:

**Pass 1 — Markdown Stripping** (`markdown.rs`, optional):

- Code blocks and inline code removed
- Links: `[text](url)` → `text`
- Headers: `# Heading` → `Heading.` (period appended for TTS sentence boundary; skipped if heading already ends with `.?!:;`)
- Bold/italic markers, list prefixes, blockquote markers removed

**Pass 2 — TTS Normalization** (`tts_normalize.rs`, optional):
Priority order:

1. Emoji removal (Unicode ranges: 1F300–1F9FF, 1FA00–1FAFF, 2600–27BF, etc.)
2. URL removal
3. Citation removal (`[1]`, `[a]`)
4. Slash lookups (`w/o` → `without`, `w/` → `with`, `n/a`)
5. Slash options (`true/false` → `true or false`)
6. Slash ratios (`100 km/h` → `100 km per h`)
7. Latin abbreviations (`e.g.` → `for example`, `etc.` → `et cetera`)
8. Title abbreviations (`Dr.` → `Doctor`, `Prof.` → `Professor`)
9. Number suffixes (`5m` → `5 million`, `2bn` → `2 billion`)
10. Metric units (`10km` → `10 kilometers`, `5cm` → `5 centimeters`)
11. Symbols (`&` → `and`, `$50` → `50 dollars`, `°` → `degrees`)
12. Punctuation normalization (em-dash → comma, parentheses → comma-delimited)
13. Artifact cleanup (double spaces, comma artifacts)
14. **Newline stripping** (replaced with single space — newlines have no effect in TTS)

**Pass 3 — Artifact Cleanup** (`cleanup.rs`, always runs):

- Collapses multiple spaces and blank lines
- Fixes spacing around punctuation
- Removes double commas, trailing commas
- Trims whitespace

**Note:** Content filtering rules (regex-based filter patterns) are deferred and available on `features-extras` branch.

#### `history.rs` - Speech History Logging

- Persistent log of all spoken text
- Timestamp and metadata tracking
- Configurable history size limits

#### `autostart.rs` - Windows Startup Integration

- Registers/unregisters app with Windows startup
- Registry key management
- User preference persistence

#### `tts/` - Backend Abstraction

The `TtsBackend` trait enables swapping TTS engines:

```rust
pub trait TtsBackend: Send + Sync {
    fn name(&self) -> &str;
    fn synthesize(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>, TtsError>;
    fn health_check(&self) -> Result<(), TtsError>;
    fn supports_streaming(&self) -> bool { false }
}
```

**Supported backends:**

| Backend                | Type             | Best For                                                     |
| ---------------------- | ---------------- | ------------------------------------------------------------ |
| **CLI Backend**        | Local process    | Offline use, privacy, local voice models (kokoro, piper)     |
| **HTTP Backend**       | Generic REST API | Custom TTS servers, self-hosted solutions                    |
| **OpenAI Backend**     | Cloud API        | Quick setup, good quality, 6 built-in voices                 |
| **ElevenLabs Backend** | Cloud API        | Best quality, voice cloning, 1000+ voices, advanced controls |

**ElevenLabs Features:**

- **Voice Management**: Dynamic voice listing from user's account
- **Output Formats**: MP3 (128/192kbps), PCM, FLAC, OGG (configurable)
- **Voice Settings**: Stability, similarity boost, style, speaker boost
- **Models**: Multilingual v2 (29 languages), Turbo variants for speed
- **Playback Control**: Speed and pitch are adjusted via browser frontend playback rate (not at generation level)

**Cloud Backend Configuration:**

```json
{
  "tts": {
    "active_backend": "elevenlabs",
    "elevenlabs": {
      "api_key": "xi-...",
      "voice_id": "21m00Tcm4TlvDq8ikWAM",
      "model_id": "eleven_turbo_v2_5",
      "output_format": "mp3_44100_128",
      "voice_stability": 0.5,
      "voice_similarity_boost": 0.75
    }
  }
}
```

---

## Frontend Architecture

```
src/
├── lib/
│   ├── assets/
│   │   └── app-logo.png
│   ├── components/
│   │   ├── history/                             # History panel components
│   │   │   ├── export-dialog.svelte
│   │   │   ├── history-bulk-actions.svelte
│   │   │   ├── history-entry.svelte
│   │   │   └── history-search.svelte
│   │   ├── layout/                              # Layout components
│   │   │   ├── app-footer.svelte
│   │   │   └── app-header.svelte
│   │   ├── settings/                            # Settings panel components
│   │   │   ├── appearance-settings.svelte
│   │   │   ├── batch-settings.svelte
│   │   │   ├── general-settings.svelte
│   │   │   ├── history-settings.svelte
│   │   │   ├── import-export-settings.svelte
│   │   │   ├── playback-settings.svelte
│   │   │   ├── sanitization-settings.svelte
│   │   │   ├── trigger-settings.svelte
│   │   │   └── tts-settings.svelte
│   │   ├── ui/                                  # Shadcn-Svelte UI components
│   │   │   └── ...
│   │   ├── clipboard-display.svelte
│   │   ├── playback-controls.svelte
│   │   ├── quick-settings.svelte
│   │   ├── recent-history.svelte
│   │   ├── settings-panel.svelte
│   │   ├── status-dashboard.svelte
│   │   ├── synthesize-page.svelte
│   │   ├── theme-toggle.svelte
│   │   └── virtual-list.svelte
│   ├── hooks/                                   # Svelte hooks
│   ├── models/                                  # Data models
│   │   ├── history.ts
│   │   ├── html-export.ts
│   │   └── index.ts
│   ├── services/                                # Tauri service bindings
│   │   └── tauri.ts
│   ├── stores/                                  # Svelte stores
│   │   ├── history-store.svelte.ts
│   │   ├── index.ts
│   │   └── listening-store.svelte.ts
│   ├── utils/                                   # Utility functions
│   │   ├── history-events.ts
│   │   ├── html-export.ts
│   │   └── html-export.test.ts
│   ├── types.ts
│   ├── utils.ts
│   └── version.ts
├── routes/
│   ├── settings/
│   │   └── +page.svelte
│   ├── +layout.css
│   ├── +layout.svelte
│   ├── +layout.ts
│   ├── +page.svelte
│   └── +page.ts
└── app.html
```

**Adding shadcn-svelte components:**

```bash
bun x shadcn-svelte@latest add <component>
```

**Available components:** `accordion`, `alert`, `alert-dialog`, `aspect-ratio`, `avatar`, `badge`, `breadcrumb`, `button-group`, `button`, `calendar`, `card`, `carousel`, `chart`, `checkbox`, `collapsible`, `combobox`, `command`, `context-menu`, `data-table`, `date-picker`, `dialog`, `drawer`, `dropdown-menu`, `empty`, `field`, `formsnap`, `hover-card`, `input-group`, `input-otp`, `input`, `item`, `kbd`, `label`, `menubar`, `native-select`, `navigation-menu`, `pagination`, `popover`, `progress`, `radio-group`, `range-calendar`, `resizable`, `scroll-area`, `select`, `separator`, `sheet`, `sidebar`, `skeleton`, `slider`, `sonner`, `spinner`, `switch`, `table`, `tabs`, `textarea`, `toggle-group`, `toggle`, `tooltip`, `typography`

### Technology Stack

- **Svelte 5** with runes (`$state`, `$effect`, `$derived`, `$props`)
- **SvelteKit** with static adapter for Tauri
- **Tailwind CSS v4.2** via `@tailwindcss/vite`
- **shadcn-svelte** for UI components
- **mode-watcher** for dark/light theme support
- **Vite 7** for bundling with multi-page support

---

## IPC Commands

Commands exposed from Rust to the frontend:

| Command                   | Purpose                                         |
| ------------------------- | ----------------------------------------------- |
| `get_config`              | Retrieve current AppConfig                      |
| `set_config`              | Update and persist AppConfig                    |
| `speak_now`               | Trigger TTS for given text or clipboard content |
| `speak_history_entry`     | Re-synthesize and play a history entry          |
| `play_history_entry`      | Play saved audio from a history entry           |
| `stop_speaking`           | Stop current audio playback                     |
| `toggle_pause`            | Pause/resume playback                           |
| `replay_cached`           | Replay the last synthesized audio               |
| `get_playback_state`      | Check if audio is playing/paused                |
| `set_listening`           | Enable/disable clipboard monitoring             |
| `get_history`             | Retrieve speech history log                     |
| `clear_history`           | Clear all speech history                        |
| `delete_history_entry`    | Remove a single history entry                   |
| `copy_history_entry_text` | Copy entry text to clipboard                    |
| `test_tts`                | Test TTS engine with sample text                |

### IPC Events (Rust → Frontend)

| Event                         | Payload        | Emitted When                                     |
| ----------------------------- | -------------- | ------------------------------------------------ |
| `history-updated`             | `()`           | After any TTS synthesis adds a new history entry |
| `synthesis-state-change`      | `bool`         | Synthesis starts (`true`) or ends (`false`)      |
| `speak-request`               | `{ text }`     | Double-copy trigger detected                     |
| `clipboard-change`            | `{ text }`     | Clipboard content changes                        |
| `text-truncated`              | lengths        | Text was truncated due to max length limit       |
| `pagination:started`          | fragment count | Multi-fragment synthesis begins                  |
| `pagination:fragment-started` | index          | Individual fragment synthesis starts             |
| `pagination:stopped`          | index          | Playback stopped mid-pagination                  |

---

## State Management

### Backend State (Rust)

State is managed via `Mutex`-wrapped structs using Tauri's `app.manage()`:

```rust
app.manage(Mutex::new(config));
app.manage(Mutex::new(audio_player));
app.manage(Mutex::new(history));
```

### Frontend State (Svelte)

Uses Svelte 5 runes for reactive state:

```svelte
let config = $state<AppConfig | null>(null);
let isPlaying = $derived(config?.playback.is_playing ?? false);
```

---

## Data Flow: Speech Trigger

```
1. User copies text (Ctrl+C)
    └─► Win32 clipboard listener detects change

2. Clipboard state machine processes
    └─► If double-copy detected: proceed
    └─► If single copy: arm timer and wait

3. Text sanitization pipeline
    └─► Strip markdown formatting
    └─► Normalize TTS text (URLs, abbreviations, symbols)
    └─► Apply character truncation if needed

4. Text pagination (if enabled)
    └─► Split long text into fragments
    └─► Queue fragments for sequential synthesis

5. TTS backend synthesizes text
    └─► CLI backend spawns external process
    └─► HTTP backend makes API call
    └─► Receives WAV bytes

6. Audio player receives WAV
    └─► Decodes and plays via rodio
    └─► Applies volume setting

7. History logging
    └─► Log text with timestamp, voice, duration
    └─► Store in persistent history (JSON on disk)
    └─► Emit `history-updated` event to frontend

8. Frontend refresh
    └─► synthesize-page.svelte listens for `history-updated`
    └─► Calls historyStore.refresh() → re-fetches from backend
    └─► recent-history.svelte re-renders with new items
```

**Deferred features** (on `features-extras` branch):

- Content filtering rules (prevent speaking sensitive data)
- Language detection with auto voice selection
- Application-specific whitelist/blacklist filtering

**Implemented features**:

- HUD waveform visualization with amplitude envelope and clipboard notifications

---

## Configuration Structure

```json
{
  "trigger": {
    "listen_enabled": true,
    "double_copy_window_ms": 1500,
    "max_text_length": 100000
  },
  "tts": {
    "active_backend": "local",
    "preset": "kokoro",
    "command": "kokoro-tts",
    "args_template": ["--text", "{text}", "--output", "{output}", "--voice", "{voice}", "--speed", "{speed}"],
    "voice": "af_nicole",
    "speed": 1.0,
    "openai": {
      "api_key": "",
      "model": "tts-1",
      "voice": "alloy"
    },
    "elevenlabs": {
      "api_key": "",
      "voice_id": "21m00Tcm4TlvDq8ikWAM",
      "model_id": "eleven_turbo_v2_5",
      "output_format": "mp3_44100_128",
      "voice_stability": 0.5,
      "voice_similarity_boost": 0.75
    }
  },
  "playback": {
    "on_retrigger": "queue",
    "volume": 100,
    "playback_speed": 1.35,
    "pitch": 1.15
  },
  "hud": {
    "enabled": true,
    "position": "bottom-center",
    "width": 300,
    "height": 100,
    "opacity": 0.85
  },
  "hotkey": {
    "enabled": false,
    "shortcut": "Super+Shift+A"
  },
  "general": {
    "start_with_windows": false,
    "start_minimized": true,
    "show_notifications": true,
    "debug_mode": false,
    "close_behavior": "minimize-to-tray",
    "appearance": "system",
    "locale": "en"
  },
  "output": {
    "enabled": false,
    "directory": "",
    "filename_pattern": "{date}_{time}_{seq}",
    "format_config": {
      "format": "wav",
      "mp3_bitrate": 192,
      "ogg_bitrate": 128,
      "flac_compression": 5
    }
  },
  "sanitization": {
    "markdown_enabled": true,
    "tts_normalize_enabled": true
  },
  "pagination": {
    "enabled": false,
    "fragment_size": 500
  },
  "history": {
    "enabled": true,
    "max_entries": 1000,
    "max_age_days": 30,
    "auto_cleanup_enabled": true,
    "auto_cleanup_interval_hours": 24,
    "save_audio": true,
    "cleanup_orphaned_files": true
  }
}
```

---

## Global Hotkey

The global hotkey feature provides an alternative trigger method to the double-copy detection:

### Architecture

- **Plugin**: `tauri-plugin-global-shortcut` registers system-wide keyboard shortcuts
- **Config**: `HotkeyConfig` in `src-tauri/src/config/hotkey.rs`
- **UI**: `HotkeySettings` component in `src/lib/components/settings/hotkey-settings.svelte`

### Data Flow

```
1. User presses hotkey (e.g., Win+Shift+A)
    └─► Global shortcut plugin detects key combination
    └─► Handler spawns async task

2. Handler calls speak_now()
    └─► Retrieves clipboard text
    └─► Sanitizes text
    └─► Synthesizes speech
    └─► Plays audio

3. Hotkey changes detected in set_config()
    └─► Unregisters old shortcut
    └─► Registers new shortcut
```

### Configuration

```rust
pub struct HotkeyConfig {
    pub enabled: bool,      // Master toggle for hotkey feature
    pub shortcut: String,   // Key combination (e.g., "Super+Shift+A")
}
```

Validation ensures:
- At least one modifier (Ctrl, Alt, Shift, or Super/Win)
- Non-empty shortcut string when enabled

---

## Security Considerations

### Tauri Capabilities

Permissions are defined in `src-tauri/capabilities/default.json`:

- Core defaults
- Window management (create, show, hide, position, focus, close)
- Event system (emit, listen)
- Global shortcut plugin (`global-shortcut:default`)
- File system access (for audio save mode)

### CLI Execution

The CLI TTS backend spawns external processes. Security considerations:

- User controls which TTS engine is installed
- Command and args are configurable but stored locally
- No remote execution
- Input sanitization via filter module

### API Keys

- API keys stored in local config file
- Config directory has appropriate permissions
- Keys are never transmitted except to configured endpoints

---

## Performance Considerations

1. **Clipboard Polling vs Events**: Using Win32 `AddClipboardFormatListener` instead of polling for efficiency
2. **Audio Buffering**: rodio handles double-buffering automatically
3. **HUD Rendering**: Minimal canvas/SVG updates for waveform
4. **State Updates**: Selective re-renders via Svelte's fine-grained reactivity
5. **History Management**: Circular buffer with configurable size limits
6. **Filter Processing**: Compiled regex patterns for efficient matching

---

## Deferred Features

The following 4 features have been deferred for future release and are preserved on the `features-extras` branch:

1. **Language Detection** — Auto-detect text language for voice selection
2. **Content Filtering** — Regex-based rules to prevent speaking sensitive data
3. **Application Filter** — Whitelist/blacklist specific applications
4. **Batch Processing** — Process multiple texts sequentially with UI

To access these features:

```bash
git checkout features-extras
```

## Implemented Features

The following features have been implemented and are available in the main branch:

- **HUD Overlay** — Transparent waveform visualization during playback with clipboard notification feedback
- **Global Hotkey** — Single configurable hotkey (default: Win+Shift+A) to trigger speech from clipboard content, providing an alternative to the double-copy trigger

## Future Considerations

- **Multiple Voice Profiles**: Quick-switch between voice configurations
- **Clipboard History**: Replay recent clips without re-copying
- **Cross-Platform**: macOS/Linux support (clipboard API abstraction needed)
- **Pronunciation Dictionary**: Custom word pronunciations
- **Update Checker**: Automated version checking
- **Usage Statistics**: Local tracking of TTS activity
