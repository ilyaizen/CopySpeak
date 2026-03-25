# Phase 8: HUD Enhancement - Research (Gap Closure)

**Researched:** 2026-03-07
**Domain:** Web Audio API AnalyserNode + Tauri cross-window events + Svelte 5 store patterns
**Confidence:** HIGH — all findings based on direct codebase inspection

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- HUD must automatically disappear when audio/video playback stops
- No manual close button — remove it entirely
- HUD shows only during active playback
- HUD window must be cached and initialized in the background (pre-warm)
- Waveform MUST use Web Audio API `AnalyserNode` for true real-time audio analysis
- NOT pre-computed envelope oscillation (Plan 04's sin-wave approach is insufficient)
- Architecture: main window `PlaybackStore` taps `<audio>` element via `createMediaElementSource()` → `AnalyserNode` → reads FFT frequency data → emits bar amplitude values to HUD via Tauri events at ~30fps
- HUD `waveform.svelte` receives live bar values and renders them directly — no envelope needed
- Bar count: 16 bars (sparse, bold aesthetic)
- During synthesis: spinner + "Preparing speech..." text only; waveform area shows nothing
- Root cause of hide bug: `playbackStore` never emits `hud:stop` when audio finishes
- Fix location: `playbackStore.svelte.ts` — `onended` handler and `handleStop()` must emit `hud:stop`
- Remove ALL draggable HUD functionality (no drag state, no drag handlers, no drag logic)
- Remove ALL custom styling options from HUD (enable toggle + position preset only)
- No close button anywhere

### Claude's Discretion

- Exact FFT bin grouping into 16 frequency bands (e.g., logarithmic spacing for perceptual balance)
- Bar animation smoothing between frames (e.g., lerp to avoid flickering)
- Tauri event payload format for amplitude data (array of 16 floats, 0.0–1.0)
- Exact emit rate (target ~30fps, throttle if needed)
- Whether to use `getByteFrequencyData` or `getFloatFrequencyData`
- Minimum bar height for visual clarity

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

## Summary

Phase 8 gap closure adds two new plans to an already partially-executed phase. Plans 01, 02, and 03 are complete. Plan 04 (envelope oscillation) is being replaced entirely. The two new plans are: (A) a bug fix wiring `hud:stop` emission in `playbackStore`, and (B) a waveform overhaul replacing the envelope-based approach with a real Web Audio API `AnalyserNode` pipeline.

The codebase is in a well-understood state. All existing hook points are confirmed by direct code inspection. The `playbackStore` already has `_audioCtx` and `_audioEl` as private fields — both are exactly what `createMediaElementSource()` needs. The `hud-overlay.svelte` already listens for `hud:stop` and transitions visibility correctly; it just never receives the event from the frontend. The Tauri `emit()` API from `@tauri-apps/api/event` is the correct frontend-to-all-windows broadcast mechanism.

**Primary recommendation:** Two plans, executed sequentially. Plan 04 (bug fix) first — it is self-contained with no risk. Plan 05 (waveform overhaul) second — it modifies three files and replaces the envelope-based `Waveform` interface entirely.

---

## Existing Code State (Critical Context)

### What Plans 01–03 Completed

Plan 01 (`hud-overlay.svelte`): Drag, close button, and theme config removed. Dark theme hardcoded. Waveform receives `{envelope}` and `{isPlaying}` props — both still present.

Plan 02 (settings): HUD settings stripped to enable toggle + position preset. Types simplified.

Plan 03 (Rust): Dead commands removed. `HudConfig` has no theme field. `HudPosition::Custom` gone. `cargo check` passes.

**Current state of `hud-overlay.svelte` (post-Plan-01):** Listens for `hud:stop` → calls `handleStop()` which sets `isVisible = false`. The CSS opacity transition (0.2s) is correct. The listener exists. The problem is only that nobody emits `hud:stop` from the frontend when audio ends.

**Current state of `waveform.svelte` (post-Plan-04, which was already executed):** Uses oscillating envelope approach. Has `envelope: AmplitudeEnvelope | null` and `isPlaying: boolean` as props. The component has `requestAnimationFrame` loop, canvas-based rendering, `drawWaveform(progress, timestamp)` with oscillation math. This entire interface will be replaced.

### What Needs to Be Built

**New Plan 04 (bug fix):** Two `emit("hud:stop", null)` calls in `playbackStore.svelte.ts`.

**New Plan 05 (waveform overhaul):** Three-file change:
1. `playbackStore.svelte.ts` — add `AnalyserNode` wiring + 30fps emit loop
2. `waveform.svelte` — replace `envelope`/`isPlaying` props with `barValues: number[]`
3. `hud-overlay.svelte` — add `hud:amplitude` listener, pass `barValues` to `Waveform`

---

## Architecture Patterns

### Pattern 1: Tauri Frontend-to-All-Windows Event Emit

**What:** The frontend (main window) emits a global Tauri event that all windows receive, including the HUD window.

**API:** `@tauri-apps/api/event` — `emit(eventName, payload)` for global broadcast.

**Confirmed usage in codebase:** `hud-overlay.svelte` already uses `eventApi.listen()` from `@tauri-apps/api/event` to receive `hud:start`, `hud:stop`, and `hud:synthesizing`. These are currently only emitted from Rust (`app.emit()`). The frontend can emit using the same `emit()` function from the same package.

```typescript
// Source: direct inspection of hud-overlay.svelte onMount and Tauri docs pattern
const { emit, listen } = await import("@tauri-apps/api/event");
await emit("hud:stop", null);  // global — all windows receive it
```

**Important:** `playbackStore.svelte.ts` already imports `listen` from `@tauri-apps/api/event` in `setupListeners()`. Adding `emit` to the same dynamic import is trivial.

**Confirmed in `hide_hud()` (Rust, `src-tauri/src/hud.rs` line 212):**
```rust
let _ = app.emit("hud:stop", ());
let _ = hud_window.hide();
```
The frontend calling `emit("hud:stop", null)` is equivalent to the Rust side — the HUD's `unlistenStop` handler fires and calls `handleStop()`. The Rust-side `hud_window.hide()` is an OS-level window hide; the frontend cannot replicate this. So the frontend emit triggers the CSS opacity transition, and the Rust `hide_hud()` can be optionally invoked via IPC for the actual OS window hide. **Decision area:** Simplest fix is `emit("hud:stop")` from frontend + separately call `invoke("hide_hud")` if needed, OR just `emit` and accept the window stays visible but transparent. The CONTEXT.md says: "emit hud:stop (or call hide_hud via Tauri invoke)" — both options are valid.

### Pattern 2: Web Audio API AnalyserNode via createMediaElementSource

**What:** Tap an existing `<audio>` element via Web Audio API to get real-time FFT frequency data without re-encoding or re-routing audio playback.

**Confirmed prerequisite in `playbackStore.svelte.ts`:**
- `_audioCtx: AudioContext | null` — already created in `handleAudioReady()` at line 136
- `_audioEl: HTMLAudioElement | null` — set by `setAudioElement()`
- `_audioCtx` is created lazily on first audio ready. By the time `onended` fires, it exists.

**Critical constraint:** `createMediaElementSource()` can only be called ONCE per `HTMLAudioElement`. Calling it again throws `InvalidStateError`. The source node must be stored and reused across replays.

**Correct wiring location:** `setAudioElement()` — called once per `<audio>` element mount. The `AnalyserNode` can be created here (or lazily on first use). The `AudioContext` may not exist yet when `setAudioElement()` is first called (it's created in `handleAudioReady()`), so lazy init in `handleAudioReady()` is safer.

```typescript
// Correct pattern (lazy init in handleAudioReady, after AudioContext is guaranteed):
if (!this._audioCtx) {
  this._audioCtx = new AudioContext();
}
if (!this._analyser && this._audioEl) {
  const source = this._audioCtx.createMediaElementSource(this._audioEl);
  this._analyser = this._audioCtx.createAnalyser();
  this._analyser.fftSize = 256;  // 128 frequency bins
  source.connect(this._analyser);
  this._analyser.connect(this._audioCtx.destination);  // MUST connect to destination or audio is silent
}
```

**CRITICAL:** `source.connect(this._analyser)` AND `this._analyser.connect(this._audioCtx.destination)` — if destination is not connected, the audio plays silently. The current `playAudio()` uses `this._audioEl.play()` directly which works because the element's default audio routing still applies. After `createMediaElementSource()`, the element's audio is rerouted through the Web Audio graph — the destination connection is mandatory.

**FFT data reading:**

```typescript
// getByteFrequencyData: Uint8Array, values 0-255
const dataArray = new Uint8Array(this._analyser.frequencyBinCount); // 128 bins for fftSize=256
this._analyser.getByteFrequencyData(dataArray);

// Map 128 bins → 16 bars (logarithmic grouping for perceptual balance)
// Bass bins are perceptually more important; use log spacing
```

**Logarithmic bin grouping (Claude's discretion — recommended):**
```typescript
function buildBarValues(dataArray: Uint8Array, numBars: number): number[] {
  const binCount = dataArray.length;
  const bars: number[] = [];
  for (let i = 0; i < numBars; i++) {
    // Log scale: bin index grows exponentially across bars
    const startBin = Math.floor(Math.pow(binCount, i / numBars));
    const endBin = Math.floor(Math.pow(binCount, (i + 1) / numBars));
    let sum = 0;
    for (let b = startBin; b < endBin; b++) {
      sum += dataArray[b];
    }
    const count = endBin - startBin || 1;
    bars.push(sum / count / 255);  // normalize to 0.0–1.0
  }
  return bars;
}
```

### Pattern 3: 30fps Emit Loop Pattern

**What:** `requestAnimationFrame` loop that reads AnalyserNode data and emits Tauri events. Throttled to ~30fps (every ~33ms).

**Emit loop location:** `playbackStore.svelte.ts` — started when playback begins, stopped when playback ends.

```typescript
private _amplitudeLoopId: number | null = null;
private _lastEmitTime = 0;

private startAmplitudeLoop(emit: (name: string, payload: unknown) => Promise<void>) {
  const loop = async (timestamp: number) => {
    if (!this._analyser || !this.isPlaying) {
      this._amplitudeLoopId = null;
      return;
    }
    // Throttle to ~30fps
    if (timestamp - this._lastEmitTime >= 33) {
      const dataArray = new Uint8Array(this._analyser.frequencyBinCount);
      this._analyser.getByteFrequencyData(dataArray);
      const barValues = buildBarValues(dataArray, 16);
      await emit("hud:amplitude", { bars: barValues });
      this._lastEmitTime = timestamp;
    }
    this._amplitudeLoopId = requestAnimationFrame(loop);
  };
  this._amplitudeLoopId = requestAnimationFrame(loop);
}
```

**Note on emit cost:** Tauri's `emit()` is async IPC. At 30fps that is 30 IPC calls/second. This is acceptable for a 16-float payload. The alternative (postMessage) is not available cross-window in Tauri.

### Pattern 4: Waveform Component Interface Replacement

**Current interface (to be replaced):**
```typescript
interface Props {
  envelope: AmplitudeEnvelope | null;
  isPlaying?: boolean;
  barColor?: string;
  activeBarColor?: string;
  backgroundColor?: string;
  barGap?: number;
  minBarHeight?: number;
  barRadius?: number;
  animationSpeed?: number;
}
```

**New interface:**
```typescript
interface Props {
  barValues: number[];  // 16 floats, 0.0-1.0, live from AnalyserNode
  barColor?: string;
  activeBarColor?: string;
  backgroundColor?: string;
  barGap?: number;
  minBarHeight?: number;
  barRadius?: number;
}
```

**What changes in the component body:**
- Remove: `envelope`, `isPlaying`, `animationSpeed` props
- Remove: `animate()` function and `requestAnimationFrame` loop (driven externally now)
- Remove: `startAnimation()`, `stopAnimation()` functions
- Remove: `startTime`, `currentProgress`, `animationSpeed` state
- Remove: progress-based `activeBarIndex` calculation
- Keep: canvas, `ctx`, `containerWidth`, `containerHeight`, `resizeCanvas()`, `drawRoundedRect()`
- Keep: ResizeObserver, onMount, onDestroy (for canvas lifecycle)
- Simplify `drawWaveform()` to use `barValues` directly — no progress, no timestamp, no oscillation:

```typescript
function drawWaveform() {
  if (!ctx || !barValues.length) return;
  ctx.clearRect(0, 0, containerWidth, containerHeight);
  const numBars = barValues.length;
  const totalGapWidth = barGap * (numBars - 1);
  const barWidth = (containerWidth - totalGapWidth) / numBars;
  const maxBarHeight = containerHeight * 0.9;
  const minHeight = containerHeight * minBarHeight;

  for (let i = 0; i < numBars; i++) {
    const amplitude = barValues[i];
    const barHeight = Math.max(minHeight, amplitude * maxBarHeight);
    const x = i * (barWidth + barGap);
    const y = (containerHeight - barHeight) / 2;
    ctx.fillStyle = activeBarColor;  // all bars use active color when live
    drawRoundedRect(ctx, x, y, barWidth, barHeight, barRadius);
  }
}
```

- Add `$effect` on `barValues` to call `drawWaveform()` whenever values update.

**Synthesizing state:** `barValues` will be empty (`[]`) during synthesis. `drawWaveform()` returns early if `!barValues.length`. The waveform area renders nothing during synthesis — correct per locked decision.

### Pattern 5: hud-overlay.svelte Amplitude Event Listener

**What to add:**
```typescript
let barValues = $state<number[]>([]);
let unlistenAmplitude: (() => void) | null = null;

// In onMount, after existing listeners:
unlistenAmplitude = await eventApi.listen<{ bars: number[] }>(
  "hud:amplitude",
  (event) => {
    barValues = event.payload.bars;
  }
);

// In onDestroy:
if (unlistenAmplitude) unlistenAmplitude();
```

**Template change:**
```svelte
<!-- REMOVE: envelope and isPlaying props -->
<!-- ADD: barValues prop -->
{#if !isSynthesizing}
  <div class="hud-waveform-row">
    <Waveform
      {barValues}
      barColor="rgba(255, 255, 255, 0.3)"
      activeBarColor="rgba(96, 165, 250, 1)"
      barGap={3}
      barRadius={2}
      minBarHeight={0.15}
    />
  </div>
{/if}
```

**Note:** The synthesizing state guard (`{#if !isSynthesizing}`) is added here — or `barValues` is empty during synthesis and `drawWaveform()` skips rendering. Either approach satisfies the locked decision. Wrapping in `{#if !isSynthesizing}` is cleaner.

---

## Bug Fix Analysis (New Plan 04)

### Root Cause (Confirmed by Code Inspection)

`src/lib/stores/playback-store.svelte.ts` lines 41–44:
```typescript
el.onended = () => {
  this.isPlaying = false;
  this.isPaused = false;
  // MISSING: emit("hud:stop", null)
};
```

`handleStop()` lines 171–178:
```typescript
handleStop() {
  if (this._audioEl) {
    this._audioEl.pause();
    this._audioEl.currentTime = 0;
  }
  this.isPlaying = false;
  this.isPaused = false;
  // MISSING: emit("hud:stop", null)
}
```

### Fix Pattern

Both locations need the same call. The `emit` function must be imported dynamically (same pattern as `setupListeners()`). Two implementation options:

**Option A: Store `emit` reference after setup**
```typescript
private _emit: ((name: string, payload: unknown) => Promise<void>) | null = null;

// In setupListeners():
const { listen, emit } = await import("@tauri-apps/api/event");
this._emit = emit;
```
Then in `onended` and `handleStop()`:
```typescript
this._emit?.("hud:stop", null);
```

**Option B: Inline invoke in handleStop + setAudioElement (async methods)**

Since `setAudioElement()` and `handleStop()` are synchronous, they cannot `await import()`. Option A (cache `emit` reference after `setupListeners()` runs) is the correct pattern, consistent with how the store already works.

**Tauri-guard:** Both calls must be guarded — `emit` only exists after `setupListeners()` runs (Tauri only). The `_emit` being `null` before setup or in browser mode is correct fail-safe behavior.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Real-time FFT | Custom FFT algorithm | Web Audio `AnalyserNode` | Browser-native, GPU-optimized, zero latency |
| Cross-window data | WebSocket / shared memory | Tauri `emit()` | Already established in codebase, no extra infra |
| Bar smoothing | Complex interpolation | Simple lerp in `$effect` | `barValues[i] = barValues[i] * 0.7 + newValues[i] * 0.3` |
| Canvas animation loop | Store-managed rAF | Component-level `$effect` on barValues | Simpler — waveform just reacts to prop changes |

---

## Common Pitfalls

### Pitfall 1: createMediaElementSource Called Twice
**What goes wrong:** Calling `createMediaElementSource(this._audioEl)` twice on the same element throws `InvalidStateError: Failed to execute 'createMediaElementSource'`.
**Why it happens:** `handleReplay()` sets a new `src` but the same element — `setAudioElement()` is not called again. The source node persists across replays.
**How to avoid:** Guard with `if (!this._sourceNode)` — create once, reuse. Store as `private _sourceNode: MediaElementAudioSourceNode | null = null`.

### Pitfall 2: Audio Goes Silent After createMediaElementSource
**What goes wrong:** Audio plays silently — no sound from speakers.
**Why it happens:** `createMediaElementSource()` takes the element out of the default audio routing. Audio must flow through the Web Audio graph to the destination.
**How to avoid:** Always `source.connect(analyser)` AND `analyser.connect(audioCtx.destination)`.

### Pitfall 3: AnalyserNode Created Before AudioContext Exists
**What goes wrong:** `TypeError: Cannot read property 'createMediaElementSource' of null`.
**Why it happens:** `setAudioElement()` is called from `onMount` in `GlobalPlayer` before the first `audio-ready` event arrives (which is when `_audioCtx` is created).
**How to avoid:** Create the AnalyserNode lazily in `handleAudioReady()`, not in `setAudioElement()`. Both `_audioCtx` and `_audioEl` are guaranteed to exist at that point.

### Pitfall 4: Amplitude Loop Keeps Running After Stop
**What goes wrong:** `hud:amplitude` events continue emitting after HUD is hidden.
**Why it happens:** `requestAnimationFrame` loop checks `this.isPlaying` — if `handleStop()` sets `isPlaying = false` before the next frame, the loop stops naturally. But if the loop is async and `emit` awaits, timing can cause one extra frame.
**How to avoid:** Check `this.isPlaying` at the TOP of the loop before any work. Cancel `_amplitudeLoopId` explicitly in `handleStop()`.

### Pitfall 5: `emit` Called Before `setupListeners()` Completes
**What goes wrong:** `this._emit?.("hud:stop")` is a no-op — HUD doesn't hide.
**Why it happens:** If stop is triggered before setup completes (edge case: very fast stop during startup).
**How to avoid:** The `?.` operator handles null gracefully. This is acceptable — if setup hasn't completed, the HUD was never shown anyway.

### Pitfall 6: waveform.svelte Still Expects envelope Prop
**What goes wrong:** TypeScript error after changing hud-overlay.svelte to pass `barValues` but waveform.svelte still has `envelope: AmplitudeEnvelope | null` in Props.
**Why it happens:** Both files must be updated atomically.
**How to avoid:** Plan 05 modifies waveform.svelte AND hud-overlay.svelte in the same plan. Both files change together. Run `bun run check` to confirm.

---

## Code Examples

### Confirmed: emit API from @tauri-apps/api/event
```typescript
// Source: direct inspection of hud-overlay.svelte line 101-102 (uses listen from same package)
// and Tauri v2 docs pattern
const { listen, emit } = await import("@tauri-apps/api/event");
await emit("hud:stop", null);    // global broadcast, payload can be null
await emit("hud:amplitude", { bars: [0.1, 0.8, 0.4, ...] });
```

### Confirmed: Existing setupListeners pattern (playback-store.svelte.ts lines 202–235)
```typescript
async setupListeners(): Promise<void> {
  if (!isTauri) return;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    // Add emit here: const { listen, emit } = ...
    // Store: this._emit = emit;
    const unAudioReady = await listen<string>("audio-ready", async (e) => { ... });
    // ...
  }
}
```

### Confirmed: hide_hud Rust function (src-tauri/src/hud.rs line 209–218)
```rust
pub fn hide_hud(app: &AppHandle) {
    if let Some(hud_window) = app.get_webview_window("hud") {
        let _ = app.emit("hud:stop", ());  // triggers CSS opacity transition
        let _ = hud_window.hide();          // OS-level window hide
    }
}
```
Frontend `emit("hud:stop")` triggers the CSS transition only. For OS-level hide, `invoke("hide_hud")` is needed. The CONTEXT.md accepts either — CSS opacity to 0 is sufficient UX.

### Confirmed: AnalyserNode getByteFrequencyData pattern (Web Audio API standard)
```typescript
// fftSize=256 → 128 frequency bins (frequencyBinCount = fftSize / 2)
const analyser = audioCtx.createAnalyser();
analyser.fftSize = 256;
analyser.smoothingTimeConstant = 0.8;  // built-in smoothing (0=none, 1=max)
const dataArray = new Uint8Array(analyser.frequencyBinCount);  // 128 values, 0–255
analyser.getByteFrequencyData(dataArray);  // fill in place
```

---

## File Change Map

### New Plan 04: Bug Fix (hud:stop emission)

| File | Change | Scope |
|------|--------|-------|
| `src/lib/stores/playback-store.svelte.ts` | Add `_emit` field; capture `emit` in `setupListeners()`; call `this._emit?.("hud:stop", null)` in `onended` handler and `handleStop()` | Small — 4 line additions + 1 field |

### New Plan 05: Waveform Overhaul (AnalyserNode)

| File | Change | Scope |
|------|--------|-------|
| `src/lib/stores/playback-store.svelte.ts` | Add `_analyser`, `_sourceNode`, `_amplitudeLoopId`, `_lastEmitTime` fields; wire AnalyserNode in `handleAudioReady()`; add `startAmplitudeLoop()` and `stopAmplitudeLoop()` methods; call them from `playAudio()` / `handleStop()` / `onended` | Medium — ~60 lines added |
| `src/lib/components/waveform.svelte` | Replace Props interface (remove `envelope`, `isPlaying`, `animationSpeed`; add `barValues: number[]`); remove animate loop; simplify `drawWaveform()` to use barValues directly; add `$effect` on barValues | Medium — component shrinks by ~50 lines |
| `src/lib/components/hud-overlay.svelte` | Add `barValues = $state<number[]>([])`; add `hud:amplitude` listener in `onMount`; pass `{barValues}` to `<Waveform>` instead of `{envelope}` and `{isPlaying}`; wrap waveform in `{#if !isSynthesizing}` | Small — ~15 lines changed |

---

## Plan Sequencing

| Plan # | Name | Depends On | Wave |
|--------|------|-----------|------|
| 08-04 | Bug Fix: emit hud:stop in playbackStore | (none — standalone) | Wave 2 |
| 08-05 | Waveform Overhaul: AnalyserNode pipeline | 08-04 | Wave 2 |

Plans 01–03 are already complete. Plan 04 (old) is replaced — do not execute it.

Plans 04 and 05 can be placed in the same wave since 05 builds on the same store file 04 modifies. Sequential execution within wave is correct.

---

## Sources

### Primary (HIGH confidence)
- Direct inspection of `src/lib/stores/playback-store.svelte.ts` — confirmed `_audioCtx`, `_audioEl` fields, `setupListeners()` pattern, missing emit calls
- Direct inspection of `src/lib/components/hud-overlay.svelte` — confirmed `hud:stop` listener present, confirmed `barValues` interface change needed
- Direct inspection of `src/lib/components/waveform.svelte` — confirmed current envelope-based Props, canvas rendering code to be preserved
- Direct inspection of `src-tauri/src/hud.rs` — confirmed `hide_hud()` emits `hud:stop` globally + calls `hud_window.hide()`
- Direct inspection of `@tauri-apps/api/event` usage in codebase — confirmed `emit()` is the correct cross-window broadcast API
- `.planning/phases/08-hud-enhancement/08-CONTEXT.md` — all locked decisions and code context

### Secondary (MEDIUM confidence)
- Web Audio API `AnalyserNode` and `createMediaElementSource()` — standard browser API, well-established, no library dependency

---

## Metadata

**Confidence breakdown:**
- Bug fix location and pattern: HIGH — confirmed by direct code inspection, root cause is unambiguous
- AnalyserNode architecture: HIGH — standard Web Audio API, confirmed `_audioCtx` and `_audioEl` exist as needed
- Tauri emit cross-window: HIGH — confirmed by existing codebase usage pattern
- FFT bin grouping approach: MEDIUM — logarithmic grouping is recommended practice, exact parameters are Claude's discretion
- Emit rate at 30fps: MEDIUM — reasonable target, may need tuning based on actual IPC overhead

**Research date:** 2026-03-07
**Valid until:** N/A — all findings based on current codebase, no external API changes expected
