---
phase: quick-2
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/lib/components/synthesize-page.svelte
  - src/lib/components/settings/playback-settings.svelte
  - src/lib/components/quick-settings.svelte
  - CHANGELOG.md
autonomous: true
requirements: [QUICK-2]

must_haves:
  truths:
    - "User can adjust Pitch slider (0.5x–2.0x) in Playback Settings below the Speed slider"
    - "User can adjust Pitch slider in Quick Settings on the Play page alongside Speed"
    - "Pitch change takes effect on the currently playing audio and on next playback"
    - "Pitch is client-side only — no Rust/backend changes, no config persistence"
    - "Speed slider still works correctly after audio playback refactor"
  artifacts:
    - path: "src/lib/components/synthesize-page.svelte"
      provides: "Web Audio API context, decoded AudioBuffer, pitch ($state), detune wiring"
    - path: "src/lib/components/settings/playback-settings.svelte"
      provides: "Pitch slider (0.5x–2.0x, step 0.05) below Speed slider"
    - path: "src/lib/components/quick-settings.svelte"
      provides: "Speed + Pitch compact sliders in Quick Settings card"
  key_links:
    - from: "src/lib/components/synthesize-page.svelte"
      to: "AudioBufferSourceNode.detune"
      via: "cents = 1200 * log2(pitch)"
      pattern: "detune.*cents|1200.*log2"
    - from: "src/lib/components/quick-settings.svelte"
      to: "src/lib/components/synthesize-page.svelte"
      via: "pitch prop (bindable) passed from synthesize-page"
      pattern: "bind:pitch"
---

<objective>
Add a client-side Pitch control (0.5x–2.0x ratio) to Playback Settings (below Speed) and to Quick Settings on the Play page. Pitch is applied via Web Audio API AudioBufferSourceNode.detune (cents = 1200 * log2(ratio)) with no backend changes.

Purpose: Users want independent pitch control without engine-level processing.
Output: Pitch slider in settings + Quick Settings; Web Audio API wiring in synthesize-page.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/quick/2-i-need-to-add-a-new-pitch-control-it-sho/2-CONTEXT.md

<interfaces>
<!-- Key contracts the executor needs. No codebase exploration required. -->

From src/lib/types.ts:
```typescript
export interface PlaybackConfig {
  on_retrigger: RetriggerMode;
  volume: number;
  playback_speed: number;
  // NOTE: pitch is NOT added here — it is frontend-only ephemeral state
}
```

From src/lib/components/synthesize-page.svelte (current audio wiring):
```typescript
// audio-ready event handler (base64 WAV → blob URL → audioElement.src)
// Currently: audioElement.playbackRate = config?.playback.playback_speed ?? 1.0
// After refactor: use AudioContext + AudioBufferSourceNode for pitch + speed

let audioElement = $state<HTMLAudioElement | null>(null);
let audioBlobUrl = $state<string | null>(null);
```

From src/lib/components/quick-settings.svelte (current props):
```typescript
interface Props { config?: AppConfig; }
let { config = $bindable() }: Props = $props();
// Currently shows: Listen toggle only
// After: also shows Speed slider (from config.playback.playback_speed) + Pitch slider
```

From src/lib/components/settings/playback-settings.svelte (current props):
```typescript
let {
  localConfig = $bindable(),
  retriggerModeOptions,
}: {
  localConfig: AppConfig;
  retriggerModeOptions: { value: string; label: string }[];
} = $props();
// Speed slider pattern (copy for Pitch):
// <Label for="playback-speed">Speed: {localConfig.playback.playback_speed.toFixed(2)}x</Label>
// <Slider id="playback-speed" min={0.25} max={4} step={0.05} bind:value={localConfig.playback.playback_speed} />
```

Pitch conversion (per user decision):
- Display: ratio (0.5x–2.0x, default 1.0x, step 0.05)
- Web Audio API: cents = 1200 * Math.log2(ratio)
- 0.5x = -1200 cents, 1.0x = 0 cents, 2.0x = +1200 cents
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Refactor synthesize-page audio to Web Audio API + add pitch state</name>
  <files>src/lib/components/synthesize-page.svelte</files>
  <action>
Refactor the audio playback in synthesize-page.svelte to use the Web Audio API so that pitch (detune) can be applied independently from speed. Add ephemeral pitch state.

**New state to add:**
```svelte
let pitch = $state(1.0); // 0.5x–2.0x ratio, client-side only
```

**Web Audio API approach:**
Replace the `audioBlobUrl` + `audioElement.src` approach with an AudioContext-based pipeline that decodes the WAV buffer and plays it via `AudioBufferSourceNode`. The hidden `<audio>` element is no longer needed for playback — remove it. Use AudioContext for all playback.

**Key implementation details:**

1. Add at the top of `<script>`:
   ```typescript
   let audioCtx = $state<AudioContext | null>(null);
   let decodedBuffer = $state<AudioBuffer | null>(null);
   let currentSource = $state<AudioBufferSourceNode | null>(null);
   let gainNode = $state<GainNode | null>(null);
   let pitch = $state(1.0);
   ```

2. In the `audio-ready` listener, replace blob URL logic with:
   - Convert base64 → ArrayBuffer (same binary decode loop, but into ArrayBuffer)
   - Lazy-create `audioCtx` if not present: `audioCtx = new AudioContext()`
   - `decodedBuffer = await audioCtx.decodeAudioData(arrayBuffer.slice(0))`
   - Call `playDecodedBuffer()` immediately

3. Create a `playDecodedBuffer()` function:
   ```typescript
   function playDecodedBuffer() {
     if (!audioCtx || !decodedBuffer) return;
     // Stop any current source
     if (currentSource) {
       currentSource.disconnect();
       currentSource.stop();
       currentSource = null;
     }
     gainNode = audioCtx.createGain();
     gainNode.gain.value = (config?.playback.volume ?? 100) / 100;
     gainNode.connect(audioCtx.destination);

     currentSource = audioCtx.createBufferSource();
     currentSource.buffer = decodedBuffer;
     currentSource.playbackRate.value = config?.playback.playback_speed ?? 1.0;
     currentSource.detune.value = 1200 * Math.log2(pitch);
     currentSource.connect(gainNode);
     currentSource.onended = () => {
       isPlaying = false;
       isPaused = false;
       currentSource = null;
     };
     currentSource.start(0);
     isPlaying = true;
     isPaused = false;
   }
   ```

4. Update `handleStop()` to stop `currentSource` and reset `isPlaying`/`isPaused`:
   ```typescript
   function handleStop() {
     if (currentSource) {
       currentSource.stop();
       currentSource = null;
     }
     isPlaying = false;
     isPaused = false;
     // keep Tauri invoke for backend stop
     if (isTauri && invoke) invoke("stop_speaking").catch(() => {});
   }
   ```

5. Update `handleTogglePause()` to use AudioContext suspend/resume:
   ```typescript
   function handleTogglePause() {
     if (!audioCtx) return;
     if (audioCtx.state === 'suspended') {
       audioCtx.resume().then(() => { isPaused = false; });
     } else {
       audioCtx.suspend().then(() => { isPaused = true; });
     }
   }
   ```

6. Update `handleReplay()` to call `playDecodedBuffer()` instead of resetting `audioElement`.

7. Remove the `<audio>` element, `audioElement` state, and `audioBlobUrl` state entirely. Remove blob URL revocation in onDestroy — replace with `audioCtx?.close()`.

8. Add `$effect` to update gain/detune when config.playback.volume, config.playback.playback_speed, or pitch changes while playing:
   ```typescript
   $effect(() => {
     if (gainNode) gainNode.gain.value = (config?.playback.volume ?? 100) / 100;
     if (currentSource) {
       currentSource.playbackRate.value = config?.playback.playback_speed ?? 1.0;
       currentSource.detune.value = 1200 * Math.log2(pitch);
     }
   });
   ```
   Note: AudioBufferSourceNode params are live-updateable during playback.

9. Pass `pitch` as bindable prop to `QuickSettings`:
   ```svelte
   <QuickSettings bind:config bind:pitch />
   ```

10. In the `playback-stop` Tauri event listener, call `handleStop()`.

11. Keep the `unlistenAudioReady`, `unlistenPlaybackStop`, `unlistenPlaybackTogglePause` listeners — update their bodies to use the new functions.

**Svelte 5 code style:** Use `$state`, `$effect`, `$derived`, Svelte 5 runes throughout. Use `onclick` not `on:click`.
  </action>
  <verify>
    <automated>cd /mnt/d/GitHub/CopySpeak && rtk bun run check</automated>
  </verify>
  <done>TypeScript/Svelte type check passes. synthesize-page.svelte has pitch $state, AudioContext-based playback, no HTMLAudioElement, and passes pitch to QuickSettings.</done>
</task>

<task type="auto">
  <name>Task 2: Add Pitch slider to Playback Settings and Quick Settings</name>
  <files>
    src/lib/components/settings/playback-settings.svelte,
    src/lib/components/quick-settings.svelte,
    CHANGELOG.md
  </files>
  <action>
**playback-settings.svelte:**

Add a Pitch slider below the existing Speed slider block. The component receives `localConfig` (AppConfig) which does NOT have a pitch field — pitch is managed via a new bindable `pitch` prop.

Update the component props to accept pitch:
```svelte
let {
  localConfig = $bindable(),
  retriggerModeOptions,
  pitch = $bindable(1.0),
}: {
  localConfig: AppConfig;
  retriggerModeOptions: { value: string; label: string }[];
  pitch: number;
} = $props();
```

Add pitch slider block after the Speed block (copy Speed block structure):
```svelte
<div class="space-y-2">
  <Label for="playback-pitch">Pitch: {pitch.toFixed(2)}×</Label>
  <Slider
    id="playback-pitch"
    min={0.5}
    max={2.0}
    step={0.05}
    bind:value={pitch}
  />
  <p class="text-xs text-muted-foreground">
    Pitch shift (0.5×–2.0×). Applied via browser audio in real-time. Default: 1.0×.
  </p>
</div>
```

Also check the settings page (`src/routes/settings/+page.svelte`) — if it renders `<PlaybackSettings>`, add `bind:pitch={localPitch}` there. If there's no local pitch state on settings page, that's fine — the settings page saves to config. For pitch (not in config), you need to thread the pitch state through from somewhere. Since pitch is ephemeral and lives in synthesize-page, the Settings page will have its OWN local pitch state that is disconnected (i.e., it shows/controls pitch but it's a separate instance). This is acceptable for now — the Quick Settings on Play page is the primary pitch control. Add a note in the description: "Changes here apply on next Play page visit."

Read `src/routes/settings/+page.svelte` to understand exactly how PlaybackSettings is used there before making changes.

**quick-settings.svelte:**

Update props to accept pitch:
```svelte
interface Props {
  config?: AppConfig;
  pitch?: number;
}
let { config = $bindable(), pitch = $bindable(1.0) }: Props = $props();
```

Add Speed and Pitch compact sliders below the existing Listen toggle section, inside the `space-y-4` div. Import `Slider` from `$lib/components/ui/slider/index.js`.

```svelte
{#if config}
  <div class="space-y-3 pt-2 border-t border-border">
    <div class="space-y-1">
      <div class="flex items-center justify-between">
        <Label for="qs-speed" class="text-sm">Speed</Label>
        <span class="text-xs text-muted-foreground">{config.playback.playback_speed.toFixed(2)}×</span>
      </div>
      <Slider
        id="qs-speed"
        min={0.25}
        max={4}
        step={0.05}
        bind:value={config.playback.playback_speed}
      />
    </div>
    <div class="space-y-1">
      <div class="flex items-center justify-between">
        <Label for="qs-pitch" class="text-sm">Pitch</Label>
        <span class="text-xs text-muted-foreground">{pitch.toFixed(2)}×</span>
      </div>
      <Slider
        id="qs-pitch"
        min={0.5}
        max={2.0}
        step={0.05}
        bind:value={pitch}
      />
    </div>
  </div>
{/if}
```

**CHANGELOG.md:**

Add under `[Unreleased]` → `Added`:
```markdown
### Added
- Client-side Pitch control (0.5×–2.0×) in Playback Settings below Speed slider
  - Applied via Web Audio API `AudioBufferSourceNode.detune` (cents = 1200 × log2(ratio))
  - No backend/engine changes — purely browser-native audio processing
- Quick Settings on Play page now shows compact Speed and Pitch sliders for in-session adjustment
- Refactored audio playback in synthesize-page to use Web Audio API (AudioContext + AudioBufferSourceNode) enabling independent pitch and speed control
```
  </action>
  <verify>
    <automated>cd /mnt/d/GitHub/CopySpeak && rtk bun run check</automated>
  </verify>
  <done>Type check passes. Playback Settings has Pitch slider below Speed. Quick Settings has Speed + Pitch sliders. CHANGELOG updated.</done>
</task>

</tasks>

<verification>
1. `bun run check` passes (zero type errors)
2. `playback-settings.svelte` has Pitch slider (min=0.5, max=2.0, step=0.05) below Speed
3. `quick-settings.svelte` has both Speed and Pitch sliders with live value display
4. `synthesize-page.svelte` uses AudioContext, not HTMLAudioElement, for playback
5. `synthesize-page.svelte` has `pitch` $state and passes it to QuickSettings via bind:pitch
6. No Rust files modified
7. CHANGELOG.md updated under [Unreleased]
</verification>

<success_criteria>
- Pitch slider appears in Playback Settings below Speed, range 0.5×–2.0×, default 1.0×
- Pitch slider appears in Quick Settings on Play page alongside Speed slider
- Changing pitch during playback updates audio pitch in real-time (via detune $effect)
- Speed still works correctly (playbackRate on AudioBufferSourceNode)
- Zero TypeScript/Svelte type errors
- No backend changes
</success_criteria>

<output>
After completion, create `.planning/quick/2-i-need-to-add-a-new-pitch-control-it-sho/2-SUMMARY.md` using the summary template.
</output>
