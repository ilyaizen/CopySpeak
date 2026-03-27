# Phase 1: Navigation Shell - Research

**Researched:** 2026-03-04
**Domain:** SvelteKit file-based routing + Svelte 5 reactive navigation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None explicitly locked — user delegated all choices to Claude.

### Claude's Discretion
- **Tab order**: Play | Engine | Settings (left to right)
- **Engine stub content**: Minimal placeholder (page title + correct structure — whichever is cleaner)
- **Engine tab icon**: Claude selects the most appropriate Lucide icon (e.g., `Cpu`, `Zap`, `Bot`)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| NAV-01 | User can navigate between Play, Engine, and Settings tabs via header | Confirmed: add one object to `navItems` array in `app-header.svelte`; create `src/routes/engine/+page.svelte` |
| NAV-02 | Active tab is visually highlighted | Confirmed: existing `isActive` logic uses `page.url.pathname.startsWith(item.href)` — zero changes needed for `/engine` route |
| NAV-03 | Tab state is preserved via URL (no reset on tray open/close) | Confirmed: SvelteKit URL-driven routing inherently preserves state; no special handling required |
</phase_requirements>

---

## Summary

This phase is a minimal, low-risk frontend change. The entire implementation fits in two file edits: one object added to `navItems` in `app-header.svelte`, and one new file `src/routes/engine/+page.svelte` created from scratch. No Rust, no Tauri IPC, no new dependencies.

The existing codebase already provides all patterns needed. The commented-out `Batch` nav item in `app-header.svelte` is a direct template for the Engine entry. SvelteKit's file-based routing means the `/engine` route activates the moment the file exists. The active-tab detection logic (`page.url.pathname.startsWith(item.href)`) handles `/engine` correctly with zero modifications.

The only genuine decision is icon selection. `Cpu` from `@lucide/svelte` is the strongest match — it visually communicates "engine / processing unit" without ambiguity. `Bot` skews toward AI assistant UX. `Zap` skews toward speed/performance rather than engine configuration.

**Primary recommendation:** Add `Cpu` icon entry to `navItems` between Play and Settings; create a minimal stub page with the brutalist card aesthetic consistent with the rest of the UI.

---

## Standard Stack

### Core (already installed — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@sveltejs/kit` | ^2.50.2 | File-based routing, `page` store | Already in project, SvelteKit's router handles all nav state |
| `@lucide/svelte` | ^0.561.0 | Tab icons | Already imported in `app-header.svelte` for Play/Settings |
| Svelte 5 | ^5.49.2 | Reactive UI with runes | Project standard; `$app/state` page store is Svelte 5 compatible |

### No New Installations Required

```bash
# Nothing to install — all dependencies already present
```

---

## Architecture Patterns

### Recommended File Changes

```
src/
├── lib/
│   └── components/
│       └── layout/
│           └── app-header.svelte    # EDIT: add Engine to navItems
└── routes/
    ├── engine/
    │   └── +page.svelte             # CREATE: stub page
    ├── +layout.svelte               # NO CHANGE
    └── +page.svelte                 # NO CHANGE
```

### Pattern 1: Adding a Nav Item (existing pattern, zero creativity required)

**What:** Insert one object into the `navItems` array. The `{#each}` loop and `isActive` logic handle the rest automatically.

**When to use:** Whenever a new top-level route needs a header tab.

**Example (from existing codebase + Engine addition):**

```typescript
// Source: src/lib/components/layout/app-header.svelte
import { page } from "$app/state";
import { Play, Settings, Cpu } from "@lucide/svelte";

const navItems = [
  {
    id: "generate",
    label: "Play",
    href: "/",
    icon: Play,
  },
  {
    id: "engine",
    label: "Engine",
    href: "/engine",
    icon: Cpu,
  },
  {
    id: "settings",
    label: "Settings",
    href: "/settings",
    icon: Settings,
  },
];
```

The active detection already handles this correctly:
```typescript
// item.href === "/" ? exact match : startsWith — /engine matches /engine routes
const isActive =
  item.href === "/"
    ? page.url.pathname === "/"
    : page.url.pathname.startsWith(item.href);
```

### Pattern 2: SvelteKit File-Based Route Creation

**What:** Create `src/routes/engine/+page.svelte`. The file's existence is sufficient to register the route. No router configuration needed.

**When to use:** Every new top-level page in SvelteKit.

**Example (stub page matching brutalist aesthetic):**

```svelte
<!-- Source: pattern from src/routes/+page.svelte structure -->
<svelte:head>
  <title>Engine - CopySpeak</title>
</svelte:head>

<div class="w-full max-w-6xl mx-auto">
  <div class="border border-border rounded-lg overflow-hidden">
    <div class="p-4 bg-muted/50 border-b border-border">
      <h2 class="text-lg font-semibold font-mono">TTS Engine</h2>
      <p class="text-sm text-muted-foreground">
        Configure your text-to-speech engine
      </p>
    </div>
    <div class="p-6 text-muted-foreground text-sm">
      Engine configuration coming soon.
    </div>
  </div>
</div>
```

### Anti-Patterns to Avoid

- **Modifying the layout file:** `+layout.svelte` does not need changes. `AppHeader` already renders at the top of every page.
- **Adding a `+page.ts` load function:** Unnecessary for a stub page with no data fetching.
- **Using `goto()` programmatically:** All navigation uses `<a href>` links, consistent with the existing pattern. `goto()` would break browser back/forward behavior.
- **Changing isActive logic:** The existing `startsWith` check correctly handles `/engine` and any future `/engine/*` sub-routes. Do not add a special case.
- **Adding a `data-sveltekit-prefetch` or `data-sveltekit-reload` attribute:** Default behavior is correct.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Active tab state | Custom store or class toggling | `page.url.pathname` from `$app/state` | SvelteKit's reactive page store is always in sync with the URL; manual state tracking diverges on back/forward navigation |
| Tab routing | Manual click handlers + conditional rendering | `<a href="/engine">` + SvelteKit file routing | File-based routing handles history, back/forward, deep links automatically |
| Icon imports | Custom SVG inline | `@lucide/svelte` (already installed) | Consistent sizing, accessibility props, tree-shaken |

**Key insight:** The entire feature is already built — the loop, the active detection, the styling. Adding Engine is filling in one array slot and creating one file.

---

## Common Pitfalls

### Pitfall 1: Icon Import Path
**What goes wrong:** Using the old `lucide-svelte` package name instead of `@lucide/svelte`.
**Why it happens:** Many tutorials and older examples use `lucide-svelte`; the project uses the scoped `@lucide/svelte` package.
**How to avoid:** Copy the import line from the existing `app-header.svelte`: `import { Play, Settings } from "@lucide/svelte";` — add `Cpu` to the same destructure.
**Warning signs:** TypeScript error "Cannot find module 'lucide-svelte'".

### Pitfall 2: Route Collision with Root Active State
**What goes wrong:** `/engine` route could theoretically match root `/` if logic were reversed, but it won't — the existing code guards root with an exact match (`=== "/"`).
**Why it happens:** Confusion about `startsWith` — `"/engine".startsWith("/")` is true, so root gets special treatment.
**How to avoid:** No change needed. The existing guard is already correct. Do not alter the `isActive` expression.
**Warning signs:** Play tab stays highlighted when on Engine page (would mean the guard was removed).

### Pitfall 3: Missing `<svelte:head>` title
**What goes wrong:** Browser tab shows "CopySpeak" but no page-specific title, making it harder to distinguish routes during development.
**Why it happens:** Forgetting `<svelte:head>` on the stub page.
**How to avoid:** Include `<title>Engine - CopySpeak</title>` in a `<svelte:head>` block (see existing `+page.svelte` for pattern).

---

## Code Examples

Verified patterns from existing source files:

### Complete nav item shape (from `app-header.svelte` lines 5-24)
```typescript
// Source: /src/lib/components/layout/app-header.svelte
const navItems = [
  { id: "generate", label: "Play",     href: "/",        icon: Play     },
  { id: "engine",   label: "Engine",   href: "/engine",  icon: Cpu      }, // NEW
  { id: "settings", label: "Settings", href: "/settings", icon: Settings },
];
```

### Active tab rendering (from `app-header.svelte` lines 49-68)
```svelte
<!-- Source: /src/lib/components/layout/app-header.svelte -->
{#each navItems as item}
  {@const isActive =
    item.href === "/"
      ? page.url.pathname === "/"
      : page.url.pathname.startsWith(item.href)}
  {@const Icon = item.icon}
  <a
    href={item.href}
    data-testid="nav-{item.id}"
    class="inline-flex items-center ... {isActive
      ? 'bg-muted text-foreground'
      : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
    aria-current={isActive ? "page" : undefined}
  >
    <div class="flex items-center gap-2">
      <Icon size={14} />
      <span>{item.label}</span>
    </div>
  </a>
{/each}
```

### Settings stub section structure (from `settings/+page.svelte` lines 268-300)
```svelte
<!-- Source: /src/routes/settings/+page.svelte - use as aesthetic reference -->
<section class="scroll-mt-32">
  <div class="border border-border rounded-lg overflow-hidden">
    <div class="p-4 bg-muted/50 border-b border-border">
      <h2 class="text-lg font-semibold">Section Title</h2>
      <p class="text-sm text-muted-foreground">Description</p>
    </div>
    <div class="p-4">
      <!-- content -->
    </div>
  </div>
</section>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `$page` store from `$app/stores` | `page` from `$app/state` (Svelte 5 rune) | SvelteKit 2 + Svelte 5 | `page.url.pathname` is reactive without `$` prefix — already used in project |
| `on:click` event directive | `onclick` attribute | Svelte 5 | Project CLAUDE.md mandates `onclick` not `on:click` |

**Deprecated/outdated:**
- `import { page } from "$app/stores"` and `$page.url.pathname`: The project already uses the Svelte 5 `$app/state` form (`page.url.pathname` without `$` prefix). Do not revert to stores form.

---

## Open Questions

None. Phase scope is fully understood from existing source code inspection. No external research was needed — all patterns are directly readable from the codebase.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Vitest 4.0.18 |
| Config file | `/home/ubuntu/CopySpeak/vitest.config.ts` |
| Quick run command | `bun run test` |
| Full suite command | `bun run test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| NAV-01 | Engine tab renders in header and links to `/engine` | unit (component) | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ Wave 0 |
| NAV-02 | Active tab has `bg-muted text-foreground` class; inactive does not | unit (component) | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ Wave 0 |
| NAV-03 | Navigating to `/engine` sets `aria-current="page"` on Engine tab | unit (component) | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `bun run test`
- **Per wave merge:** `bun run test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/lib/components/layout/app-header.test.ts` — covers NAV-01, NAV-02, NAV-03
  - Mock `$app/state` page store with varying `pathname` values
  - Assert Engine `<a>` exists with correct `href="/engine"`
  - Assert `aria-current="page"` on Engine link when pathname is `/engine`
  - Assert `aria-current` absent on Play/Settings when on `/engine`

> Note: Testing SvelteKit components with `$app/state` requires mocking the `page` rune. The existing vitest setup uses jsdom but has no `$app/state` mock infrastructure yet. The test file should use `vi.mock('$app/state', ...)` to return a controlled `page` object.

---

## Sources

### Primary (HIGH confidence)
- Direct source code inspection: `/home/ubuntu/CopySpeak/src/lib/components/layout/app-header.svelte` — all patterns verified from live file
- Direct source code inspection: `/home/ubuntu/CopySpeak/src/routes/+page.svelte` and `settings/+page.svelte` — stub page aesthetic reference
- `/home/ubuntu/CopySpeak/vitest.config.ts` — test framework configuration verified

### Secondary (MEDIUM confidence)
- `@lucide/svelte` package.json version `^0.561.0` — `Cpu` icon confirmed available in Lucide icon set (stable, high availability in all recent versions)

### Tertiary (LOW confidence)
- None needed — research fully satisfied by codebase inspection

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — directly read from package.json and existing source
- Architecture: HIGH — patterns copied verbatim from existing working code
- Pitfalls: HIGH — derived from direct analysis of isActive logic and import paths in source

**Research date:** 2026-03-04
**Valid until:** 2026-09-04 (stable — SvelteKit routing and Lucide icons are stable APIs)
