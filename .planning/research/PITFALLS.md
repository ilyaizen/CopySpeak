# Pitfalls Research

**Domain:** Tauri v2 + SvelteKit desktop app — multi-route navigation and startup onboarding refactor
**Researched:** 2026-03-04
**Confidence:** HIGH (findings drawn from official Tauri v2 docs, official SvelteKit docs, and verified GitHub issues)

---

## Critical Pitfalls

### Pitfall 1: Unsaved Settings State Lost When Navigating Away From Split Settings Page

**What goes wrong:**
The current Settings page (`/settings`) holds the entire `localConfig` state locally as `$state<AppConfig | null>`. When TTS engine settings are extracted into a new `/engine` route, users who make changes on `/settings`, navigate to `/engine` to check something, then navigate back will find their `/settings` changes discarded — the component remounts and calls `loadConfig()` fresh from the backend.

**Why it happens:**
SvelteKit's SPA router destroys and recreates page components on route navigation unless state is lifted into a persistent store or the layout. Each `+page.svelte` has independent component-local `$state`. When `/settings` is unmounted, its `localConfig` is garbage collected. There is no "unsaved changes" guard by default in SvelteKit.

**How to avoid:**
- Lift config draft state into a module-level Svelte 5 rune store (`.svelte.ts` singleton) so it survives route transitions.
- Use `beforeNavigate` from `$app/navigation` to intercept and warn the user if `hasChanges` is true before they leave.
- Alternatively, implement auto-save per section so navigating away always persists the change immediately.

**Warning signs:**
- Settings page loads `get_config` in `onMount` without checking whether a draft already exists in shared state.
- No `beforeNavigate` guard in `settings/+page.svelte` or `engine/+page.svelte`.
- Users or test steps that navigate between tabs lose field values.

**Phase to address:**
Navigation/routing phase (when the third route is added and Settings is split). Must be resolved before the Engine route is considered usable.

---

### Pitfall 2: Event Listeners in `+layout.svelte` Registered Multiple Times or Leaked

**What goes wrong:**
`+layout.svelte` already sets up `speak-request` and `synthesis-state-change` listeners inside `onMount`. Because the layout is a SPA singleton that is never destroyed during navigation, listeners accumulate if `onMount` is ever called more than once. Adding startup logic (health check, initial route redirect) to the same `onMount` block creates an ordering problem: if the new init code fails, the listener teardown in `onDestroy` still runs, but the listen subscription may already be partially set up, leaving orphaned handlers.

**Why it happens:**
There is a known Svelte 5 / SvelteKit issue (#10176) where returning a cleanup function from `onMount` interacts unexpectedly with navigation in some configurations. Additionally, `__TAURI_INTERNALS__` guards are inline rather than extracted, making it easy to accidentally register listeners in dev-mode browser reloads.

**How to avoid:**
- Store unlisten handles as module-level variables and guard with `if (unlistenX) return` before re-registering.
- Keep the health-check invoke call separate from the listener setup block (two distinct `onMount` calls as the existing code already does for appearance sync — this pattern is correct and should be extended, not collapsed into one block).
- Test listener counts: add a `console.count('listener registered')` probe during development and navigate between all routes to confirm the count never increments beyond 1.

**Warning signs:**
- A single clipboard copy triggers the `speak-request` handler more than once.
- `synthesis-state-change` events cause double state flips (synthesizing toggles on/off rapidly).
- The console shows Tauri event listener registration logs more than once per session.

**Phase to address:**
Layout/navigation phase. Any startup logic added to `+layout.svelte` must be audited against this pitfall before merge.

---

### Pitfall 3: Startup Health Check Blocking the Window or Causing a Race Condition

**What goes wrong:**
A health check invoked at app startup (before the window has fully rendered) causes the app to hang, show a blank white frame, or silently fail. There is a documented Tauri issue (#7546) where `invoke()` calls made during the very first load in `cargo tauri dev` do not receive responses until a manual refresh. In production builds this manifests less often, but async `invoke` calls that happen before `DOMContentLoaded` or before the Tauri bridge is injected can reject silently.

**Why it happens:**
Tauri's IPC bridge (`__TAURI_INTERNALS__`) is injected into the WebView2 context asynchronously after the window object exists but before all scripts may have run. An `invoke` call fired too early returns a promise that never resolves or rejects. If the health check is `await`ed at the top level of a `+layout.ts` load function (rather than in `onMount`), it runs before the bridge is confirmed ready.

**How to avoid:**
- Run the startup health check exclusively inside `onMount` in `+layout.svelte`, where the Tauri bridge is guaranteed to be present.
- Never place `invoke()` calls in `+layout.ts` load functions — these run during SSG/prerender and have no access to Tauri APIs.
- Guard all invoke calls with the existing `__TAURI_INTERNALS__` pattern already used in the codebase.
- Show the health check result as a non-blocking toast or banner, not as a modal that gates the entire app (users who ignore the health check should still reach the Play tab).

**Warning signs:**
- Health check logic is placed in `+layout.ts` or `+page.ts` rather than `onMount`.
- The startup flow `await`s the health check before rendering any UI.
- In dev mode the health check result never appears without a manual page refresh.

**Phase to address:**
Startup health check phase. The health check Rust command must be registered in `generate_handler!` before the frontend calls it, and the frontend must call it only from `onMount`.

---

### Pitfall 4: New Rust IPC Commands Not Registered in `generate_handler!`

**What goes wrong:**
New Tauri commands written for the Engine route (e.g., `test_engine_health`, `list_voices`, `validate_engine_config`) compile correctly in Rust but silently fail at runtime with an "invalid IPC message" or "command not found" error on the frontend. This is one of the most common Tauri gotchas.

**Why it happens:**
Tauri's `generate_handler!` macro in `main.rs` is the single registration point. You cannot call `invoke_handler` more than once — only the last call takes effect. Any command omitted from the single `generate_handler!` list is unreachable from the frontend even though it compiles. The frontend `invoke("test_engine_health", ...)` call returns a rejected promise with a generic error, not a helpful "not found" message.

**How to avoid:**
- Every new `#[tauri::command]` function in `commands/` must be added to the `generate_handler!` list in `main.rs` in the same commit that introduces the command.
- Use a test: add a Rust integration test or a frontend vitest smoke test that calls each new command and asserts it does not reject with an IPC error.
- Review the `commands/mod.rs` re-export pattern already in place — adding a module file without adding its re-export to `mod.rs` AND to `generate_handler!` is a two-step omission that is easy to miss.

**Warning signs:**
- Frontend console shows `tauri command not found` or an unhandled promise rejection on the new route.
- The Engine route renders but every action silently fails.
- Rust tests pass but frontend integration fails.

**Phase to address:**
Engine route implementation phase. Each new command needs a checklist item: (1) `#[tauri::command]`, (2) re-exported in `commands/mod.rs`, (3) listed in `generate_handler!`.

---

### Pitfall 5: TTS Engine Config Fields Duplicated Between Settings and Engine Routes

**What goes wrong:**
After splitting TTS engine settings out of the Settings page into the Engine route, two copies of the TTS config UI exist in the codebase. When a field is updated in one place (e.g., voice selection added to `TtsSettings`) it is not propagated to the Engine route's equivalent component. Users see stale values in one tab while the other has the current values.

**Why it happens:**
The existing `TtsSettings` component (`src/lib/components/settings/tts-settings.svelte`) is currently embedded in the Settings page. If the refactor copies it rather than moves it — or if new TTS fields are added to `AppConfig` after the refactor is complete — the field will only appear in whichever component the developer happened to update.

**How to avoid:**
- Perform a hard delete of TTS engine sections from `settings/+page.svelte` and `tts-settings.svelte` immediately after they are moved to the Engine route — no soft commenting out.
- The single source of truth for TTS config is `AppConfig.tts` (Rust `TtsConfig` struct). Any UI that renders or edits `TtsConfig` fields should live in exactly one route.
- Do a `grep` audit after the refactor: search for `tts.` and `TtsEngine` in `src/routes/settings/` — if any remain, the split is incomplete.

**Warning signs:**
- `tts-settings.svelte` is still imported in `settings/+page.svelte` after the Engine route is introduced.
- Two different components write to `localConfig.tts.*` fields independently.
- Users report that saving on Settings overwrites voice changes made on the Engine tab.

**Phase to address:**
Settings refactor phase. The move must be atomic: add Engine route, move TTS UI, remove from Settings, test round-trip save — all in one focused change.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Keep `localConfig` local to each page component | Simple, no shared store needed | Config state lost on navigation; both pages fight over the same backend state | Never — once there are 2+ routes that edit config, local-only state is broken |
| Copy `TtsSettings` component instead of moving it | Faster to implement Engine route | Permanent divergence of two UIs editing the same config fields | Never |
| Run health check in `+layout.ts` load function | Looks clean, runs early | Tauri API not available; invoke silently fails | Never |
| Block entire app startup on health check result | Forces user to fix config before using app | Users with broken engines can't reach Play tab to use cached/last-good audio | Never for tray apps — use non-blocking notice |
| Collapse all startup `onMount` logic into one block | Fewer lifecycle hooks | Single failure aborts all setup (listeners + appearance + health check all fail together) | Only acceptable if failures are completely independent |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Tauri IPC — new commands | Adding `#[tauri::command]` without updating `generate_handler!` | Always update `generate_handler!` in the same commit as the new command |
| Tauri IPC — event listeners across routes | Re-registering `listen()` inside page components instead of layout | Register persistent listeners once in `+layout.svelte onMount`; page components should not call `listen()` for app-level events |
| SvelteKit beforeNavigate guard | Calling `beforeNavigate` inside a non-component context (store, service) | Must be called during component initialization inside a `+page.svelte` or `+layout.svelte` |
| Config save on Engine route | Engine route calls `set_config` with only partial TTS fields, discarding other config sections | Always pass the full `AppConfig` object to `set_config`; merge TTS changes into a complete config clone before invoking |
| `$app/state` `page` store | Using `page.url.pathname` before the router has initialized (during `+layout.ts` load) | Only read `page.url.pathname` inside component scripts or `onMount`; it is reactive and correct after hydration |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Health check blocks initial render | App shows blank window for 1-3 seconds on startup | Make health check non-blocking; render layout immediately, show check result async | Every startup — not a scale issue, a UX issue |
| `get_config` called redundantly on every route visit | Network-equivalent latency to `%APPDATA%` on every tab switch | Cache config in a module-level store after first load; invalidate only on explicit save | Noticeable if settings sections have many `onMount` calls |
| `loadConfig()` called both on Settings and Engine `onMount` with no shared cache | Two round-trips to Rust on every tab change | Shared config store with a "loaded" flag | Perceptible on spinning-disk machines |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Health check failure shown as modal blocking the app | User cannot access Play tab if engine is misconfigured, even to listen to history | Show health check result as a dismissible banner on the Engine tab or a toast; never block the Play tab |
| No unsaved-changes indicator when navigating away from Settings or Engine | User makes changes, clicks another tab, returns to find edits gone | Show a subtle "Unsaved changes" badge on the tab or use `beforeNavigate` to confirm discard |
| Engine tab shown in nav even when no engine is configured yet | User clicks Engine tab and sees a blank or error state with no guidance | If health check fails on startup, highlight the Engine tab (e.g., an orange dot) to draw attention without blocking the app |
| Settings page still showing TTS engine section after it moves to Engine route | Duplicate controls; saving from Settings silently overwrites Engine settings | Complete the settings split before shipping the Engine route |
| 3-tab navigation with no visual indicator of active route | Users lose their place, especially if they opened the window from the tray | The existing `page.url.pathname` active-class pattern in `app-header.svelte` must be extended to include the new Engine tab — do not leave it commented out |

---

## "Looks Done But Isn't" Checklist

- [ ] **Engine route added to nav:** The `navItems` array in `app-header.svelte` must include the Engine entry — the commented-out `batch` example shows how easily this is forgotten.
- [ ] **TTS settings removed from Settings:** Verify `TtsSettings` import is deleted from `settings/+page.svelte`, not just commented out.
- [ ] **All new IPC commands registered:** Run a search for `#[tauri::command]` in `src-tauri/src/commands/` and cross-check against the `generate_handler!` list in `main.rs`.
- [ ] **Health check tested with broken engine:** Simulate a missing binary or bad API key and verify the app still opens and reaches the Play tab.
- [ ] **Config round-trip verified:** Make changes on Engine tab, navigate to Settings, back to Engine — verify changes persist without an explicit save in between.
- [ ] **`beforeNavigate` guard active on Engine tab:** Verify that unsaved engine credentials trigger a confirmation before navigation discards them.
- [ ] **Synthesis listener not double-registered:** Open the app, navigate Play → Engine → Settings → Play and confirm `synthesis-state-change` events do not fire twice.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| TTS config duplicated across two routes | MEDIUM | Hard-delete TTS section from Settings; consolidate into Engine route component; re-test config save/load round-trip |
| Listeners double-registered | LOW | Add guard variable (`if (unlistenSpeak) return`) and unlisten before re-registering; add dev-mode console.count probe |
| New IPC command unreachable | LOW | Add command name to `generate_handler!` list; rebuild; no frontend changes needed |
| Config state lost on navigation | MEDIUM | Extract `localConfig` draft into a `.svelte.ts` module-level store; update both route components to read/write from store instead of local `$state` |
| Health check blocking startup | LOW | Move `invoke` call into `onMount`; change result handling from blocking to async notification |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Unsaved settings state lost on navigation | Settings split phase (before Engine route ships) | Navigate Play → Settings → Engine → Settings; unsaved changes must survive the round-trip |
| Event listeners double-registered in layout | Navigation / layout phase | Navigate all three routes; confirm `speak-request` fires exactly once per clipboard copy |
| Startup health check race condition | Health check implementation phase | Kill the TTS binary; restart app; verify app opens without hanging and Play tab is reachable |
| New IPC commands not in `generate_handler!` | Engine route implementation phase | Frontend smoke test calls each new command and asserts no IPC rejection |
| TTS config duplicated across routes | Settings split phase | Grep for TTS field references in `src/routes/settings/`; must return zero results for moved fields |
| Engine tab missing from navigation | Navigation phase | Visual smoke test: all three tabs render with correct active state |

---

## Sources

- [Tauri v2 — Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/) — `generate_handler!` requirement (HIGH confidence)
- [Tauri v2 — Calling the Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/) — event listener lifecycle (HIGH confidence)
- [SvelteKit — State Management](https://svelte.dev/docs/kit/state-management) — per-route vs layout state (HIGH confidence)
- [SvelteKit — `$app/navigation`](https://svelte.dev/docs/kit/$app-navigation) — `beforeNavigate` guard pattern (HIGH confidence)
- [Mainmatter — Runes and Global State: do's and don'ts](https://mainmatter.com/blog/2025/03/11/global-state-in-svelte-5/) — module-level singleton pitfalls (HIGH confidence)
- [GitHub tauri-apps/tauri #7546 — invoke on first load doesn't work](https://github.com/tauri-apps/tauri/issues/7546) — startup race condition (MEDIUM confidence, confirmed pattern)
- [GitHub tauri-apps/tauri #12338 — crash on navigation with pending invoke](https://github.com/tauri-apps/tauri/issues/12338) — async invoke during navigation (MEDIUM confidence)
- [GitHub sveltejs/svelte #10176 — SvelteKit navigation breaks with onMount return](https://github.com/sveltejs/svelte/issues/10176) — listener lifecycle edge case (MEDIUM confidence)
- [Loopwerk — Refactoring Svelte stores to $state runes](https://www.loopwerk.io/articles/2025/svelte-5-stores/) — migration patterns (MEDIUM confidence)
- Codebase analysis: `src/routes/+layout.svelte`, `src/routes/settings/+page.svelte`, `src-tauri/src/commands/config.rs`, `src-tauri/src/main.rs` — direct inspection (HIGH confidence)

---
*Pitfalls research for: CopySpeak — multi-route navigation and startup onboarding (Tauri v2 + Svelte 5 + SvelteKit)*
*Researched: 2026-03-04*
