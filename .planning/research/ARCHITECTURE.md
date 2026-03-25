# Architecture Research

**Domain:** Tauri v2 + SvelteKit single-window desktop app — 3-tab navigation with engine config extraction
**Researched:** 2026-03-04
**Confidence:** HIGH — based on direct codebase inspection, no speculation required

---

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    Tauri Single Window                       │
├──────────────────────────────────────────────────────────────┤
│  src/routes/+layout.svelte  (shared shell, event listeners)  │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  AppHeader  (nav: Play | Engine | Settings)            │  │
│  ├────────────────────────────────────────────────────────┤  │
│  │  <main>  {@render children()}                          │  │
│  │  ┌──────────┐  ┌──────────────┐  ┌──────────────────┐  │  │
│  │  │  /       │  │  /engine     │  │  /settings       │  │  │
│  │  │  Play    │  │  Engine      │  │  Settings        │  │  │
│  │  │  page    │  │  page (new)  │  │  page (trimmed)  │  │  │
│  │  └──────────┘  └──────────────┘  └──────────────────┘  │  │
│  ├────────────────────────────────────────────────────────┤  │
│  │  AppFooter                                             │  │
│  └────────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────────┤
│                  SvelteKit SPA Router                        │
│  prerender=true  ssr=false  (adapter-static for Tauri)       │
├──────────────────────────────────────────────────────────────┤
│                  Rust Backend (Tauri IPC)                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │config.rs │  │  tts.rs  │  │history.rs│  │playback  │      │
│  │get/set   │  │test_tts  │  │          │  │          │      │
│  │reset     │  │list_     │  │          │  │          │      │
│  │validate  │  │voices    │  │          │  │          │      │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘      │
│            Mutex<AppConfig>  (single source of truth)        │
└──────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component                          | Responsibility                                                                                              | Communicates With                                                            |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `+layout.svelte`                   | Global event listeners (speak-request, synthesis-state-change, history-updated), AppHeader, AppFooter shell | Rust via Tauri events; all route pages via slot                              |
| `AppHeader`                        | Tab nav (Play / Engine / Settings); active-route highlighting via `$app/state page`                         | SvelteKit router only                                                        |
| `/` route page                     | TTS play status, clipboard listening indicator, Speak Now button                                            | `synthesisStore`, `listen-store`, IPC: `speak_now`, `get_listening_state`    |
| `/engine` route page (new)         | Backend selector, credential/command config, health check, voice browsing                                   | IPC: `get_config`, `set_config`, `test_tts_engine`, `list_elevenlabs_voices` |
| `/settings` route page (trimmed)   | General, Playback, Trigger, Sanitization, History config; import/export                                     | IPC: `get_config`, `set_config`, `reset_config`, `run_history_cleanup`       |
| `TtsSettings` component (existing) | Renders CLI/HTTP/OpenAI/ElevenLabs form fields — will move to `/engine`                                     | Receives `localConfig` via bindable prop                                     |
| `synthesis-store.svelte.ts`        | Tracks `isSynthesizing` boolean, set by Rust `synthesis-state-change` event                                 | Layout (sets up listener), Play page (reads state)                           |
| `listening-store.svelte.ts`        | Tracks clipboard listening state                                                                            | Play page                                                                    |
| `history-store.svelte.ts`          | Speech history items                                                                                        | Play page, possibly Engine health-check context                              |

---

## Recommended Project Structure

After this milestone the frontend tree should look like:

```
src/
├── routes/
│   ├── +layout.svelte         # Shell: global events, AppHeader, AppFooter
│   ├── +layout.ts             # prerender=true, ssr=false (unchanged)
│   ├── +layout.css            # Global styles (unchanged)
│   ├── +page.svelte           # /  — Play route (unchanged)
│   ├── engine/
│   │   └── +page.svelte       # /engine — new Engine route
│   └── settings/
│       └── +page.svelte       # /settings — trimmed (TTS section removed)
├── lib/
│   ├── components/
│   │   ├── layout/
│   │   │   ├── app-header.svelte   # Add Engine tab to navItems array
│   │   │   └── app-footer.svelte   # Unchanged
│   │   ├── engine/                 # NEW: Engine-specific components
│   │   │   ├── backend-selector.svelte      # Backend radio/select
│   │   │   ├── engine-health-check.svelte   # Test button + result display
│   │   │   ├── engine-credential-form.svelte # API key / command fields
│   │   │   └── voice-browser.svelte         # ElevenLabs voice list
│   │   └── settings/               # Existing; TtsSettings stays here or moves
│   │       ├── tts-settings.svelte          # Move to engine/ or keep, re-export
│   │       ├── general-settings.svelte
│   │       ├── playback-settings.svelte
│   │       ├── trigger-settings.svelte
│   │       ├── sanitization-settings.svelte
│   │       ├── history-settings.svelte
│   │       ├── appearance-settings.svelte
│   │       └── import-export-settings.svelte
│   ├── stores/
│   │   ├── synthesis-store.svelte.ts   # Unchanged
│   │   ├── listening-store.svelte.ts   # Unchanged
│   │   └── history-store.svelte.ts     # Unchanged
│   └── types.ts                        # AppConfig, TtsConfig, etc. (unchanged)
```

### Structure Rationale

- **`engine/` route:** Giving TTS engine config its own SvelteKit route means SPA navigation handles tab switching with zero reload. State is fresh on each visit — no stale data carry-over from Settings.
- **`lib/components/engine/`:** Groups the new Engine components separately from generic settings widgets. `TtsSettings` can be refactored into smaller pieces or simply re-used inside the Engine page via a thin wrapper.
- **`settings/+page.svelte` trimmed:** Remove the `section-tts` block entirely. The sidebar `settingsCategories` array shrinks from 6 items to 5. No Rust changes needed.

---

## Architectural Patterns

### Pattern 1: Per-Route Config Load (current pattern — keep it)

**What:** Each route page independently calls `invoke("get_config")` on mount, maintains a `localConfig` copy, and saves via `invoke("set_config", { newConfig })`. A "save bar" floats when `localConfig !== originalConfig`.

**When to use:** All config-editing pages (Settings, Engine). Single-source of truth is the Rust `Mutex<AppConfig>` — each tab gets a fresh snapshot on mount.

**Trade-offs:** No cross-tab reactive sync (if user edits Settings and Engine simultaneously in two browser tabs this would diverge, but Tauri is a single window so this cannot happen). Clean — no shared frontend config store to manage.

**Example (current pattern, Settings page):**
```typescript
// /engine/+page.svelte will follow the same pattern
let localConfig = $state<AppConfig | null>(null);
let originalConfig = $state<AppConfig | null>(null);

onMount(async () => {
  const config = await invoke<AppConfig>("get_config");
  localConfig = JSON.parse(JSON.stringify(config));
  originalConfig = JSON.parse(JSON.stringify(config));
});

async function saveConfig() {
  await invoke("set_config", { newConfig: localConfig });
  originalConfig = JSON.parse(JSON.stringify(localConfig));
}
```

**Why not a shared Svelte config store:** The Rust backend is the authoritative store. A frontend config store would require invalidation logic whenever config changes from outside the frontend (tray menu, file edit). Avoiding it keeps the architecture simpler.

### Pattern 2: SvelteKit File-Based Routing as Tab Navigation

**What:** Each tab is a SvelteKit route (`/`, `/engine`, `/settings`). The shared `+layout.svelte` renders `AppHeader` with `<a href="/engine">` links. SvelteKit's client-side router handles navigation without page reload. The layout's `onMount` listeners (speak-request, synthesis-state-change) persist across tab switches because the layout is never torn down during SPA navigation.

**When to use:** Any navigation in this app. This is the idiomatic Tauri + SvelteKit pattern; do not deviate from it.

**Trade-offs:** Route-level state (localConfig, health check result) is reset on tab switch. This is desirable for the Engine page — a re-visit always reflects the current saved config. If persistent tab state were needed, stores would be required.

**Example — AppHeader change needed:**
```typescript
// app-header.svelte: add Engine to navItems
const navItems = [
  { id: "play",     label: "Play",     href: "/",        icon: Play    },
  { id: "engine",   label: "Engine",   href: "/engine",  icon: Cpu     },
  { id: "settings", label: "Settings", href: "/settings", icon: Settings },
];
```

### Pattern 3: Startup Health Check via Layout onMount

**What:** The app needs to detect on startup whether the TTS engine is configured and working, then redirect or show a prompt if broken. This belongs in `+layout.svelte`'s `onMount`, not in any individual route, so it fires once regardless of which tab the user enters on.

**When to use:** Startup-only checks. Do not repeat the health check on every route visit.

**Trade-offs:** The layout `onMount` already has two async blocks (event listeners and appearance sync). A third block for health check is fine — they are independent. Keep them separate for readability, not chained.

**Proposed flow:**
```typescript
// +layout.svelte onMount addition
onMount(async () => {
  try {
    const result = await invoke<TtsHealthResult>("test_tts_engine");
    if (!result.success) {
      // Navigate to /engine with a flag, or show a toast with action button
      goto("/engine?setup=true");
    }
  } catch {
    // Silently ignore — don't block app startup on health check failure
  }
});
```

**Implementation note:** Use `goto("/engine?setup=true")` from `$app/navigation` rather than a modal. The Engine page reads the `setup` query param and renders a setup-mode banner. This avoids a new modal component and uses the natural navigation pattern.

---

## Data Flow

### Config Edit Flow (Engine page and Settings page — identical)

```
User edits form field
    ↓
Svelte $state mutation (localConfig)
    ↓
$derived hasChanges detects diff vs originalConfig
    ↓
Save bar appears
    ↓ (user clicks Save)
invoke("set_config", { newConfig: localConfig })
    ↓
Rust: Mutex<AppConfig> updated + config.json written to %APPDATA%/CopySpeak/
    ↓
originalConfig = deep clone of localConfig
    ↓
Save bar disappears
```

### Engine Health Check Flow

```
User clicks "Test Engine" (or startup check triggers)
    ↓
invoke("test_tts_engine")  [synchronous Rust command]
    ↓
Rust: reads Mutex<AppConfig>, creates TTS backend, calls backend.health_check()
    ↓
Returns TtsHealthResult { success, message, error_type }
    ↓
Frontend: displays result inline (success = green, failure = red + error_type guidance)
```

Note: `test_tts_engine` is already implemented in `src-tauri/src/commands/tts.rs`. It returns structured `TtsHealthResult` including `error_type` (not_found, api_key_missing, auth_failed, etc.). The Engine page UI should render different guidance text per `error_type`.

### TTS Engine Config to Speech Flow (no change needed)

```
User double-copies text
    ↓
Win32 clipboard event → Rust clipboard.rs state machine
    ↓
Emits "speak-request" Tauri event
    ↓
layout.svelte listener calls invoke("speak_queued", { text })
    ↓
Rust: reads Mutex<AppConfig> for active_backend + credentials
    ↓
Creates TTS backend, synthesizes audio
    ↓
Emits "audio-ready" event with base64 WAV
    ↓
Frontend audio player plays WAV
```

The Engine page config changes take effect on the next speech synthesis call. No restart required — Rust reads from `Mutex<AppConfig>` at synthesis time, not at startup.

### Voice Listing Flow (ElevenLabs)

```
User selects ElevenLabs backend + enters API key
    ↓
$effect triggers invoke("list_elevenlabs_voices")
    ↓
Rust: creates ElevenLabsTtsBackend, calls list_voices() via HTTP
    ↓
Returns Vec<ElevenLabsVoice> to frontend
    ↓
Voice selector renders the list
    ↓
User selects voice → mutates localConfig.tts.elevenlabs.voice_id
    ↓
Save stores it in config.json
```

---

## Component Boundaries

### What the Engine Page Owns

The Engine page (`/engine/+page.svelte`) is the sole owner of:

- Backend selector (active_backend field)
- CLI preset selector + command/args fields
- HTTP preset + URL/body template fields
- OpenAI API key + model + voice fields
- ElevenLabs API key + voice selector + model + output format + voice settings sliders
- Health check trigger button + health result display
- Save bar (same pattern as Settings)

The Engine page does NOT own:
- Playback settings (volume, retrigger mode) — stays in Settings
- Trigger settings (double-copy window) — stays in Settings
- General settings (autostart, appearance) — stays in Settings

### What Stays in Settings

After TTS section is removed, Settings owns:
- General (autostart, start minimized, close behavior, debug mode)
- Appearance (dark/light/system)
- Playback (volume, speed, retrigger mode)
- Triggers (double-copy window ms, max text length)
- Sanitization (markdown stripping, text normalization)
- History (storage mode, auto-delete, cleanup)
- Import/Export + Reset to Defaults

### Shared via AppConfig (Rust single source of truth)

Both pages read from `Mutex<AppConfig>` on mount. Both write to it via `set_config`. There is no shared frontend store for config — Rust is the store.

---

## Build Order (Phase Dependencies)

The milestone has clear dependency ordering:

**Phase 1 — Navigation shell (no dependencies)**
- Add Engine nav item to `AppHeader` (`navItems` array + Cpu icon import)
- Create `src/routes/engine/+page.svelte` skeleton (placeholder text)
- Verify 3-tab navigation works; no other changes

**Phase 2 — Engine page core (depends on Phase 1)**
- Build `/engine` page with backend selector + per-backend credential forms
- Reuse `TtsSettings` component OR decompose into `engine/` components
- Wire `get_config` / `set_config` IPC (same pattern as Settings page)
- Add save bar (copy pattern from Settings page)
- Test: can change backend, fill credentials, save config, verify persistence

**Phase 3 — Health check UI (depends on Phase 2)**
- Add health check button to Engine page
- Call existing `test_tts_engine` IPC command
- Render `TtsHealthResult` with per-`error_type` guidance text
- Test: each backend type produces correct diagnostic message

**Phase 4 — Remove TTS from Settings (depends on Phase 2 working correctly)**
- Delete `section-tts` block from `settings/+page.svelte`
- Remove `tts` from `settingsCategories` array
- Remove TTS-related state (ttsPresetOptions, isTtsHealthChecking, testTtsEngine, etc.) from Settings page
- Verify Settings page still saves/loads correctly without TTS section

**Phase 5 — Startup health check (depends on Phase 3)**
- Add `onMount` block in `+layout.svelte`
- Call `test_tts_engine` on startup
- On failure: `goto("/engine?setup=true")`
- Engine page reads `$page.url.searchParams.get("setup")` and renders setup banner
- Test: fresh install or broken config routes to Engine tab automatically

**Phase 6 — Voice browser enhancement (depends on Phase 3)**
- ElevenLabs: auto-load voices when API key is present and valid (already partially implemented in `TtsSettings`)
- Piper: static voice list with download links (already exists in `TtsSettings`)
- Verify voice selection saves correctly

---

## Scaling Considerations

This is a desktop app — single user, single machine. "Scaling" means component complexity management, not load.

| Concern              | Current (2 tabs)                      | After (3 tabs)                                                |
| -------------------- | ------------------------------------- | ------------------------------------------------------------- |
| Config load time     | ~1 IPC call per tab visit             | Same — each tab loads independently                           |
| Save conflicts       | Cannot happen (single window)         | Cannot happen                                                 |
| Component complexity | Settings page has 8 sections          | Engine page will have 4-5 backend-specific sections           |
| WebView2 crash risk  | `mountedCount` stagger pattern exists | Engine page should use same stagger pattern for complex forms |

---

## Anti-Patterns

### Anti-Pattern 1: Shared Frontend Config Store

**What people do:** Create a Svelte store for `AppConfig` that both Settings and Engine subscribe to, syncing changes reactively.

**Why it's wrong:** The Rust backend is the authoritative store. A frontend config store creates a second source of truth that can drift (e.g., config changed by tray menu while Settings is open). The current per-route load-on-mount pattern is correct for this architecture.

**Do this instead:** Each route loads config fresh on mount. Changes are committed to Rust immediately on Save. No frontend config store.

### Anti-Pattern 2: Inline TTS Logic in the Engine Route

**What people do:** Put ElevenLabs voice loading, health check logic, and preset application directly in `/engine/+page.svelte` as a 600-line file.

**Why it's wrong:** The existing `TtsSettings` component already encapsulates this logic. Duplicating it creates two places to maintain.

**Do this instead:** Extract or reuse `TtsSettings` on the Engine page. If the component needs to evolve (e.g., add setup-mode banner), extend it via props rather than duplicating. Alternatively move it to `lib/components/engine/engine-config.svelte` and import it from both places if needed.

### Anti-Pattern 3: Startup Health Check as a Blocking Modal

**What people do:** Show a modal on launch that must be dismissed before the app is usable.

**Why it's wrong:** If the health check takes time (network timeout for cloud backends), the app is blocked. Users who know what they're doing are frustrated. CopySpeak already works — the play page should be accessible even if TTS is broken.

**Do this instead:** Navigate to `/engine?setup=true` silently. The Engine page shows a non-blocking banner at the top: "Your TTS engine needs configuration." The Play page remains accessible; the user can dismiss the setup state.

### Anti-Pattern 4: Separate `+page.ts` Load Functions for Config

**What people do:** Use SvelteKit's `load()` function in `+page.ts` to fetch config before the page renders.

**Why it's wrong:** In Tauri with `ssr=false` and `prerender=true`, the `load()` function runs at build time when Tauri IPC is not available. This causes the build to fail or the config fetch to silently produce nothing.

**Do this instead:** Fetch config in `onMount` (as the Settings page already does). This is the established pattern for the entire codebase — do not deviate.

---

## Integration Points

### Internal Boundaries

| Boundary                       | Communication                                 | Notes                                                            |
| ------------------------------ | --------------------------------------------- | ---------------------------------------------------------------- |
| Engine page ↔ Rust config      | IPC: `get_config` / `set_config`              | Same pattern as Settings; no new IPC commands needed             |
| Engine page ↔ health check     | IPC: `test_tts_engine` (sync command)         | Already implemented; returns `TtsHealthResult` with `error_type` |
| Engine page ↔ voice list       | IPC: `list_elevenlabs_voices`                 | Already implemented; requires valid API key in config            |
| Layout ↔ Engine page (startup) | SvelteKit `goto()` + URL search param         | `goto("/engine?setup=true")` from layout onMount                 |
| Settings page ↔ TTS section    | Remove entirely                               | After Phase 4, Settings no longer references TTS config fields   |
| AppHeader ↔ router             | `page.url.pathname` reactive via `$app/state` | Active tab detection already uses this pattern                   |

### No New Rust IPC Commands Required

The full Engine page feature set is supported by existing commands:
- `get_config` — load current config
- `set_config` — save modified config
- `test_tts_engine` — health check (returns `TtsHealthResult`)
- `list_elevenlabs_voices` — voice listing for ElevenLabs
- `get_elevenlabs_output_formats` — format options

The only new Rust work needed is the startup health check trigger, which reuses `test_tts_engine` from the layout — no new command required.

---

## Sources

- Direct codebase inspection (HIGH confidence):
  - `src/routes/+layout.svelte` — current shell structure and event listener pattern
  - `src/routes/settings/+page.svelte` — config load/save pattern, section structure
  - `src/lib/components/layout/app-header.svelte` — navItems array, active-route detection
  - `src/lib/components/settings/tts-settings.svelte` — TTS form component scope
  - `src-tauri/src/commands/tts.rs` — `test_tts_engine`, `TtsHealthResult`, `list_elevenlabs_voices`
  - `src/lib/types.ts` — `AppConfig`, `TtsConfig`, `TtsHealthResult` shape
  - `src/routes/+layout.ts` — `prerender=true`, `ssr=false` (Tauri SPA mode)
- Tauri v2 official docs pattern: load function restriction with prerender+ssr=false (MEDIUM confidence — well-known limitation, consistent with codebase's onMount usage)

---

*Architecture research for: CopySpeak 3-tab navigation + Engine route extraction*
*Researched: 2026-03-04*
