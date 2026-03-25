# Project Research Summary

**Project:** CopySpeak — TTS Engine Configuration Page + 3-Tab Navigation
**Domain:** Tauri v2 + SvelteKit desktop app — multi-route navigation with engine config extraction
**Researched:** 2026-03-04
**Confidence:** HIGH

## Executive Summary

CopySpeak is a Tauri v2 + SvelteKit desktop tray app that reads clipboard text and speaks it via configurable TTS backends (CLI, ElevenLabs, OpenAI, HTTP). This milestone introduces a dedicated Engine configuration route extracted from the Settings page, adds a third tab to the navigation (Play / Engine / Settings), and implements a non-blocking startup health check that routes users to the Engine page when their TTS backend is broken or unconfigured. The full implementation requires zero new Rust IPC commands — all backend infrastructure (`test_tts_engine`, `list_elevenlabs_voices`, `get_config`, `set_config`) is already in place. This is primarily a frontend restructuring task with well-understood patterns.

The recommended approach is to use SvelteKit's existing file-system router to add an `/engine` route alongside the existing `/` and `/settings` routes. Navigation uses `<a href="/engine">` links and `$app/state page` for active-route detection — identical to the existing pattern in `app-header.svelte`. The `TtsSettings` component is moved (not copied) from Settings to the Engine route, and the startup health check is wired into `+layout.svelte`'s `onMount`. No new packages, no new routing library, and no Rust changes are required for the core milestone features.

The primary risk is state management across routes: config edits made on the Engine page or Settings page can be silently discarded when the user navigates to another tab. This must be addressed before the Engine route ships, either via `beforeNavigate` guards or a module-level config draft store. A secondary risk is the startup health check creating a race condition if placed in a `+layout.ts` load function instead of `onMount` — the Tauri IPC bridge is not available at SSG/prerender time.

## Key Findings

### Recommended Stack

No new dependencies are required. The existing SvelteKit 2.x file-system router, `$app/state page` rune, Tailwind CSS v4, and Lucide icons are the complete toolkit for this milestone. The critical architectural insight is that in a Tauri tray app, URL-based routing via SvelteKit file-system routes is mandatory — not optional. In-memory tab state (a `$state` variable tracking the active tab) resets every time the window is shown from the system tray because component state is not preserved across WebView2 hide/show cycles. File-system routes persist because WebView2 preserves the URL.

**Core technologies:**
- SvelteKit file-system router (`@sveltejs/kit ^2.50.2`): Route-based page ownership — each tab gets its own `+page.svelte` with independent lifecycle and a persistent URL
- `$app/state page` rune: Active-route detection — already used in `app-header.svelte`; the Svelte 5-native API (not `$app/stores` which is Svelte 4)
- `<a href="/engine">` nav links: Tab navigation — SvelteKit intercepts these automatically in SPA mode; no `goto()` needed for user-visible tabs
- shadcn-svelte `Tabs` (local): In-page sub-sectioning only — correct for Engine sub-tabs (CLI / ElevenLabs / OpenAI / HTTP) but must NOT be used for top-level navigation (no URL awareness)
- `@lucide/svelte Cpu` icon: Engine tab icon — consistent with existing Play and Settings icon pattern

### Expected Features

The Engine page must reach feature parity with the current TTS section in Settings plus add health check UI. Users of audio configuration tools (Home Assistant TTS, AllTalk TTS, VS Code language servers) universally expect backend selection, credential entry, a "Test" button with specific error messages, and inline setup help when a dependency is missing.

**Must have (table stakes — P1 for this milestone):**
- 3-tab navigation (Play / Engine / Settings) — Engine page needs a route before any other feature can exist
- Backend selector (CLI / ElevenLabs / OpenAI / HTTP) moved from Settings — the page's reason for being
- Per-backend credential and command fields moved from Settings — users expect to set what they set before
- Engine preset selector (Piper / kokoro / Edge TTS / etc.) — exposes already-existing preset system
- Live "Test Engine" button with specific error diagnosis — generic "failed" is unacceptable; `TtsError` variants map directly
- Non-blocking startup health check with banner routing to Engine page — the core milestone deliverable
- Inline install instructions per preset (markdown snippet + copy button) — replaces backlogged Quick Install Guide

**Should have (differentiators — P2, next milestone):**
- "Speak test phrase" audio preview button — end-to-end synthesis + playback confirmation
- Voice list fetch + dropdown for ElevenLabs — eliminates opaque voice ID copy-paste; requires new Rust surface
- Diagnostic detail panel (collapsible stderr / HTTP error body) — power-user debug without log diving

**Defer (v0.3+):**
- Named multi-engine preset manager — explicitly out of scope per PROJECT.md
- Auto-detect python / python3 on Windows — quality-of-life improvement, not blocking
- Full engine installation wizard — do not build; inline guidance covers 80% of value at 20% the cost

### Architecture Approach

The architecture is a single Tauri window with a SvelteKit SPA shell. `+layout.svelte` owns the persistent `AppHeader` and global event listeners (`speak-request`, `synthesis-state-change`). Route pages (`/`, `/engine`, `/settings`) are mounted into the layout's `{@render children()}` slot — each page gets a fresh mount on navigation. Rust's `Mutex<AppConfig>` is the single source of truth for all configuration; pages load via `invoke("get_config")` in `onMount` and write via `invoke("set_config")`. No shared frontend config store is needed or wanted.

**Major components:**
1. `+layout.svelte` — Global shell: Tauri event listeners, AppHeader, AppFooter; startup health check `onMount` block
2. `app-header.svelte` — Tab navigation: extend `navItems` array to add Engine entry with `Cpu` icon
3. `/engine/+page.svelte` (new) — Engine route: backend selector, credential forms, health check button, preset guide; follows identical config load/save pattern as Settings
4. `/settings/+page.svelte` (trimmed) — Settings route: remove TTS section entirely; shrinks `settingsCategories` from 6 to 5
5. `TtsSettings` component — Move to `lib/components/engine/`; do not duplicate; single owner of TTS config UI
6. Rust backend (no changes) — `test_tts_engine`, `list_elevenlabs_voices`, `get_config`, `set_config` already implemented

### Critical Pitfalls

1. **Config state lost on tab navigation** — Each `+page.svelte` remounts on navigation and calls `get_config` fresh, discarding unsaved edits. Prevention: add `beforeNavigate` guard in both Settings and Engine pages to warn before discarding, OR extract `localConfig` draft into a module-level `.svelte.ts` singleton store. Must be resolved before Engine route ships.

2. **Event listeners double-registered in layout** — Adding startup health check logic to `+layout.svelte onMount` risks ordering conflicts with existing `speak-request` and `synthesis-state-change` listeners. Prevention: keep health check as a separate `onMount` call (not chained); store unlisten handles as module-level variables with `if (unlistenX) return` guards; probe with `console.count` during development.

3. **Startup health check race condition** — Placing `invoke()` in `+layout.ts` load functions causes silent failure because the Tauri IPC bridge is not available at prerender time. Prevention: health check must live exclusively in `onMount` inside `+layout.svelte`. Never `await` it before rendering UI — show result as a non-blocking banner.

4. **TTS config duplicated across Settings and Engine routes** — If `TtsSettings` is copied rather than moved, two components will write to `AppConfig.tts.*` independently. Prevention: perform a hard delete of TTS section from `settings/+page.svelte` in the same PR that introduces the Engine route. Verify with `grep` audit: no `tts.` references should remain in `src/routes/settings/`.

5. **New IPC commands missing from `generate_handler!`** — Any new `#[tauri::command]` function that is not listed in `main.rs`'s `generate_handler!` compiles correctly but silently rejects all frontend calls. Prevention: for each new command, a three-step checklist: (1) `#[tauri::command]` annotation, (2) re-export in `commands/mod.rs`, (3) entry in `generate_handler!`. Note: no new commands are required for the core milestone — this pitfall applies if P2 features are introduced.

## Implications for Roadmap

Based on research, the architecture has clear dependency ordering. Each phase delivers something independently testable before the next begins.

### Phase 1: Navigation Shell

**Rationale:** The Engine page route must exist before any other feature can be built on it. This is a trivial change (3 lines in `app-header.svelte` + 1 new file) but it is the prerequisite for everything else. Doing it first also validates that 3-tab navigation works without breaking the existing Play and Settings tabs.

**Delivers:** 3-tab navigation (Play / Engine / Settings) with correct active-state highlighting; Engine route shows placeholder content.

**Addresses:** "3-tab navigation" P1 feature from FEATURES.md; validates SvelteKit file-system router extends cleanly.

**Avoids:** Pitfall 5 (Engine tab missing from navigation — the "looks done but isn't" failure mode).

**Research flag:** Standard pattern — no phase research needed. Identical to how `/settings` was originally added.

### Phase 2: Engine Page Core — Config Forms

**Rationale:** Move (not copy) the `TtsSettings` component and all associated state management to the Engine route. This must happen as an atomic operation — Engine route added, TTS UI moved, Settings trimmed, save/load round-trip tested — to prevent the duplication pitfall.

**Delivers:** Fully functional Engine page with backend selector, credential fields, preset selector, voice/speed fields, and save bar. Settings page trimmed to non-TTS sections only.

**Uses:** Existing `get_config` / `set_config` IPC; shadcn-svelte in-page `Tabs` for CLI/ElevenLabs/OpenAI/HTTP sub-tabs; existing `TtsSettings` component refactored into `lib/components/engine/`.

**Implements:** Engine route component boundary; Settings route trimming.

**Avoids:** Pitfall 4 (TTS config duplicated); Pitfall 1 (add `beforeNavigate` guard in this phase for both Settings and Engine pages).

**Research flag:** Standard pattern — follows established Settings page pattern exactly. No phase research needed.

### Phase 3: Health Check UI

**Rationale:** The `test_tts_engine` Rust command and `TtsHealthResult` struct already exist. This phase wires them to a frontend button and renders per-`error_type` diagnostic messages. Depends on Phase 2 (Engine page must exist to host the button).

**Delivers:** "Test Engine" button on Engine page with specific error diagnosis messages per `TtsError` variant (not_found, api_key_missing, auth_failed, etc.). Users can verify their engine configuration interactively.

**Implements:** `engine-health-check.svelte` component; `TtsHealthResult` → user guidance text mapping.

**Avoids:** Pitfall 3 (health check must be called from `onMount`, not from a load function — same principle applies to the button's invoke call).

**Research flag:** Standard pattern — IPC command and return type already defined. No phase research needed.

### Phase 4: Settings Cleanup

**Rationale:** Must follow Phase 2 (Engine page must be fully functional before Settings TTS section is removed) but can be done in the same PR as Phase 2 if confidence is high. Separate phase makes the cleanup reviewable independently.

**Delivers:** Settings page with 5 categories (no TTS); verified that no `TtsConfig` fields remain referenced in Settings routes.

**Avoids:** Pitfall 4 (TTS duplication) — the `grep` audit step belongs here as a merge gate.

**Research flag:** Standard pattern — deletion task with clear verification criteria.

### Phase 5: Startup Health Check

**Rationale:** Depends on Phase 3 (health check UI must exist on Engine page before layout can route there). This phase wires the layout-level startup check that fires once on app open.

**Delivers:** Non-blocking startup health check in `+layout.svelte onMount`; on failure, `goto("/engine?setup=true")`; Engine page reads query param and renders setup-mode banner. App is never blocked — Play tab always reachable.

**Implements:** Pattern 3 from ARCHITECTURE.md (startup health check via layout onMount); `beforeNavigate` guard on Engine page.

**Avoids:** Pitfall 2 (separate `onMount` block, not chained with existing listeners); Pitfall 3 (placed in `onMount`, never in load function); Anti-pattern 3 (non-blocking, no modal gating).

**Research flag:** Medium complexity — the `goto()` + query param handshake pattern needs careful testing in Tauri's WebView2 context (not just browser dev). Consider smoke-testing the startup redirect with a broken engine before merging.

### Phase 6: Voice Browser Enhancement (P2)

**Rationale:** ElevenLabs voice list fetch partially exists in `TtsSettings` already. This phase completes it and ensures the fetched dropdown replaces the static text field only for cloud backends. Depends on Phase 2 (Engine page must exist).

**Delivers:** ElevenLabs voice dropdown populated from API when API key is valid; Piper static voice list with download links.

**Avoids:** Feature conflict noted in FEATURES.md: voice list fetch must replace (not coexist with) the static voice name field for cloud backends.

**Research flag:** Needs attention — ElevenLabs API call requires valid key before fetching. Error handling for expired keys or rate limits should be specified before implementation.

### Phase Ordering Rationale

- Phase 1 before all others: The route must exist before any UI can be built on it.
- Phase 2 before Phase 3: Health check button lives on the Engine page; the page must be functional before the button is meaningful.
- Phase 2 and Phase 4 are tightly coupled: The Settings cleanup is the other half of the Engine move; they should ship in the same PR or back-to-back.
- Phase 3 before Phase 5: The startup check routes to `/engine?setup=true`; the Engine page must already show useful diagnostic content, not a placeholder.
- Phase 6 is independent of Phases 4 and 5: Can be parallelized with cleanup tasks if multiple developers are available, or deferred to the next milestone.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 5 (Startup Health Check):** The `goto()` + URL query param handshake pattern should be smoke-tested in Tauri's WebView2 context before finalizing the approach. An alternative (toast with action button rather than redirect) may be lower risk if the redirect creates navigation-loop edge cases.
- **Phase 6 (Voice Browser):** ElevenLabs API error handling (rate limits, expired keys, network failure mid-fetch) needs explicit specification. The fetched dropdown UX when voices are loading or fail to load is underspecified in current research.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Navigation Shell):** Identical to how `/settings` was added. Zero unknowns.
- **Phase 2 (Engine Page Core):** Copy the Settings page load/save/save-bar pattern exactly.
- **Phase 3 (Health Check UI):** IPC command and return type fully defined; mapping `error_type` to strings is pure UI work.
- **Phase 4 (Settings Cleanup):** Deletion with grep verification. No architectural decisions.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Direct codebase inspection; no new dependencies; existing patterns confirmed in source |
| Features | MEDIUM | Analogous products analyzed (Home Assistant, AllTalk TTS, VS Code); no direct user research; P1 features are grounded in existing codebase scope; P2+ features have more uncertainty |
| Architecture | HIGH | All components exist; all IPC commands confirmed in Rust source; patterns drawn from Settings page which already works |
| Pitfalls | HIGH | Backed by official Tauri v2 docs, SvelteKit docs, and confirmed GitHub issues; codebase patterns validated against pitfall checklist |

**Overall confidence:** HIGH

### Gaps to Address

- **Unsaved-changes UX decision:** Research identified two valid approaches (module-level draft store vs `beforeNavigate` warning). The roadmap should pick one before Phase 2 begins — mixing both adds complexity. Recommendation: `beforeNavigate` confirmation is simpler and matches user expectation for a settings-style page.

- **Startup health check UX finalization:** Research identified `goto("/engine?setup=true")` as the preferred approach but noted a toast-with-action-button alternative. If the redirect produces edge cases in Tauri WebView2 (e.g., back-button issues), the toast pattern is a lower-risk fallback. Validate in Phase 5 spike before committing.

- **Engine page component decomposition depth:** Research recommends either reusing `TtsSettings` as-is or decomposing it into `engine/` sub-components. The right choice depends on how much the Engine page UI diverges from the Settings-era TTS form. Leave this as an implementation decision for Phase 2 — start with reuse, decompose if the component requires too many props or conditional branches.

- **`beforeNavigate` guard scope:** Pitfalls research requires an unsaved-changes guard on both Settings and Engine pages. Confirm during Phase 2 whether `beforeNavigate` called in `+page.svelte` is sufficient or whether it needs to be in `+layout.svelte` for Tauri window hide/show events (which may not trigger SvelteKit's navigation lifecycle).

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection: `src/routes/+layout.svelte`, `src/routes/settings/+page.svelte`, `src/lib/components/layout/app-header.svelte`, `src/lib/components/settings/tts-settings.svelte`, `src-tauri/src/commands/tts.rs`, `src/lib/types.ts`, `src/routes/+layout.ts`
- Tauri v2 official docs: https://v2.tauri.app/develop/calling-rust/ — `generate_handler!` requirement
- Tauri v2 official docs: https://v2.tauri.app/develop/calling-frontend/ — event listener lifecycle
- SvelteKit docs: https://svelte.dev/docs/kit/state-management — per-route vs layout state
- SvelteKit docs: https://svelte.dev/docs/kit/$app-navigation — `beforeNavigate` guard pattern
- Tauri v2 SvelteKit guide: https://v2.tauri.app/start/frontend/sveltekit/ — adapter-static SPA mode

### Secondary (MEDIUM confidence)
- GitHub tauri-apps/tauri #7546 — invoke on first load race condition (confirmed pattern)
- GitHub tauri-apps/tauri #12338 — async invoke during navigation crash
- GitHub sveltejs/svelte #10176 — SvelteKit navigation + onMount return lifecycle edge case
- Home Assistant TTS integration docs — engine selector and health check UX patterns
- AllTalk TTS V2 QuickStart Guide — engine setup and inline install guidance patterns
- VS Code language server extension guide — inline error handling and non-blocking status patterns
- Carbon Design System notification pattern — banner UX for non-blocking warnings
- Mainmatter blog: Runes and Global State do's and don'ts — module-level singleton patterns

### Tertiary (LOW confidence)
- Loopwerk: Refactoring Svelte stores to $state runes — migration pattern reference
- LogRocket: Creating setup wizard — when not to build a wizard (validated anti-feature decision)

---
*Research completed: 2026-03-04*
*Ready for roadmap: yes*
