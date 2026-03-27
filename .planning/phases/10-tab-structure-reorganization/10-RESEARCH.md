---
phase: 10-tab-structure-reorganization
research_date: 2026-03-26
status: complete
---

# Phase 10 Research: Tab Structure Reorganization

**Question:** What do I need to know to PLAN this phase well?

## Executive Summary

Phase 10 consolidates 8 settings navigation items into 3 tabs (General, Advanced, About) while maintaining scroll-aware navigation highlighting within each consolidated tab. The reorganization is purely frontend—no backend/data model changes required.

## Current Architecture

### Navigation Structure

**Current sidebar navigation** (settings-page.svelte, lines 61-70):

```typescript
const settingsCategories = [
  { id: "app", categoryKey: "general" },
  { id: "playback", categoryKey: "playback" },
  { id: "triggers", categoryKey: "triggers" },
  { id: "pagination", categoryKey: "pagination" },
  { id: "sanitization", categoryKey: "sanitization" },
  { id: "history", categoryKey: "history" },
  { id: "hud", categoryKey: "hud" },
  { id: "about", categoryKey: "about" }
];
```

**8 navigation items → 3 tabs target:**

| Current            | Target Tab | Target Sections     |
| ------------------ | ---------- | ------------------- |
| General (app)      | General    | Startup, Appearance |
| Playback           | General    | Playback            |
| Triggers + Hotkeys | General    | Triggers, Hotkeys   |
| Pagination         | Advanced   | Pagination          |
| Sanitization       | Advanced   | Sanitization        |
| History            | General    | History             |
| HUD                | General    | HUD                 |
| About              | About      | App Info            |

**Import/Export** currently lives at top of General section—moves to About.

### Scroll-Aware Navigation (IntersectionObserver)

**Current implementation** (lines 139-167):

- Uses `IntersectionObserver` with `rootMargin: "-120px 0px -60% 0px"`
- Tracks `activeSection` state (reactive string)
- `scrollToSection()` sets `isManualScroll = true` for 800ms to prevent observer fighting
- Each section has `id="section-{category.id}"`for anchoring

**Key pattern to preserve:**

```typescript
function scrollToSection(sectionId: string) {
  isManualScroll = true;
  activeSection = sectionId;
  const element = document.getElementById(`section-${sectionId}`);
  if (element) {
    element.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  setTimeout(() => {
    isManualScroll = false;
  }, 800);
}
```

### Staggered Mounting Pattern

**Current pattern** (lines 27-28, 241-258):

- Uses `mountedCount` state (starts at999, decrements would be pattern but currently unused)
- Components render skeleton/spinner while mounting
- Purpose: Prevent WebView2 crash from rapid component initialization

**Important:** This pattern should be preserved or simplified—not removed entirely.

### Component Responsibilities

| Component                       | Current Content                       | Reorganization Needed               |
| ------------------------------- | ------------------------------------- | ----------------------------------- |
| `general-settings.svelte`       | Startup toggles, language, debug mode | Move to General tab, keep structure |
| `appearance-settings.svelte`    | Theme selector (light/dark/system)    | Move to General tab                 |
| `playback-settings.svelte`      | Volume, speed, pitch, retrigger mode  | Move to General tab                 |
| `trigger-settings.svelte`       | Double-copy window, max text length   | Move to General tab                 |
| `hotkey-settings.svelte`        | Global hotkey configuration           | Move to General tab                 |
| `hud-settings.svelte`           | HUD enable/position                   | Move to General tab                 |
| `pagination-settings.svelte`    | Enable + fragment size                | Move to Advanced tab                |
| `sanitization-settings.svelte`  | Markdown strip, TTS normalization     | Move to Advanced tab                |
| `history-settings.svelte`       | Storage mode, auto-delete, cleanup    | Move to General tab                 |
| `import-export-settings.svelte` | Export/Import/Reset dialogs           | Move to About tab                   |
| `about-settings.svelte`         | Version, acknowledgments              | Stay in About tab                   |

### i18n Structure

**Locale file structure** (src/lib/locales/en.json):

```json
"settings": {
  "categories": {
    "general": "General",
    "playback": "Playback",
    ...
  },
  "descriptions": { ... }
}
```

**New i18n keys needed:**

- `settings.tabs.general` → "General"
- `settings.tabs.advanced` → "Advanced"
- `settings.tabs.about` → "About"
- `settings.sections.startup`, `.appearance`, `.playback`, `.triggers`, `.hotkeys`, `.history`, `.hud`, `.pagination`, `.sanitization`

## Target Architecture

### Tab-Based Navigation

**New sidebar structure:**

```typescript
const tabs = [
  { id: "general", labelKey: "settings.tabs.general" },
  { id: "advanced", labelKey: "settings.tabs.advanced" },
  { id: "about", labelKey: "settings.tabs.about" }
];
```

**Per-tab sections:**

| Tab      | Sections (ordered)                                             |
| -------- | -------------------------------------------------------------- |
| General  | Startup, Appearance, Playback, HUD, History, Triggers, Hotkeys |
| Advanced | Pagination, Sanitization                                       |
| About    | App Info, Import/Export                                        |

### Section Navigation Within Tabs

**Two-level navigation:**

1. **Tab level:** Click tab → scroll to top of that tab's content
2. **Section level:** Within each tab, left sidebar shows sections for that tab only

**Pattern:**

```svelte
{#each tabs as tab}
  <button class:active={activeTab === tab.id} onclick={() => switchTab(tab.id)}>
    {$_(tab.labelKey)}
  </button>
{/each}
```

**Section sidebar updates per tab:**

- When `activeTab === 'general'`: Show sections for General
- When `activeTab === 'advanced'`: Show sections for Advanced
- When `activeTab === 'about'`: About has no sub-sections (single scroll)

## Technical Decisions

### Decision 1: Tab State Management

**Approach:** Add `activeTab` state to settings-page.svelte

- `activeTab = $state<'general' | 'advanced' | 'about'>('general')`
- Tab click: Set `activeTab`, scroll to top, reset `activeSection` to first section in that tab

### Decision 2: Section Registry

**Approach:** Create section configuration per tab

```typescript
const tabSections = {
  general: [
    { id: "startup", labelKey: "settings.sections.startup" },
    { id: "appearance", labelKey: "settings.sections.appearance" },
    { id: "playback", labelKey: "settings.sections.playback" },
    { id: "hud", labelKey: "settings.sections.hud" },
    { id: "history", labelKey: "settings.sections.history" },
    { id: "triggers", labelKey: "settings.sections.triggers" },
    { id: "hotkeys", labelKey: "settings.sections.hotkeys" }
  ],
  advanced: [
    { id: "pagination", labelKey: "settings.sections.pagination" },
    { id: "sanitization", labelKey: "settings.sections.sanitization" }
  ],
  about: [] // No sub-navigation
};
```

### Decision 3: URL Routing (Optional)

**Recommendation:** Do NOT add URL routing in this phase

- Keeps scope minimal
- Settings page already loads on `/settings` route
- Tab state can be added to URL later if needed

### Decision 4: Preserve Existing Components

**Approach:** Reuse existing settings components unchanged

- Each component (e.g., `general-settings.svelte`) remains as-is
- Only the container structure in `settings-page.svelte` changes
- Components receive same props (`localConfig`, `errors`, etc.)

## Validation Architecture

### Must-Preserve Behaviors

1. **Scroll-aware sidebar highlight** — IntersectionObserver must continue to work
2. **Staggered mounting** — Prevent WebView2 crash with lazy component loading
3. **Save bar** — Unsaved changes detection and save/cancel buttons
4. **Config loading** — `loadConfig()` / `saveConfig()` / `cancelChanges()` pattern
5. **i18n** — All labels use `$_("settings.xxx")` pattern

### Testable Acceptance Criteria

| Criterion                   | How to Verify                                              |
| --------------------------- | ---------------------------------------------------------- |
| 3 tabs visible              | Visual: sidebar shows General, Advanced, About             |
| Click tab → sections change | Click "Advanced" → sidebar shows Pagination, Sanitization  |
| Scroll → section highlight  | Scroll within General → sidebar highlights correct section |
| Save bar appears on change  | Change any setting → save bar appears                      |
| Import/Export in About      | Navigate to About → Import/Export buttons visible          |

## Risk Assessment

| Risk                                             | Mitigation                                                      |
| ------------------------------------------------ | --------------------------------------------------------------- |
| IntersectionObserver breaks after reorganization | Keep same `id="section-{id}"` pattern for anchors               |
| i18n keys missing for new section names          | Add keys to en.json, verify translation files                   |
| Component mounting order changes                 | Maintain staggered mount pattern or validate WebView2 stability |
| Save bar position affected                       | Save bar is fixed at bottom; unrelated to tab structure         |

## File Modification Summary

| File                      | Change Type    | Description                                            |
| ------------------------- | -------------- | ------------------------------------------------------ |
| `settings-page.svelte`    | Major refactor | Restructure with tabs, section registry, two-level nav |
| `en.json`                 | Add keys       | New tab names, section names                           |
| Other settings components | None           | Reused as-is                                           |

## Out of Scope

- Backend changes (config structure unchanged)
- Data model changes
- New settings features
- URL routing for tabs
- Translation file updates (only en.json)

---

**Research complete.** Ready for planning.
