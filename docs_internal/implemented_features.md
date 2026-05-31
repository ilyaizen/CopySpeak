# Implemented Features

This document serves as a distilled reference of the features currently implemented in CopySpeak.

> **Note (2026-02-24):** Six features have been deferred for future releases and moved to the `features-extras` branch. See the **Deferred Features** section at the end of this document for details.

## Core

### Application Update Checker

**Status:** verified

Checks GitHub releases API for new CopySpeak versions on startup (respects check frequency setting). Shows notification with download link when update available.

**Implementation Details:**

- Uses `tauri-plugin-updater` for seamless updates from GitHub Releases
- Endpoint: `https://github.com/ilyaizen/CopySpeak/releases/latest/download/latest.json`
- Update artifacts signed with minisign (private key stored in GitHub secrets)
- Configuration requirements in `tauri.conf.json`:
  - `bundle.createUpdaterArtifacts: true` — Enables generation of `latest.json` manifest
  - `plugins.updater.pubkey` — Minisign public key for signature verification
  - `plugins.updater.endpoints` — URL to `latest.json` on GitHub Releases
- GitHub Actions workflow:
  - `TAURI_SIGNING_PRIVATE_KEY` secret must match the pubkey in config
  - Releases must be published (not draft) for `/latest/` endpoint to work
- Frontend: `update-checker.svelte` component in footer with check/download/install flow
- Settings toggle in General settings to enable/disable automatic update checks

### Application-Specific Whitelist/Blacklist

**Status:** deferred → features-extras

Configure which applications trigger clipboard monitoring. Detects source app of clipboard change and applies whitelist/blacklist rules. Moved to `features-extras` branch for future release.

### Audio Format Conversion Options

**Status:** verified

Converts saved audio to MP3, OGG, or FLAC formats using external encoder (ffmpeg). Configurable bitrate and quality settings per format.

### Audio Output Device Selection

**Status:** verified

Enumerates available audio output devices and allows user to select specific device for TTS playback. Integrates with rodio device enumeration API.

### Audio Skip Forward/Backward

**Status:** verified

Hotkeys to skip forward/backward by 5 seconds during TTS playback. Implements seek functionality in AudioPlayer using rodio sink controls.

### Automatic Language Detection

**Status:** deferred → features-extras

Detects clipboard text language using heuristics (character sets, common words). Auto-selects appropriate voice preset based on detected language if configured. Moved to `features-extras` branch for future release.

### Batch Text-to-Speech Processing

**Status:** deferred → features-extras

UI to paste or import multiple text snippets and generate audio for all. Saves to numbered files in selected directory with progress indicator. Moved to `features-extras` branch for future release.

### Clipboard Content Filter Rules

**Status:** deferred → features-extras

Regex-based rules to ignore certain clipboard content patterns (e.g., passwords, credit cards, hex strings). Prevents accidental speaking of sensitive data. Moved to `features-extras` branch for future release.

### Comprehensive Error Logging

**Status:** verified

Logs all errors and warnings to %APPDATA%/CopySpeak/logs/app.log with timestamps and context. Implements log rotation (max 10MB per file, keep 5 files).

### Configuration Preset Import/Export

**Status:** backlog

Export entire app config or individual presets to JSON file. Import configs from file with merge or replace options. Enables sharing configurations.

### Configuration Validation System

**Status:** verified

Validates all config values before saving to prevent invalid states. Checks window bounds, speed ranges (0.5-2.0), opacity (0.0-1.0), hotkey format, and command template validity.

### Custom Pronunciation Dictionary

**Status:** backlog

User-defined word/phrase substitutions before TTS processing. Maps words to phonetic spellings or alternative text (e.g., 'LOL' → 'laugh out loud').

### Debug Mode with Verbose Logging

**Status:** verified

Hidden debug mode toggle in settings that enables verbose logging of all IPC calls, clipboard events, and TTS operations. Useful for troubleshooting.

### Global Hotkey Registration

**Status:** deferred → features-extras

Registers user-configured global shortcut using tauri-plugin-global-shortcut. Triggers speak_now command when hotkey is pressed system-wide. Moved to `features-extras` branch for future release.

### HTTP API TTS Backend

**Status:** verified

Alternative TtsBackend implementation that calls HTTP endpoints for TTS synthesis. Supports templated URL, headers, and JSON body with placeholder tokens.

### Local Usage Statistics Tracking

**Status:** backlog

Tracks local statistics: total speeches, total duration, most used voice, words per day. Displays in dedicated stats view with charts. No data leaves device.

### Maximum Text Length Enforcement

**Status:** verified

Configurable max character limit (default 50,000) to prevent extremely long clipboard content from being spoken. Shows truncation warning in notification.

### Multi-Monitor HUD Positioning

**Status:** deferred → features-extras

Detects connected monitors and allows user to select which screen HUD appears on. Calculates positions relative to selected monitor bounds. Moved to `features-extras` branch for future release.

### Pause/Resume Playback Control

**Status:** verified

Global hotkey and UI button to pause/resume current TTS playback without stopping. Preserves position in audio stream using rodio sink pause/play.

### Playback Speed Adjustment Hotkeys

**Status:** verified

Global hotkeys to increase/decrease TTS speed during playback (e.g., Ctrl+Shift+Up/Down). Adjusts browser frontend playback rate directly on the audio output, allowing real-time speed and pitch changes without re-synthesis.

### Playback Volume Control

**Status:** verified

Adds volume adjustment to AudioPlayer (0-100%). Exposes volume slider in settings and quick volume control in status view. Applies gain to rodio sink.

### Save Audio Without Playback Mode

**Status:** verified

Config option to save generated TTS audio to file instead of playing. User specifies output directory pattern with {date}, {time}, {hash} placeholders.

### Save Last Spoken Audio Command

**Status:** verified

IPC command and UI button to save the most recently spoken audio to user-selected file location. Caches last generated WAV bytes in memory.

### Speech History Persistence

**Status:** verified

Logs all TTS events to %APPDATA%/CopySpeak/history.json with timestamp, text snippet (first 50 chars), voice, and duration. Implements circular buffer with max 1000 entries.

### SSML Markup Support

**Status:** verified

Allows clipboard text to contain SSML tags for advanced pronunciation control (pause, emphasis, phonemes). Passes SSML through to TTS engines that support it.

### Streaming TTS Playback

**Status:** verified

Begins playing audio as soon as first chunks arrive from TTS engine instead of waiting for complete synthesis. Reduces perceived latency for long texts.

### Text Preprocessing Pipeline

**Status:** backlog

Normalizes clipboard text before TTS: trims whitespace, removes URLs, expands common abbreviations, handles markdown formatting. Configurable rules in settings.

### Tray Listening Toggle Handler

**Status:** verified

Connects tray menu 'Toggle Listening' action to set_listening IPC command. Updates menu item label to reflect current state (Pause/Resume Listening).

### Tray Speak Now Handler

**Status:** verified

Connects tray menu 'Speak Clipboard Now' action to speak_now IPC command. Provides quick TTS trigger without opening main window.

### Voice Preset Management

**Status:** backlog

System for saving and loading multiple voice/engine configurations as named presets. Allows users to quickly switch between different TTS setups (e.g., 'Fast English', 'Natural Japanese').

### WAV Amplitude Envelope Parser

**Status:** verified

Full WAV parsing implementation to extract sample rate, channels, and bits per sample. Computes RMS values per chunk and normalizes amplitude data for HUD waveform visualization.

### Win32 Clipboard Listener Implementation

**Status:** verified

Replace polling fallback with proper Win32 AddClipboardFormatListener integration. Implements read_clipboard_text() using Windows API for efficient clipboard monitoring without CPU overhead.

### Windows Auto-Start Registration

**Status:** verified

Implements start_with_windows config by adding/removing registry entry in HKCU\Software\Microsoft\Windows\CurrentVersion\Run. Handles elevated permissions gracefully.

## UI

### Clipboard Text Preview in Status

**Status:** removed

Feature removed. No longer needed - users can type or paste text directly into the synthesis input field.

### Custom Hotkey Configuration UI

**Status:** deferred → features-extras

Settings panel with interactive hotkey capture for all available actions. Validates for conflicts and invalid combinations. Moved to `features-extras` branch for future release.

### HUD Drag-to-Position Mode

**Status:** deferred → features-extras

Alternative HUD positioning mode where user drags HUD to desired screen location. Saves custom X/Y coordinates instead of using preset positions. Moved to `features-extras` branch for future release.

### HUD Visual Theme Customization

**Status:** deferred → features-extras

Settings for HUD appearance: waveform color, background color/opacity, border radius, animation speed. Includes dark/light theme presets. Moved to `features-extras` branch for future release.

### HUD Waveform Visualization Component

**Status:** deferred → features-extras

Svelte component that renders animated waveform using canvas or SVG. Listens to hud:start events and displays amplitude envelope data with smooth animations. Moved to `features-extras` branch for future release.

### Keyboard Shortcuts Help Dialog

**Status:** deferred → features-extras

Overlay showing all configured hotkeys and their functions. Displays current bindings from config. Moved to `features-extras` branch for future release.

### Main Window Settings View

**Status:** verified

Comprehensive settings interface for configuring trigger options, TTS engine selection, playback modes, HUD preferences, and general app behavior. Uses form components with real-time validation.

### Main Window Status View

**Status:** verified

Status dashboard showing listening state indicator, current voice/engine info, last spoken text preview, and Pause/Speak Now action buttons. Provides quick access to core functions.

### Minimize to Tray Behavior

**Status:** verified

Intercepts main window close button to minimize to tray instead of quitting. Adds 'Exit' vs 'Minimize to Tray' option in settings.

### Speech History Viewer

**Status:** backlog

Dedicated view showing speech history with search/filter. Allows re-speaking past entries, copying text, and clearing history. Paginated list with virtual scrolling.

### Statistics Dashboard View

**Status:** backlog

Visual dashboard with charts showing usage trends over time, most spoken words, peak usage hours. Uses Chart.js or similar for visualizations.

### Status/Settings View Toggle

**Status:** backlog

Gear icon button that switches between status and settings views in main window. Implements smooth transition animations and preserves unsaved changes warning.

### TTS Engine Health Check UI

**Status:** backlog

Settings panel button to test TTS engine availability. Displays success/error message with specific diagnostics (command not found, permission issues, etc.).

### TTS Engine Quick Install Guide

**Status:** backlog

In-app wizard showing installation instructions for popular TTS engines (kokoro-tts, piper, espeak). Includes download links and command examples.

### TTS Queue Viewer Panel

**Status:** backlog

Shows pending TTS items when in queue mode. Displays text snippets, estimated duration, and allows reordering or removing items from queue.

---

## Deferred Features (features-extras branch)

The following 6 features have been deferred for future release. They are fully implemented but have been moved to the `features-extras` branch to create a clean, focused v0.2 release. To access them:

```bash
git checkout features-extras
```

### Deferred Feature List

1. **Automatic Language Detection** — Auto-detect text language for voice selection
2. **Clipboard Content Filter Rules** — Regex-based rules to prevent speaking sensitive data
3. **Application-Specific Whitelist/Blacklist** — Per-app clipboard monitoring
4. **Global Hotkey Registration** — System-wide keyboard shortcuts for control
5. **HUD Overlay System** — Transparent waveform visualization during playback (includes 3 related features)
   - HUD Waveform Visualization Component
   - HUD Drag-to-Position Mode
   - HUD Visual Theme Customization
   - Multi-Monitor HUD Positioning
6. **Batch Text-to-Speech Processing** — Process multiple texts sequentially
7. **Keyboard Shortcuts Help Dialog** — Overlay showing all hotkeys
8. **Custom Hotkey Configuration UI** — Interactive hotkey capture in settings

### Why Deferred?

These features were implemented fully but are less critical to the core use case (clipboard monitoring + TTS). By moving them to a separate branch:

- ✅ **Cleaner main branch** — Focuses on core clipboard-to-speech functionality
- ✅ **Faster builds** — Removes 4 unused dependencies (whatlang, tauri-plugin-global-shortcut, regex, lazy_static)
- ✅ **Reduced complexity** — Less code in core paths, easier to debug
- ✅ **Preserved work** — All implementations are safe on `features-extras` for future integration
- ✅ **Better testing** — v0.2 release can be thoroughly tested on stable feature set
- ✅ **Clear roadmap** — Features can be re-evaluated for v0.3+ based on user feedback

All removed code from `main` is intact on `features-extras` and can be cherry-picked or merged in future releases.
