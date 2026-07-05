# HUD Overlay System

The HUD (Heads-Up Display) overlay is a transparent, always-on-top window that provides real-time visual feedback during TTS playback and clipboard operations in CopySpeak.

## Overview

The HUD displays:

- **Waveform visualization** during audio playback - animated bars representing audio amplitude
- **"Clipboard Copied" notification** when double-copy is detected - pill-shaped indicator with progress fill
- **Synthesis progress** during TTS generation - animated progress indicator
- **Scrolling text marquee** of spoken content - scrolls horizontally when text exceeds container width

## Component Architecture

### Frontend Components

| Component                                                           | Path                                    | Purpose                                              |
| ------------------------------------------------------------------- | --------------------------------------- | ---------------------------------------------------- |
| [`hud-overlay.svelte`](../../src/lib/components/hud-overlay.svelte) | `src/lib/components/hud-overlay.svelte` | Main container, timer management, event coordination |
| [`hud-store.svelte.ts`](../../src/lib/stores/hud-store.svelte.ts)   | `src/lib/stores/hud-store.svelte.ts`    | Reactive state management using Svelte 5 runes       |
| [`use-hud-events.ts`](../../src/lib/composables/use-hud-events.ts)  | `src/lib/composables/use-hud-events.ts` | Composable for Tauri event handling                  |

### Child Components

| Component                                                                                     | Path                                                   | Purpose                                          |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------ |
| [`clipboard-notification.svelte`](../../src/lib/components/hud/clipboard-notification.svelte) | `src/lib/components/hud/clipboard-notification.svelte` | "Clipboard Copied" pill notification             |
| [`hud-playback-content.svelte`](../../src/lib/components/hud/hud-playback-content.svelte)     | `src/lib/components/hud/hud-playback-content.svelte`   | Waveform + marquee text display                  |
| [`hud-synthesis-progress.svelte`](../../src/lib/components/hud/hud-synthesis-progress.svelte) | `src/lib/components/hud/hud-synthesis-progress.svelte` | Processing progress indicator                    |
| [`hud-status.svelte`](../../src/lib/components/hud/hud-status.svelte)                         | `src/lib/components/hud/hud-status.svelte`             | Status indicator (not currently used in overlay) |

### Backend (Rust)

| File                                                               | Purpose                                            |
| ------------------------------------------------------------------ | -------------------------------------------------- |
| [`src-tauri/src/hud.rs`](../../src-tauri/src/hud.rs)               | HUD window management, event emission, positioning |
| [`src-tauri/src/config/hud.rs`](../../src-tauri/src/config/hud.rs) | HUD configuration types and validation             |

## Event Flow

The HUD operates on a push-based event system where the Rust backend emits events that the frontend listens to and reacts to:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Rust Backend  │────>│  Tauri Events   │────>│  Svelte Store   │
│   (hud.rs)      │     │                 │     │  (hud-store)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                               ┌─────────────────┐
                                               │  HUD Components │
                                               │  (Reactive UI)  │
                                               └─────────────────┘
```

### Event Types

| Event                                                                         | Direction           | Payload                                                     | Description                                   |
| ----------------------------------------------------------------------------- | ------------------- | ----------------------------------------------------------- | --------------------------------------------- |
| [`hud:start`](../../src-tauri/src/hud.rs:220)                                 | Rust → Frontend     | [`HudStartPayload`](../../src/lib/types/hud.ts:7)           | HUD should become visible with initial data   |
| [`hud:stop`](../../src-tauri/src/hud.rs:423)                                  | Rust → Frontend     | None                                                        | Hide HUD                                      |
| [`hud:synthesizing`](../../src-tauri/src/hud.rs:254)                          | Rust → Frontend     | [`HudSynthesizingPayload`](../../src/lib/types/hud.ts:14)   | TTS generation started                        |
| [`hud:playback_start`](../../src-tauri/src/hud.rs:294)                        | Rust → Frontend     | [`HudPlaybackStartPayload`](../../src/lib/types/hud.ts:21)  | Audio playback started                        |
| [`hud:amplitude`](../../src-tauri/src/commands/playback.rs:100)               | Rust → Frontend     | [`AmplitudePayload`](../../src/lib/types/hud.ts:50)         | Real-time amplitude data for waveform         |
| [`hud:synthesis-progress`](../../src-tauri/src/commands/tts/synthesis.rs:547) | Rust → Frontend     | [`SynthesisProgressPayload`](../../src/lib/types/hud.ts:28) | Progress updates during synthesis             |
| [`hud:clipboard-copied`](../../src-tauri/src/hud.rs:395)                      | Rust → Frontend     | [`ClipboardCopiedPayload`](../../src/lib/types/hud.ts:46)   | Double-copy detected                          |
| `hud:audio-duration`                                                          | Frontend → Frontend | `{ duration_ms: number }`                                   | Accurate audio duration from Web Audio decode |
| `pagination:started`                                                          | Rust → Frontend     | [`PaginationPayload`](../../src/lib/types/hud.ts:40)        | Text pagination started                       |
| `pagination:fragment-started`                                                 | Rust → Frontend     | [`PaginationPayload`](../../src/lib/types/hud.ts:40)        | New fragment being processed                  |
| `pagination:fragment-ready`                                                   | Rust → Frontend     | [`PaginationPayload`](../../src/lib/types/hud.ts:40)        | Fragment ready for playback                   |
| `playback-toggle-pause`                                                       | Rust → Frontend     | None                                                        | Pause/resume toggle                           |

## Cross-Window Communication

The HUD window (`/hud`) and main window (`/`) run in separate JavaScript contexts—each has its own `hudStore` instance. This means state updates in one window don't automatically propagate to the other.

### The Problem

1. Main window's `playbackStore.setupListeners()` decodes audio via Web Audio API
2. `AudioBuffer.duration` gives accurate playback duration (critical for ElevenLabs MP3 where server estimates are ±30% inaccurate)
3. `handleAudioReady()` calls `hudStore.setAccurateDurationMs()`
4. **BUT** this only updates the main window's `hudStore`, not the HUD window's `hudStore`
5. HUD window's `accurateDurationMs` stays `null`, `isPlaybackReady` stays `false`
6. Progress bar shows 0%, marquee stays centered without animation

### The Solution: Cross-Window Events

The `hud:audio-duration` event bridges the window gap:

```typescript
// Main window (playback-store.svelte.ts)
async handleAudioReady(audioData: ArrayBuffer) {
  const audioBuffer = await audioContext.decodeAudioData(audioData);
  const accurateDurationMs = audioBuffer.duration * 1000;
  emit('hud:audio-duration', { duration_ms: accurateDurationMs });
}

// HUD window (use-hud-events.ts)
listen('hud:audio-duration', (event) => {
  hudStore.setAccurateDurationMs(event.payload.duration_ms);
});
```

### Timing Formula

Effective playback duration accounts for pitch and speed adjustments:

```typescript
// Pitch is applied via OfflineAudioContext rendering (changes audio length)
// Speed is applied via HTMLAudioElement.playbackRate (changes playback speed)
adjustedDurationMs = accurateDurationMs / (pitch * speed);
```

### Readiness State

The HUD uses `isPlaybackReady` to prevent showing wrong progress:

```typescript
isPlaybackReady = accurateDurationMs !== null && accurateDurationMs > 0;
playbackProgressPercent = isPlaybackReady ? (playbackElapsedMs / adjustedDurationMs) * 100 : 0;
```

This ensures:

- Progress bar stays at 0% until accurate duration is known
- Marquee text centers (no animation) until `isPlaybackReady`
- Animation timing matches actual audio clip duration

## State Management

The [`hudStore`](../../src/lib/stores/hud-store.svelte.ts) uses Svelte 5 runes (`$state`, `$derived`) for reactive state management.

### Core State Properties

```typescript
// Playback state
barValues: number[]          // Amplitude values for waveform visualization
isVisible: boolean          // Whether HUD is shown
isSynthesizing: boolean      // TTS generation in progress
isPaused: boolean           // Playback paused
spokenText: string | null   // Text being spoken
provider: string | null     // TTS provider name
voice: string | null        // Selected voice name
audioDurationMs: number     // Duration from envelope (approximate)
elapsedMs: number           // Elapsed playback time
accurateDurationMs: number | null  // Accurate duration from Web Audio decode
playbackElapsedMs: number   // Elapsed time during playback
pitch: number              // Pitch multiplier (default 1.0)
speed: number              // Speed multiplier (default 1.0)

// Pagination state
currentFragment: number | null
totalFragments: number | null
isPaginated: boolean

// Progress state
totalChars: number
processedChars: number
progressConfidence: number
estimatedDurationMs: number | null

// Clipboard notification state
isClipboardCopied: boolean
clipboardDurationMs: number
```

### Derived Values

```typescript
// Combined provider/voice label for display
providerVoiceLabel: string | null; // "ElevenLabs · Rachel"

// Status text based on current state
statusLabel: string; // "Processing...", "Paused", or "Playing"

// Progress calculation with fallbacks
progressPercent: number; // Character-based or time-based progress

// Animation state
hasEstimate: boolean; // Whether we have progress data
dotPulsing: boolean; // Whether status dot should pulse

// Playback timing (requires accurateDurationMs)
isPlaybackReady: boolean; // True when accurate duration available
adjustedDurationMs: number; // accurateDurationMs / (pitch * speed)
playbackProgressPercent: number; // playbackElapsedMs / adjustedDurationMs * 100
```

### Handler Methods

The store provides compound handler methods that update multiple state values:

```typescript
handleStart(payload); // Initialize on hud:start
handleSynthesizing(payload); // Set synthesizing state
handlePlaybackStart(payload, durationMs); // Set playback state
handleStop(); // Reset all state
handleSynthesisProgress(payload); // Update progress
handlePagination(payload, fragmentReady); // Update pagination
handleClipboardCopied(payload); // Show clipboard notification
handleAmplitude(payload); // Update waveform bars
togglePause(); // Toggle pause state
clearClipboardCopied(); // Clear notification
```

## Timer System

The [`hud-overlay.svelte`](../../src/lib/components/hud-overlay.svelte) component manages two timers:

### 1. Elapsed Timer

Updates `elapsedMs` every 100ms during synthesis to track progress:

```typescript
function startElapsedTimer() {
  clearTimer(elapsedTimerState);
  elapsedTimerState = createTimer((elapsed) => {
    hudStore.setElapsedMs(elapsed);
  }, 100);
}

// Automatic management via $effect
$effect(() => {
  if (hudStore.isSynthesizing && elapsedTimerState.timer === null) {
    startElapsedTimer();
  } else if (!hudStore.isSynthesizing && elapsedTimerState.timer !== null) {
    stopElapsedTimer();
  }
});
```

### 2. Clipboard Dismiss Timer

Auto-hides the clipboard notification after `trigger_window_ms + 200ms`:

```typescript
function handleClipboardCopied(triggerWindowMs: number) {
  if (hudStore.isVisible || hudStore.isSynthesizing) return;

  clearClipboardCopied();
  hudStore.setClipboardDurationMs(triggerWindowMs);
  hudStore.setIsClipboardCopied(true);

  clipboardDismissTimerState = createTimeout(() => {
    hudStore.clearClipboardCopied();
    clipboardDismissTimerState = { timer: null };
  }, triggerWindowMs + 200);
}
```

## UI States

The HUD displays different content based on state:

### 1. Clipboard Notification Only

When double-copy detected but not speaking:

```svelte
{#if hudStore.isClipboardCopied && !hudStore.isVisible}
  <ClipboardNotification durationMs={hudStore.clipboardDurationMs} />
{/if}
```

Shows a pill-shaped notification with:

- "Clipboard Copied" text
- Progress fill animation that fills over `durationMs`
- Scale-in animation (0.4s cubic-bezier)

### 2. Synthesis Progress

During TTS generation:

```svelte
{#if hudStore.isSynthesizing}
  <HudSynthesisProgress
    estimatedDurationMs={hudStore.estimatedDurationMs}
    elapsedMs={hudStore.elapsedMs}
  />
{/if}
```

Shows:

- Pill-shaped progress indicator
- Progress fill based on character count or time estimate
- Scale-in animation

### 3. Playback Content

During audio playback:

```svelte
{:else}
  <HudPlaybackContent
    barValues={hudStore.barValues}
    spokenText={hudStore.spokenText}
    durationMs={hudStore.audioDurationMs}
    speed={playbackStore.speed}
  />
{/else}
```

Shows:

- **Waveform** - Animated bars showing amplitude (10+ bars with attack/decay smoothing)
- **Marquee text** - Scrolling text that animates horizontally when text is wider than container
- **Progress fill** - Background gradient that fills left-to-right based on playback position

### 4. Hidden

When nothing active:

```svelte
<div class="hud-overlay" class:visible={hudStore.isVisible || hudStore.isClipboardCopied}>
```

The overlay has `opacity: 0` by default, transitioning to `opacity: 1` when visible.

## Dev Mode

When running outside Tauri (browser dev), the component shows mock data for development:

```typescript
if (isTauri) {
  await setupEventListeners();
} else {
  // Dev mode: setup mock data and animation
  animateDevBars();
  hudStore.setSpokenText("The quick brown fox...");
  hudStore.setProvider("ElevenLabs");
  hudStore.setVoice("Rachel");
  hudStore.setIsVisible(true);
  hudStore.setAudioDurationMs(8000);
}
```

The `animateDevBars()` function generates animated waveform data using sine waves:

```typescript
function animateDevBars() {
  const bars: number[] = [0, 0];
  for (let i = 0; i < 10; i++) {
    const t = i / 10;
    const wave1 = Math.sin(t * Math.PI * 3 + Date.now() / 200) * 0.3;
    const wave2 = Math.sin(t * Math.PI * 5 + Date.now() / 150) * 0.2;
    const wave3 = Math.sin(t * Math.PI * 7 + Date.now() / 100) * 0.1;
    const value = 0.4 + wave1 + wave2 + wave3;
    bars.push(Math.max(0.1, Math.min(1, value)));
  }
  hudStore.setBarValues(bars);
  devAnimationId = requestAnimationFrame(animateDevBars);
}
```

## Configuration

### HUD Settings in AppConfig

The HUD is configured via [`HudConfig`](../../src-tauri/src/config/hud.rs):

```rust
pub struct HudConfig {
    pub enabled: bool,              // Enable/disable HUD
    pub position: HudPosition,     // Screen position preset
    pub width: u32,                // Window width (default: 300)
    pub height: u32,               // Window height (default: 100)
    pub opacity: f32,              // Background opacity (default: 0.85)
}
```

### Position Presets

```rust
pub enum HudPresetPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,  // Default
    BottomRight,
}
```

### Default Configuration

```rust
hud: HudConfig {
    enabled: true,
    position: HudPosition::Preset(HudPresetPosition::BottomCenter),
    width: 300,
    height: 100,
    opacity: 0.85,
}
```

### Tauri Window Configuration

The HUD window is defined in [`tauri.conf.json`](../../src-tauri/tauri.conf.json:49):

```json
{
  "label": "hud",
  "title": "",
  "url": "/hud",
  "width": 300,
  "height": 140,
  "decorations": false,
  "transparent": true,
  "shadow": false,
  "backgroundColor": [0, 0, 0, 0],
  "alwaysOnTop": true,
  "resizable": false,
  "skipTaskbar": true,
  "visible": false,
  "focus": false
}
```

Key properties:

- **transparent**: Enables transparent background
- **alwaysOnTop**: Keeps HUD above other windows
- **decorations**: false - Removes title bar
- **skipTaskbar**: true - Hides from taskbar
- **focus**: false - Allows clicks to pass through to underlying windows

## Integration Points

### Backend → Frontend Flow

1. **TTS Synthesis Started**: [`show_hud_synthesizing()`](src-tauri/src/hud.rs:254) emits `hud:synthesizing`
2. **TTS Generation Complete**: [`show_hud()`](src-tauri/src/hud.rs:220) emits `hud:start` with amplitude envelope
3. **Playback Started**: [`show_hud_playback()`](src-tauri/src/hud.rs:294) emits `hud:playback_start`
4. **Amplitude Updates**: [`emit_amplitude()`](src-tauri/src/commands/playback.rs:100) emits `hud:amplitude` during playback
5. **Clipboard Copied**: [`show_hud_clipboard_copied()`](src-tauri/src/hud.rs:395) emits `hud:clipboard-copied`
6. **Hide HUD**: [`hide_hud()`](src-tauri/src/hud.rs:423) emits `hud:stop` and hides window

### Frontend → Backend Flow

The frontend does not directly communicate back to the backend via events. The HUD is purely reactive to backend events.

### HUD Lifecycle

```
Double-copy detected ─▶ Show window ─▶ Emit hud:clipboard-copied ─▶ Auto-hide after trigger_window_ms + 200ms
                           │
TTS started ───────────────┼──────▶ Emit hud:synthesizing ──▶ Show synthesis progress
                           │
TTS complete ──────────────┼──────▶ Emit hud:start ──▶ Show waveform + marquee
                           │
Playback starts ───────────┼──────▶ Emit hud:playback_start ──▶ Update playback content
                           │
Amplitude updates ─────────┼──────▶ Emit hud:amplitude ──▶ Update waveform bars
                           │
Playback ends ─────────────┴──────▶ Emit hud:stop ──▶ Hide window
```

## Type Definitions

### Frontend Types ([`src/lib/types/hud.ts`](../../src/lib/types/hud.ts))

```typescript
interface AmplitudeEnvelope {
  values: number[];
  duration_ms: number;
  sample_rate: number;
}

interface HudStartPayload {
  envelope: AmplitudeEnvelope;
  text: string | null;
  provider?: string | null;
  voice?: string | null;
}

interface HudSynthesizingPayload {
  text: string | null;
  provider?: string | null;
  voice?: string | null;
  duration_ms?: number;
}

interface HudPlaybackStartPayload {
  text: string | null;
  provider?: string | null;
  voice?: string | null;
  audio_duration_ms?: number;
}

interface SynthesisProgressPayload {
  estimated_total_ms: number | null;
  elapsed_ms: number;
  fragment_index: number;
  fragment_total: number;
  is_paginated: boolean;
  confidence: number;
  text_preview: string;
  total_chars: number;
  processed_chars: number;
}

interface PaginationPayload {
  current_index: number;
  total: number;
  is_paginated: boolean;
}

interface ClipboardCopiedPayload {
  trigger_window_ms: number;
}

interface AmplitudePayload {
  bars: number[];
}
```

## See Also

- [Event System](./event-system.md) - General event architecture
- [brutalist_design.md](./brutalist_design.md) - UI design system
- [Architecture](./architecture.md) - System architecture overview
