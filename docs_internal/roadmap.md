# CopySpeak Implementation Roadmap

> **Last Updated:** 2026-03-25
> **Version:** v0.1.0 (HUD completed; TTS Engine overhaul completed)
> **Status:** Core feature set complete. Phase 9 (TTS Engine Overhaul) delivered. Remaining: onboarding refinement (OBD-02/03), optional enhancements

---

## Table of Contents

- [CopySpeak Implementation Roadmap](#copyspeak-implementation-roadmap)
  - [Table of Contents](#table-of-contents)
  - [Overview](#overview)
  - [Phase Status Legend](#phase-status-legend)
  - [Phase 1: Project Scaffold \& Core Architecture ✅](#phase-1-project-scaffold--core-architecture-)
    - [Deliverables](#deliverables)
  - [Phase 2: TTS Backend Abstraction ✅](#phase-2-tts-backend-abstraction-)
    - [Deliverables](#deliverables-1)
  - [Phase 3: Configuration System ✅](#phase-3-configuration-system-)
    - [Deliverables](#deliverables-2)
  - [Phase 4: Audio Playback ✅](#phase-4-audio-playback-)
    - [Deliverables](#deliverables-3)
  - [Phase 5: Clipboard Watching ✅](#phase-5-clipboard-watching-)
    - [Deliverables](#deliverables-4)
    - [Technical Notes](#technical-notes)
  - [Phase 6: HUD Overlay ✅](#phase-6-hud-overlay-)
    - [Deliverables](#deliverables-5)
    - [Position Options](#position-options)
    - [Integration Notes](#integration-notes)
  - [Phase 7: Main Window UI ✅](#phase-7-main-window-ui-)
    - [Deliverables](#deliverables-6)
      - [Status View](#status-view)
      - [Settings View](#settings-view)
  - [Phase 8: System Tray Integration ✅](#phase-8-system-tray-integration-)
    - [Deliverables](#deliverables-7)
  - [Phase 9: Global Hotkey 🚧](#phase-9-global-hotkey-)
    - [Deliverables](#deliverables-8)
  - [Phase 10: Amplitude Envelope Extraction ✅](#phase-10-amplitude-envelope-extraction-)
    - [Deliverables](#deliverables-9)
  - [Phase 11: Extended Features 🚧](#phase-11-extended-features-)
    - [Completed](#completed)
    - [Backlog (Priority 1)](#backlog-priority-1)
    - [Backlog (Priority 2)](#backlog-priority-2)
    - [Backlog (Priority 3)](#backlog-priority-3)
  - [Phase 12: UI Redesign - Brutalist Aesthetic ✅](#phase-12-ui-redesign---brutalist-aesthetic-)
    - [Deliverables](#deliverables-10)
      - [Design System](#design-system)
      - [Components Updated](#components-updated)
      - [Dark Mode System](#dark-mode-system)
      - [Color System](#color-system)
      - [Theme-Aware Fixes](#theme-aware-fixes)
      - [Documentation](#documentation)
    - [Technical Implementation](#technical-implementation)
    - [Files Created](#files-created)
    - [Testing Checklist](#testing-checklist)
    - [Design Decisions](#design-decisions)
  - [Implementation Priority](#implementation-priority)
  - [Milestones](#milestones)
  - [Feature Deferral Rationale (2026-02-24)](#feature-deferral-rationale-2026-02-24)
    - [Deferred Features](#deferred-features)
    - [Why Defer?](#why-defer)
    - [Impact on v0.1](#impact-on-v01)
    - [Path to v0.2+](#path-to-v02)

---

## Overview

This document tracks the implementation progress of CopySpeak, organized by development phases. Phases 1-8, 10, 12 are complete. Phases 9, 11 have deferred features.

**Recent Change (2026-03-06):** Phase 6 (HUD Overlay) completed. Basic HUD with waveform visualization, preset positioning, and theme customization is now implemented. Monitor selection removed as overkill; HUD now defaults to primary monitor only.

**Previous Change (2026-02-24):** Updated roadmap based on implemented_features.md. Ten advanced features were deferred from v0.1 and are now on the `features-extras` branch. This includes Phase 9 (Global Hotkey) and additional Phase 11 features. Core clipboard-to-speech functionality is complete.

---

## Phase Status Legend

| Status         | Description                            |
| -------------- | -------------------------------------- |
| ✅ Completed   | Feature fully implemented and tested   |
| 🚧 In Progress | Active development, partially complete |
| 📋 Pending     | Not yet started                        |

---

---

---

## Phase 1: Project Scaffold & Core Architecture ✅

**Status:** Completed

### Deliverables

- [x] Tauri v2 project setup with multi-window configuration
- [x] Rust module structure:
  - `main.rs` - App setup, tray icon
  - `clipboard.rs` - Clipboard monitoring
  - `config.rs` - Configuration persistence
  - `commands.rs` - IPC handlers
  - `audio.rs` - Audio playback
  - `hud.rs` - HUD management
  - `tts/` - TTS backends
- [x] Vite multi-page build (`index.html`, `hud.html`)
- [x] TypeScript configuration
- [x] Tailwind CSS v4.2 integration
- [x] shadcn-svelte button component

---

## Phase 2: TTS Backend Abstraction ✅

**Status:** Completed

### Deliverables

- [x] `TtsBackend` trait definition
  - `synthesize(text, voice, _speed) -> WAV bytes`
  - `health_check() -> bool`
- [x] CLI backend implementation
  - Templated command/args with placeholders
  - Placeholder tokens: `{text}`, `{output}`, `{voice}`, `{data_dir}`, `{raw_text}`
- [x] kokoro-tts preset configuration
- [x] Browser frontend playback rate control for speed and pitch adjustments

---

## Phase 3: Configuration System ✅

**Status:** Completed

### Deliverables

- [x] `AppConfig` structure with nested configs:
  - `trigger` - Double-copy window, global hotkey
  - `tts` - Engine preset, command, args, voice, speed
  - `playback` - Re-trigger mode, output device
  - `hud` - Enable, position, dimensions, opacity
  - `general` - Startup behavior, notifications
- [x] JSON persistence to `%APPDATA%/CopySpeak/config.json`
- [x] Load/save functions with error handling

---

## Phase 4: Audio Playback ✅

**Status:** Completed

### Deliverables

- [x] `AudioPlayer` struct with rodio integration
- [x] Re-trigger modes:
  - `Interrupt` - Stop current, play new
  - `Queue` - Append to current playback
- [x] WAV decoding and playback
- [x] Audio output device selection (via rodio device enumeration)

---

## Phase 5: Clipboard Watching ✅

**Status:** Completed

### Deliverables

- [x] Double-copy state machine
- [x] Clipboard listener thread structure
- [x] Win32 `AddClipboardFormatListener` integration
- [x] `read_clipboard_text()` Win32 implementation

### Technical Notes

The state machine logic:

```
IDLE → ARMED (on clipboard change) → SPEAK (same text within window)
                                   → IDLE (different text or timeout)
```

Win32 integration:

1. Creating a message-only window for clipboard messages
2. Registering with `AddClipboardFormatListener`
3. Handling `WM_CLIPBOARDUPDATE` messages
4. Reading text via `OpenClipboard` / `GetClipboardData` / `CloseClipboard`

---

## Phase 6: HUD Overlay ✅

**Status:** Completed (2026-03-06) - Basic HUD with preset positioning

### Deliverables

- [x] HUD window configuration in Tauri
  - Transparent, borderless, always-on-top, skip-taskbar
- [x] Show/hide functions with position calculation
- [x] `hud.html` entry point
- [x] Tauri events: `hud:start`, `hud:stop`
- [x] Waveform visualization component (canvas-based)
- [x] Amplitude envelope rendering with smooth animations
- [x] Preset positioning (6 corner positions)
- [x] Theme customization (dark/light/custom presets)
- [x] HUD settings in main settings page

### Position Options

```
top-left     top-center     top-right
bottom-left  bottom-center  bottom-right
```

### Integration Notes

- HUD enabled by default in configuration
- Shows on primary monitor only (multi-monitor support deferred)
- Position updates in real-time when settings change
- Supports custom x/y coordinates via drag positioning

---

## Phase 7: Main Window UI ✅

**Status:** Completed

### Deliverables

#### Status View

- [x] Listening state indicator (green/red dot)
- [x] Current voice/engine display
- [x] Last spoken text preview
- [x] Pause/Resume button
- [x] Speak Now button

#### Settings View

- [x] Trigger configuration (double-copy window, global hotkey)
- [x] TTS engine configuration (preset, command, args, voice, speed)
- [x] Playback configuration (re-trigger mode, output device)
- [x] HUD configuration (enable, position, opacity)
- [x] General settings (start with Windows, start minimized, notifications)
- [x] View toggle between Status/Settings

---

## Phase 8: System Tray Integration ✅

**Status:** Completed

### Deliverables

- [x] Tray icon with context menu
- [x] Menu items: Toggle Listening, Speak Now, Settings, Quit
- [x] Left-click shows/hides main window
- [x] Settings and Quit handlers working
- [x] Toggle Listening handler with clipboard/TTS integration
- [x] Speak Now handler with TTS integration
- [x] Dynamic menu label updates (Pause ↔ Resume)

---

## Phase 9: Global Hotkey 🚧

**Status:** Deferred to features-extras branch

### Deliverables

- [ ] Global shortcut plugin initialization
- [ ] Hotkey registration from config (default: `Ctrl+Shift+S`)
- [ ] Handler to trigger `speak_now` command
- [ ] Hotkey parsing and validation

---

## Phase 10: Amplitude Envelope Extraction ✅

**Status:** Completed

### Deliverables

- [x] Full WAV parsing implementation
  - Extract sample rate
  - Extract channel count
  - Extract bits per sample
- [x] RMS calculation per chunk
- [x] Normalization to 0.0-1.0 range
- [x] Duration extraction for synchronization

---

## Phase 11: Extended Features 🚧

**Status:** In Progress

### Completed

- [x] **Audio Save Mode** — Save TTS output to file with `{date}`, `{time}`, `{hash}` filename patterns
- [x] **Audio Format Conversion** — Export to MP3, OGG, FLAC with ffmpeg
- [x] **Audio Output Device Selection** — Enumerate and select output device
- [x] **Audio Skip Forward/Backward** — 5-second seek hotkeys
- [x] **Comprehensive Error Logging** — Log rotation (10MB max, 5 files)
- [x] **Configuration Validation** — Schema validation for config.json
- [x] **Debug Mode** — Verbose logging toggle in settings
- [x] **HTTP TTS Backend** — REST API backend for cloud TTS
- [x] **Max Text Length Limit** — Configurable cap (default 50,000)
- [x] **Pause/Resume Playback** — Pause/resume controls for TTS audio
- [x] **Playback Speed Hotkeys** — Ctrl+Shift+Up/Down adjustment
- [x] **Playback Volume Control** — Volume slider for TTS output
- [x] **Save Last Audio** — Quick-save button for last played audio
- [x] **Speech History Log** — Persistent log in history.json (1000 entries)
- [x] **SSML Support** — Speech Synthesis Markup Language input
- [x] **Streaming TTS** — Real-time streaming synthesis
- [x] **Tray Listening Toggle** — Toggle clipboard monitoring via tray
- [x] **Tray Speak Now** — Trigger TTS via tray menu
- [x] **WAV Amplitude Parser** — RMS calculation for visualization
- [x] **Win32 Clipboard Listener** — AddClipboardFormatListener API
- [x] **Windows Auto-Start** — Registry-based startup
- [x] **Clipboard Text Preview** — Real-time preview in status view
- [x] **Main Window Settings** — Comprehensive settings interface
- [x] **Main Window Status View** — Status dashboard
- [x] **Minimize to Tray** — Close-to-tray behavior

### Backlog (Priority 1)

- [ ] **Application Update Checker** — GitHub releases API check
- [ ] **Config Import/Export** — JSON preset sharing
- [ ] **Custom Pronunciation Dictionary** — Word/phrase substitutions
- [ ] **Text Preprocessing** — Strip URLs, formatting, etc.
- [ ] **Voice Preset Manager** — Save/load voice configurations

### Backlog (Priority 2)

- [ ] **Audio Device Selector UI** — Refinement for device selection
- [ ] **Custom Hotkey Editor** — UI for rebinding shortcuts
- [ ] **Keyboard Shortcuts Help** — Help overlay showing shortcuts
- [ ] **View Toggle Mechanism** — Keyboard shortcut for toggle

### Backlog (Priority 3)

- [ ] **Application Whitelist** — Per-app clipboard monitoring
- [ ] **Batch TTS Processor** — Process multiple texts sequentially
- [ ] **History Viewer UI** — Browse speech history log
- [ ] **Language Detection** — Auto-detect text language
- [ ] **Multi-Monitor Support** — HUD across monitors
- [ ] **Preset Import/Export** — Share presets as JSON
- [ ] **Pronunciation Dictionary** — Custom pronunciations
- [ ] **Skip Forward/Backward** — Audio navigation
- [ ] **Statistics Dashboard** — Usage analytics
- [ ] **TTS Engine Installer** — Guided TTS setup
- [ ] **TTS Health Check UI** — Engine availability
- [ ] **TTS Queue Viewer** — Visual queue
- [ ] **Usage Statistics** — Track metrics

---

## Phase 12: UI Redesign - Brutalist Aesthetic ✅

**Status:** Completed

### Deliverables

#### Design System

- [x] Hard edges (0px border radius) throughout application
- [x] Muted impressionist color palette using OKLCH
- [x] Cool slate and blue-gray tones
- [x] Complete dark mode system with manual toggle

#### Components Updated

- [x] Button components - hard edges, variant preservation
- [x] Switch toggle - hard edges, maintained functionality
- [x] Tabs system - hard edges for all tab elements
- [x] Input fields - hard edges, accessibility maintained
- [x] Select dropdowns - hard edges, user experience preserved
- [x] All container elements - consistent hard edges

#### Dark Mode System

- [x] mode-watcher integration
- [x] ThemeToggle component with Sun/Moon icons
- [x] Manual toggle functionality
- [x] System preference detection
- [x] Smooth icon transitions
- [x] Accessibility labels and roles

#### Color System

- [x] OKLCH color space implementation
- [x] Light mode slate tones
- [x] Dark mode slate tones
- [x] Theme-aware color classes
- [x] High contrast ratios (9.5:1+)
- [x] WCAG 2.1 AAA compliance

#### Theme-Aware Fixes

- [x] Removed hardcoded dark mode colors
- [x] Replaced green/yellow with emerald/amber
- [x] All components use CSS variables
- [x] Consistent theme support across app

#### Documentation

- [x] Complete brutalist design spec
- [x] Color palette documentation
- [x] Implementation guidelines
- [x] Accessibility compliance reports

### Technical Implementation

**Dependencies**

```bash
bun install mode-watcher
```

**Files Modified**

- `src/routes/+layout.svelte` - Added ModeWatcher
- `src/routes/layout.css` - Complete color system overhaul
- `src/lib/components/ThemeToggle.svelte` - New component
- `src/lib/components/StatusDashboard.svelte` - Theme toggle, color fixes
- UI Components - All updated to hard edges

**Design Principles**

- **KISS**: Minimal custom CSS, mostly Tailwind
- **DRY**: Single source of truth with CSS variables
- **Modern Brutalism**: Hard edges, muted tones, strong hierarchy
- **Impressionist**: Muted colors, soft yet distinct aesthetic

**Accessibility**

- WCAG 2.1 AA compliance (5.8:1+)
- Focus states with clear ring indicators
- Full keyboard navigation
- Screen reader support
- High contrast ratios

**Code Quality**

- TypeScript: 0 errors, 0 warnings
- Type checking: All pass
- Build: Successful
- Dev server: Starts successfully

### Files Created

**Documentation**

- `docs_internal/brutalist_design.md` - Complete design specification

### Testing Checklist

- [x] Light mode display and functionality
- [x] Dark mode display and functionality
- [x] Manual theme toggle works
- [x] System preference detection
- [x] All UI components render correctly
- [x] All components have hard edges
- [x] Theme-aware colors applied correctly
- [x] Focus states visible and functional
- [x] Keyboard navigation works
- [x] Responsive layout maintained
- [x] Type checking passes
- [x] Build process succeeds
- [x] No console errors
- [x] No console warnings

### Design Decisions

**Why OKLCH?**

- Perceptually uniform colors
- Accurate contrast ratios
- Modern industry standard
- Better color perception

**Why Hard Edges?**

- Pure brutalist aesthetic
- Strong visual hierarchy
- Clear boundaries between elements
- Contemporary design trend

**Why Manual+System Mode?**

- User control + automation
- Best of both worlds
- Shadcn-svelte best practice
- Modern UX pattern

**Why Slate Tones?**

- Professional, elegant
- Reduced eye strain
- Timeless aesthetic
- Suitable for extended use

---

## Implementation Priority

**v0.1 Release** (Current) — Core MVP complete with 5 features deferred:

1. ✅ Core Architecture (Phases 1-5)
2. ✅ TTS Backend (Phase 2)
3. ✅ Configuration (Phase 3)
4. ✅ Audio Playback (Phase 4)
5. ✅ Clipboard Watching (Phase 5)
6. ✅ System Tray (Phase 8)
7. ✅ Main Window UI (Phase 7)
8. ✅ UI Redesign — Brutalist (Phase 12)
9. ✅ Amplitude Envelope Extraction (Phase 10)
10. ✅ HUD Overlay — Basic (Phase 6)
11. ⏸️ **Deferred to v0.2+** (Phase 9, 11 Extended Features)
    - Global Hotkey (Phase 9)
    - Multi-Monitor Support for HUD
    - Language Detection
    - Content Filtering
    - Application Filter
    - Batch Processing

**Deferred features location:**

```bash
git checkout features-extras
```

---

## Milestones

| Milestone                     | Target | Status |
| ----------------------------- | ------ | ------ |
| Backend Core Complete         | -      | ✅     |
| End-to-End Speech Flow        | -      | ✅     |
| v0.1 Release (Stable Core)    | Apr24  | ✅     |
| v0.1.x Patch Releases         | TBD    | 📋     |
| v0.2 (with Deferred Features) | TBD    | 📋     |
| v1.0 Production Ready         | TBD    | 📋     |

---

## Feature Deferral Rationale (2026-02-24)

Ten features were deferred from v0.1 to create a focused, stable release:

### Deferred Features

1. **Global Hotkey** — System-wide keyboard shortcuts (Phase 9)
2. **HUD Waveform Visualization** — Canvas-based amplitude visualization
3. **HUD Drag Positioning** — Drag-to-position mode
4. **HUD Theme Customization** — Custom colors and styles
5. **HUD Multi-Monitor** — Multi-monitor support
6. **Language Detection** — Automatic language detection + voice selection
7. **Content Filtering** — Regex-based rules to skip sensitive content
8. **Application Filter** — Per-app whitelist/blacklist
9. **Keyboard Shortcuts Help** — Help overlay showing hotkeys
10. **Batch Processing** — Sequential text processing UI

### Why Defer?

- ✅ **Reduces v0.1 scope** — Focuses on stable core functionality
- ✅ **Fewer dependencies** — Removed 4 unused crates (whatlang, tauri-plugin-global-shortcut, regex, lazy_static)
- ✅ **Cleaner architecture** — Easier to test, debug, and maintain
- ✅ **Faster development** — No need to delay v0.1 for non-critical features
- ✅ **Preserved work** — All code is complete and on `features-extras`

### Impact on v0.1

- Core clipboard → speech flow works perfectly
- Audio playback, settings, history all functional
- Clean, maintainable codebase
- Faster iteration and bug fixes

### Path to v0.2+

When ready, the `features-extras` branch can be:

1. Cherry-picked feature-by-feature
2. Merged wholesale with conflict resolution
3. Re-implemented with fresh architecture
4. Made conditional with feature flags

All options are available with the deferred code preserved.
