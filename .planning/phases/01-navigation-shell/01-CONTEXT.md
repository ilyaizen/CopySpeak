# Phase 1: Navigation Shell - Context

**Gathered:** 2026-03-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a third "Engine" tab to the persistent header nav and register the `/engine` SvelteKit route. The Engine page shows a minimal stub — actual content is filled in Phase 2. Play and Settings tabs must continue working without regression.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
User chose not to discuss any specific areas — all implementation choices are delegated to Claude:
- **Tab order**: Play | Engine | Settings (left to right, most logical reading/flow order)
- **Engine stub content**: Minimal placeholder (page title + brief "coming soon" or empty shell with correct structure — whichever is cleaner)
- **Engine tab icon**: Claude selects the most appropriate Lucide icon (e.g., `Cpu`, `Zap`, `Bot`)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/lib/components/layout/app-header.svelte`: Has a `navItems` array — adding Engine is one object entry. A commented-out Batch item shows the exact pattern to follow.
- `@lucide/svelte`: Already imported for Play and Settings icons; Engine icon uses same import pattern.

### Established Patterns
- **Active-tab detection**: `page.url.pathname === "/"` for exact root match; `page.url.pathname.startsWith(item.href)` for all other routes. Adding `/engine` will highlight correctly with zero changes to detection logic.
- **Nav item shape**: `{ id, label, href, icon }` — icon is a Lucide component reference.
- **Active styles**: `bg-muted text-foreground`; inactive: `text-muted-foreground hover:bg-muted/50 hover:text-foreground`

### Integration Points
- New route: `src/routes/engine/+page.svelte` (SvelteKit file-based routing — just create the file)
- Layout wraps all routes via `src/routes/+layout.svelte` → `AppHeader` renders at top of every page
- No Rust/Tauri backend changes needed for Phase 1

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The brutalist aesthetic (hard edges, muted palette, mono fonts) is already established and should be consistent on the stub page.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-navigation-shell*
*Context gathered: 2026-03-04*
