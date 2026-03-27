# Phase 11: Layout Consolidation - Research

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 11 implements a compact 2-column layout for all settings rows and minifies redundant controls (Pagination, HUD) into single-select dropdowns.

**Scope:**

- SETT-05: All settings use 2-column grid layout (Label+InfoTip | Control)
- SETT-06: Pagination consolidated to single select dropdown
- SETT-07: HUD position shown as select dropdown with Disabled option

**Out ofScope:**

- New settings or backend changes
- Tab structure changes (completed in Phase 10)
- Settings data model changes

</domain>

<findings>
## Current Implementation Analysis

### 1. Settings Layout Pattern (Current State)

All settings components use a similar pattern:

- `flex items-center justify-between` for single-row controls
- Label + InfoTooltip on left, control on right
- Components wrap themselves in container divs with styling

**Files affected:**

- `src/lib/components/settings/general-settings.svelte` (6 settngs)
- `src/lib/components/settings/appearance-settings.svelte` (theme selector)
- `src/lib/components/settings/playback-settings.svelte` (4 controls)
- `src/lib/components/settings/trigger-settings.svelte` (3 settings)
- `src/lib/components/settings/pagination-settings.svelte` (2 controls)
- `src/lib/components/settings/hud-settings.svelte` (2 controls)
- `src/lib/components/settings/history-settings.svelte`
- `src/lib/components/settings/hotkey-settings.svelte`
- `src/lib/components/settings/sanitization-settings.svelte`
- `src/lib/components/settings/import-export-settings.svelte`
- `src/lib/components/settings/about-settings.svelte`

### 2. Pagination Settings (Current)

**File:** `src/lib/components/settings/pagination-settings.svelte`

```svelte
<div class="flex items-center justify-between">
  <div class="flex items-center gap-1.5">
    <Label>Enabled</Label>
    <InfoTooltip />
  </div>
  <Switch bind:checked={localConfig.pagination.enabled} />
</div>

<div class="space-y-2">
  <div class="flex items-center gap-1.5">
    <Label>Fragment Size</Label>
    <InfoTooltip />
  </div>
  <Input type="number" bind:value={localConfig.pagination.fragment_size} />
</div>
```

**Type:** `PaginationConfig { enabled: boolean; fragment_size: number; }`

**Required Change (SETT-06):**

- Convert to single `<Select>` dropdown
- Options: `Disabled` | `200` | `400` | `600` | `800` | `1000` | `1200` | `1400` | `1600` | `1800` | `2000`
- When "Disabled" → `enabled: false`
- When number selected → `enabled: true, fragment_size: <number>`

### 3. HUD Settings (Current)

**File:** `src/lib/components/settings/hud-settings.svelte`

```svelte
<div class="flex items-center justify-between">
  <div class="space-y-0.5">
    <Label>Enabled</Label>
    <p class="text-muted-foreground text-xs">Description</p>
  </div>
  <Switch bind:checked={localConfig.hud.enabled} />
</div>

{#if localConfig.hud.enabled}
  <div class="space-y-2">
    <Label>Position</Label>
    <Select options={hudPositionOptions} value={localConfig.hud.position} />
  </div>
{/if}
```

**Type:** `HudConfig { enabled: boolean; position: HudPosition; width: number; height: number; opacity: number; }`
**HudPosition:** `"top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right"`

**Required Change (SETT-07):**

- Convert to single `<Select>` dropdown
- Options: `Disabled` | `top-left` | `top-center` | `top-right` | `bottom-left` | `bottom-center` | `bottom-right`
- When "Disabled" → `enabled: false`
- When position selected → `enabled: true, position: <position>`

### 4. 2-Column Grid Layout (SETT-05)

**Current Pattern:**

```svelte
<div class="flex items-center justify-between">
  <div class="flex items-center gap-1.5">
    <Label>Label Text</Label>
    <InfoTooltip text="..." />
  </div>
  <Control />
</div>
```

**Required Pattern:**

```svelte
<div class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-3">
  <div class="flex items-center gap-1.5">
    <Label>Label Text</Label>
    <InfoTooltip text="..." />
  </div>
  <Control />
</div>
```

**Key Points:**

- 2-column grid layout
- Column 1: Label + InfoTooltip (auto-width)
- Column 2: Control (1fr - takes remaining space)
- Consistent `gap-x-4 gap-y-3` spacing
- Each setting row follows this pattern

### 5. UI Components Available

**Select Component** (`src/lib/components/ui/select/select.svelte`):

- Props: `options: Array<{ value: string; label: string }>`, `value`, `class`
- Already used in settings for language and retrigger mode

**InfoTooltip** (`src/lib/components/ui/info-tooltip.svelte`):

- Already used consistently across all settings

**Container Pattern:**

- Current: each component wraps itself in `<div class="border-border rounded-lg border p-4 shadow-sm">`
- Phase 10 uses cards in settings-page.svelte: `<div class="border-border overflow-hidden rounded-lg border">`

</findings>

<standard_stack>

## Tech Stack

- **Framework:** Tauri v2 + Svelte 5 + SvelteKit
- **Styling:** Tailwind CSS v4 + shadcn-svelte
- **State:** Svelte 5 runes ($state, $derived, $props)
- **Bindings:** Svelte 5 two-way binding ($bindable)

</standard_stack>

<architecture_patterns>

## Architecture Patterns

### 1. Component Structure

- Settings components are pure presentational
- They receive `localConfig` as `$bindable()` prop
- Parent (`settings-page.svelte`) manages save/cancel lifecycle

### 2. 2-Column Layout Approach

- **Option A:** Create reusable `SettingRow` component
- **Option B:** Apply grid class to existing containers
- **Recommendation:** Option A - reusable component for consistency

```svelte
<!-- SettingRow.svelte -->
<script lang="ts">
  let { label, tooltip, children } = $props<{
    label: string;
    tooltip?: string;
    children: import("svelte").Snippet;
  }>();
</script>

<div class="grid grid-cols-[auto_1fr] items-center gap-x-4">
  <div class="flex items-center gap-1.5">
    <Label>{label}</Label>
    {#if tooltip}<InfoTooltip text={tooltip} />{/if}
  </div>
  <div class="flex justify-end">
    {@render children()}
  </div>
</div>
```

### 3. Pagination Dropdown Logic

```typescript
const PAGINATION_OPTIONS = [
  { value: "disabled", label: "Disabled" },
  { value: "200", label: "200 characters" },
  { value: "400", label: "400 characters" }
  // ... up to 2000
];

// On change:
function handlePaginationChange(newValue: string) {
  if (newValue === "disabled") {
    localConfig.pagination.enabled = false;
  } else {
    localConfig.pagination.enabled = true;
    localConfig.pagination.fragment_size = parseInt(newValue);
  }
}

// Computed value for select:
const paginationValue = $derived(
  localConfig.pagination.enabled ? String(localConfig.pagination.fragment_size) : "disabled"
);
```

### 4. HUD Dropdown Logic

```typescript
const HUD_OPTIONS = [
  { value: "disabled", label: "Disabled" },
  { value: "top-left", label: "Top Left" },
  { value: "top-center", label: "Top Center" }
  // ...
];

function handleHudChange(newValue: string) {
  if (newValue === "disabled") {
    localConfig.hud.enabled = false;
  } else {
    localConfig.hud.enabled = true;
    localConfig.hud.position = newValue as HudPosition;
  }
}
```

</architecture_patterns>

<dont_hand_roll>

## Do Not Hand Roll

- **UI Components:** Use existing shadcn-svelte components (Select, Label, Switch, Input, Slider)
- **Icons:** Use @lucide/svelte icons (already imported)
- **i18n:** Use existing `$_()` pattern for internationalization
- **Tooltips:** Use existing InfoTooltip component

</dont_hand_roll>

<common_pitfalls>

## Common Pitfalls

### 1. Breaking Existing Bindings

Current components use `bind:value` and `bind:checked` directly. Minified dropdowns need derived state + onchange handler.

### 2. Losing InfoTooltips

All settings have InfoTooltip. Don't drop them during refactoring.

### 3. Grid vs Flex Confusion

Grid requires `grid-cols-[auto_1fr]` for proper column sizing. Flex `justify-between` won't work for multi-row layouts.

### 4. Missing i18n Keys

New options need i18n keys (e.g., "Disabled", character counts). Check `en.json` first.

### 5. Conditional Rendering

HUD currently conditionally renders position when enabled. After consolidation, always show dropdown with "Disabled" option.

</common_pitfalls>

<validation_architecture>

## Validation Requirements

### SETT-05 Verification

- All setting rows use 2-column grid layout
- Label + InfoTooltip aligned left
- Control aligned right
- Consistent spacing across all tabs

### SETT-06 Verification

- Single Select dropdown for Pagination
- Options: Disabled, 200, 400, 600...2000
- "Disabled" sets `pagination.enabled = false`
- Number value sets `enabled = true` + `fragment_size`

### SETT-07 Verification

- Single Select dropdown for HUD
- Options: Disabled, top-left, top-center, top-right, bottom-left, bottom-center, bottom-right
- "Disabled" sets `hud.enabled = false`
- Position value sets `enabled = true` + `position`

### Layout Consistency (SETT-05)

- Grid layout persists across all tabs (General, Advanced, About)
- Same spacing, alignment, visual rhythm

</validation_architecture>

<todos>
## Implementation Todos

1. Create reusable `SettingRow` component for 2-column layout
2. Convert PaginationSettings to single-select dropdown
3. Convert HudSettings to single-select dropdown
4. Update all settings components to use SettingRow
5. Add i18n keys for new options
6. Verify layout consistency across all tabs

</todos>

---

_Phase: 11-layout-consolidation_
_Research gathered: 2026-03-26_
