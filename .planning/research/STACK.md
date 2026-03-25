# Stack Research

**Domain:** Tauri v2 + SvelteKit desktop app — multi-route tab navigation
**Researched:** 2026-03-04
**Confidence:** HIGH (based on direct codebase inspection + framework documentation patterns)

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| SvelteKit file-system router | @sveltejs/kit ^2.50.2 | Route-based page ownership | Already in use; SPA mode (`ssr: false`) is the correct Tauri setup; each route gets its own `+page.svelte` with clean lifecycle isolation |
| `$app/state` `page` rune | SvelteKit ^2.x | Active-route detection | The Svelte 5-native way to read `page.url.pathname`; already used in `app-header.svelte`; reactive without stores |
| `<a href="...">` navigation links | SvelteKit built-in | Tab navigation triggers | SvelteKit's client-side router intercepts `<a>` tags automatically in SPA mode; no `goto()` needed for simple tab clicks |
| Tailwind CSS v4 | ^4.1.18 | Tab styling (active/inactive states) | Already in use; active state via conditional class binding on `isActive` is already the pattern in `app-header.svelte` |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@lucide/svelte` | ^0.561.0 | Tab icons | Already in use for Play/Settings icons; add a third icon (e.g., `Cpu` or `Mic`) for the Engine tab |
| shadcn-svelte `Tabs` (existing) | local | In-page sectioned content | Use for sub-sections WITHIN a route (e.g., Engine page sub-tabs for backend types); do NOT use for top-level navigation — it has no URL awareness |
| `goto()` from `$app/navigation` | SvelteKit built-in | Programmatic navigation | Use only for the onboarding redirect on startup health check fail; not for normal tab clicks |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| SvelteKit file-system router | Route ownership | Add `/engine/+page.svelte` alongside existing `/settings/+page.svelte` |
| Tauri `app-handle` | Window visibility | If opening the window on startup for onboarding, use Tauri's `show()` command; not a JS concern |

## Installation

No new packages required. All navigation primitives are built into the existing stack.

```bash
# No new dependencies needed
# New route: create src/routes/engine/+page.svelte
# Update app-header.svelte navItems array to add Engine entry
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| File-system routes (`/engine/+page.svelte`) | Single-page tab switching with shadcn Tabs + conditional rendering | Never for this app — file-system routes give each page independent `onMount`/`onDestroy` lifecycle, clean URL identity, and forward/back support. The shadcn Tabs component is stateful but not URL-aware, so the active tab resets on window focus |
| `page.url.pathname` from `$app/state` | `page` store from `$app/stores` | Never in Svelte 5 — `$app/stores` is the Svelte 4 pattern; `$app/state` is the Svelte 5 rune-compatible API already used in this codebase |
| `<a href="/engine">` link navigation | `goto('/engine')` on button click | Only use `goto()` for programmatic redirects (e.g., health check failure → redirect to Engine); for user-visible nav tabs, use `<a>` so browser semantics and accessibility work correctly |
| Extending `app-header.svelte` navItems array | Adding a second nav bar or bottom tab bar | Only add a second nav structure if the app grows to 5+ top-level routes; 3 tabs fit comfortably in the existing header nav |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| shadcn-svelte `Tabs` for top-level navigation | The existing `Tabs` component (see `tabs.svelte`) uses a custom context with `getValue`/`setValue` — it has no concept of URL state. Switching tabs does not change the URL, so the Engine page cannot be linked to from the system tray "Settings" menu item, and the active tab resets every time the window is shown/hidden (common in tray apps) | SvelteKit `<a>` + file-system routes as already implemented |
| `$app/stores` `page` store | Svelte 4 pattern; requires `$page` store subscription syntax; this codebase uses `$app/state` which is the Svelte 5 runes-compatible API | `import { page } from "$app/state"` (already in use) |
| React Router / TanStack Router | Not applicable — SvelteKit's router is built-in and already handles SPA routing for Tauri correctly | SvelteKit file-system router |
| Separate Tauri windows per route | Opening a new `WebviewWindow` per section creates separate JS contexts, separate event listener registrations, and breaks shared stores | Single window with SvelteKit client-side routing |
| Hash-based routing (`/#/engine`) | Technically works in static SPA mode but produces ugly URLs, breaks SvelteKit's `page.url.pathname` checks, and is unnecessary since SvelteKit adapter-static with `fallback: 'index.html'` handles client-side routing correctly | SvelteKit's default pathname-based routing |

## Stack Patterns by Variant

**For the Engine tab (new top-level route):**
- Create `src/routes/engine/+page.svelte`
- Add `{ id: "engine", label: "Engine", href: "/engine", icon: Cpu }` to `navItems` in `app-header.svelte`
- Active state detection already works via `page.url.pathname.startsWith(item.href)` — no changes to the detection logic needed
- Confidence: HIGH — this is identical to how `/settings` was added

**For the startup health check / onboarding flow:**
- Run the health check in `+layout.svelte` `onMount` (it already loads config there)
- Use `goto('/engine')` with `replaceState: true` if the engine is unconfigured, so back-button doesn't loop
- Alternatively: show a `Dialog`/`Alert` overlay rather than redirecting — avoids navigation side effects during init
- Confidence: MEDIUM — depends on UX decision; both patterns work with the current stack

**For Engine sub-sections (backend selector, credentials, health check UI, voice picker):**
- Use the existing shadcn-svelte `Tabs` component WITHIN the Engine page for switching between sub-sections (e.g., "CLI", "ElevenLabs", "OpenAI", "HTTP")
- This is the correct use case for the Tabs component: in-page sectioning where URL identity is not needed
- Confidence: HIGH

**For the Settings page refactor (moving TTS engine settings to Engine route):**
- Remove the `TtsSettings` import and section from `settings/+page.svelte`
- No routing changes required — the Engine page is a new route, not a renamed existing route
- Confidence: HIGH

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `@sveltejs/kit ^2.50.2` | `svelte ^5.49.2` | SvelteKit 2.x is the Svelte 5-compatible major; `$app/state` (rune-based `page`) is available from SvelteKit 2.x |
| `$app/state` `page` | SvelteKit 2.x+ only | Do not use `$app/stores` — it is the Svelte 4 API and will produce deprecation warnings with Svelte 5 |
| shadcn-svelte local `Tabs` | `bits-ui ^2.14.4` | The existing Tabs implementation is a custom build (not delegating to bits-ui Tabs); it uses Svelte 5 `setContext`/`getContext` pattern correctly |
| `@tauri-apps/api ^2` | Tauri v2 only | The IPC and event listener patterns in `+layout.svelte` are Tauri v2 specific; do not mix with Tauri v1 APIs |

## Key Architectural Decision: Why URL-based Routes, Not Tab State

In a Tauri tray app, the window is repeatedly shown and hidden. Component state is NOT preserved across hide/show cycles in all Tauri configurations. If navigation is implemented as in-memory tab state (e.g., a `$state` variable tracking which tab is active), the selected tab resets to the default every time the window is focused from the tray.

File-system routes with SvelteKit's router persist across window show/hide because the URL is preserved by the browser engine (WebView2 on Windows). The user returns to the same route they left on. This is the correct pattern for Tauri apps with tray-based window management.

Confidence: HIGH (observed pattern in Tauri community; consistent with how WebView2 handles navigation state).

## Sources

- Direct codebase inspection: `src/lib/components/layout/app-header.svelte` — existing nav pattern using `$app/state` page + `<a>` links (HIGH confidence)
- Direct codebase inspection: `src/routes/+layout.ts` — confirms `ssr: false`, `prerender: true` SPA mode (HIGH confidence)
- Direct codebase inspection: `src/lib/components/ui/tabs/tabs.svelte` — confirms local Tabs component has no URL awareness (HIGH confidence)
- SvelteKit docs pattern: `$app/state` vs `$app/stores` — Svelte 5 rune-compatible API (HIGH confidence, current as of SvelteKit 2.x)
- Tauri v2 SvelteKit guide: https://v2.tauri.app/start/frontend/sveltekit/ — confirms adapter-static SPA mode as the standard Tauri+SvelteKit setup (HIGH confidence)

---
*Stack research for: CopySpeak multi-route navigation (Play / Engine / Settings)*
*Researched: 2026-03-04*
