# Phase 8: HUD Enhancement - Context

**Gathered:** 2026-03-07 (updated 2026-03-07)
**Status:** Ready for planning (bug fix plans needed)
**Source:** PRD Express Path (initial) + user discussion (update)

<domain>
## Phase Boundary

This phase delivers a clean, playback-synchronized HUD overlay. The HUD becomes a purely functional status display that appears during active speech playback and disappears automatically when playback ends — no user interaction required to dismiss it. The waveform visualization is genuinely responsive to audio amplitude in real-time. All customization features, drag-and-drop positioning, and the close button are removed entirely.

</domain>

<decisions>
## Implementation Decisions

### Auto-Hide Behavior (LOCKED)
- HUD must automatically disappear when audio/video playback stops
- No manual close button — remove it entirely
- HUD shows only during active playback

### Background Initialization (LOCKED)
- HUD window must be cached and initialized in the background
- Must appear and disappear instantly with no perceptible delay
- No lazy initialization on show — pre-warm the window

### Waveform Visualization (REVISED — replaces Plan 04 approach)
- Must use Web Audio API `AnalyserNode` for true real-time audio analysis
- NOT pre-computed envelope oscillation (Plan 04's sin-wave approach is insufficient)
- Architecture: main window `PlaybackStore` taps `<audio>` element via `createMediaElementSource()` → `AnalyserNode` → reads FFT frequency data → emits bar amplitude values to HUD via Tauri events at ~30fps
- HUD `waveform.svelte` receives live bar values and renders them directly — no envelope needed
- Bar count: 16 bars (sparse, bold aesthetic)
- Bar data: FFT frequency bands mapped to 16 bars (covers bass/mid/treble spectrum)
- Higher amplitude = taller bars, live (not pre-computed)

### Synthesizing State UX (LOCKED)
- During synthesis (before audio is ready): spinner + "Preparing speech..." text only
- Waveform area shows nothing / hides during synthesizing state
- No placeholder animation during synthesizing

### HUD Hide Bug Fix (NEW — wiring gap from Plans 01-04)
- Root cause: `playbackStore` never emits `hud:stop` when audio finishes
- Fix: in `playbackStore.setAudioElement()`, the `onended` handler must emit `hud:stop` (or call `hide_hud` via Tauri invoke)
- Also: `handleStop()` must emit `hud:stop` so manual stop also hides HUD
- The CSS opacity transition (0.2s) and `hud:stop` handler in `hud-overlay.svelte` are already correct — only the trigger is missing

### Draggable Functionality (LOCKED — REMOVED)
- Remove ALL draggable HUD functionality entirely
- No drag-and-drop positioning by user
- No drag state, drag event handlers, or drag logic anywhere in codebase
- HUD supports preset configurations only for positioning

### Custom Styling (LOCKED — REMOVED)
- Remove ALL custom styling options from HUD
- Settings panel: enable toggle + position preset only (already implemented)

### No Close Button (LOCKED)
- Close button removed entirely from HUD

### Claude's Discretion
- Exact FFT bin grouping into 16 frequency bands (e.g., logarithmic spacing for perceptual balance)
- Bar animation smoothing between frames (e.g., lerp to avoid flickering)
- Tauri event payload format for amplitude data (array of 16 floats, 0.0–1.0)
- Exact emit rate (target ~30fps, throttle if needed)
- Whether to use `getByteFrequencyData` or `getFloatFrequencyData`
- Minimum bar height for visual clarity

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/lib/components/waveform.svelte`: Existing waveform renderer with canvas-based bar drawing. Needs input interface changed from `envelope: AmplitudeEnvelope` to live `barValues: number[]` (16 values, 0.0–1.0). Remove oscillation logic from Plan 04.
- `src/lib/stores/playback-store.svelte.ts`: Already has `AudioContext` instance (`_audioCtx`) and `HTMLAudioElement` reference (`_audioEl`). `createMediaElementSource()` + `AnalyserNode` can be added here. Already has `setupListeners()` pattern for Tauri events.
- `src/lib/components/hud-overlay.svelte`: Already listens to `hud:stop` event and sets `isVisible = false`. CSS opacity transition is already correct. Just needs the trigger to actually fire.
- `src-tauri/src/hud.rs`: `hide_hud()` exists and emits `hud:stop` globally + calls `hud_window.hide()`. Could be invoked from frontend via IPC, OR frontend can emit `hud:stop` directly (simpler — no Rust roundtrip needed).

### Bug: HUD Never Hides
- `playbackStore.setAudioElement()` sets `el.onended` → only sets `this.isPlaying = false`; never emits `hud:stop`
- `playbackStore.handleStop()` pauses audio; never emits `hud:stop`
- `hide_hud()` in Rust exists but is never called from the TTS flow after frontend audio ends
- Fix location: `playbackStore.svelte.ts` — add `emit("hud:stop", null)` in `onended` and `handleStop()`

### Bug: Waveform Not Responsive
- Plan 04 added sinusoidal oscillation on pre-computed envelope values — not actual real-time audio
- Pre-computed envelope: 40 values extracted from WAV before playback, fallback to `vec![0.5; 40]` if extraction fails
- The HUD waveform and the audio element are in different windows — amplitude data must cross via Tauri events

### Established Patterns
- Tauri global events: `app.emit()` in Rust / `listen()` in frontend (used for `hud:start`, `hud:stop`, `audio-ready`)
- Frontend→HUD cross-window data: currently only via Rust-emitted events; frontend can also emit directly from main window to HUD via `@tauri-apps/api/event`
- `PlaybackStore.setupListeners()`: established pattern for wiring Tauri event listeners in the store

### Integration Points
- `playbackStore.setAudioElement()` → where `AnalyserNode` should be wired up
- `playbackStore.setupListeners()` → where amplitude emit loop should start/stop
- `hud-overlay.svelte` → receives amplitude events, passes `barValues` prop to `waveform.svelte`
- `waveform.svelte` → replace `envelope`-based rendering with `barValues`-based rendering

</code_context>

<specifics>
## Specific Ideas

- 16 bars, sparse/bold: fewer wider bars like a classic equalizer, not a dense spectrum analyzer
- Bars should react to what's actually playing in real time — louder audio = taller bars immediately
- Synthesizing state: spinner only, no waveform placeholder
- HUD hides when audio ends naturally (onended event)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 08-hud-enhancement*
*Context updated: 2026-03-07 — added bug analysis and waveform overhaul decisions*
