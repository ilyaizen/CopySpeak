---
phase: quick
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  [
    src/lib/components/settings-page.svelte,
    src/lib/components/settings/general-settings.svelte,
    src/lib/components/settings/appearance-settings.svelte
  ]
autonomous: true
must_haves:
  truths:
    - "User sees General, Advanced, About as three stacked category cards"
    - "Each category card contains its subsection cards inside"
    - "No left sidebar navigation - all content visible in vertical stack"
    - "Language and Theme settings grouped together in Appearance section"
    - "Save bar still appears on configuration changes"
  artifacts:
    - path: "src/lib/components/settings-page.svelte"
      provides: "Main settings layout"
      min_lines: 350
    - path: "src/lib/components/settings/appearance-settings.svelte"
      provides: "Consolidated Language + Theme settings"
      min_lines: 40
    - path: "src/lib/components/settings/general-settings.svelte"
      provides: "General settings without Language"
  key_links:
    - from: "settings-page.svelte"
      to: "settings/*.svelte"
      via: "component imports"
---

<objective>
Remove the double left menu (tabs + section navigation) and consolidate into a cleaner vertical card layout where General, Advanced, and About appear as three stacked category cards, each containing their subsections.

Purpose: Reduce visual clutter, simplify navigation, and create a more compact settings layout.
Output: Refactored settings-page.svelte with removed sidebar and stacked category cards.
</objective>

<execution_context>
@$HOME/.config/opencode/get-shit-done/workflows/execute-plan.md
@$HOME/.config/opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/STATE.md
@AGENTS.md

# Current Architecture (from code analysis)

The settings page has:

- Left sidebar (w-28 ~112px) with tab buttons (General/Advanced/About) and section navigation
- Main content area with scrollable sections
- IntersectionObserver for scroll-aware sidebar highlighting
- `activeTab` and `activeSection` state for navigation tracking
- Staggered rendering (`mountedCount`) for WebView2 performance

# Target Architecture

- No left sidebar - all content in main area
- Three vertically-stacked category cards: General, Advanced, About
- Each category card contains subsection cards inside
- Compact styling (smaller headers, tighter spacing)
  </context>

<tasks>

<task type="auto">
  <name>Task 1: Consolidate language/theme settings and refactor layout to remove sidebar</name>
  <files>src/lib/components/settings-page.svelte, src/lib/components/settings/general-settings.svelte, src/lib/components/settings/appearance-settings.svelte</files>
  <action>
    Refactor settings to consolidate appearance settings and simplify the layout:

    0. **Consolidate Appearance settings (language + theme):**
       - Move the Language Select from general-settings.svelte to appearance-settings.svelte
       - appearance-settings.svelte now contains both Theme and Language rows
       - Update settings-page.svelte: extract AppearanceSettings as its own section card (separate from GeneralSettings)
       - GeneralSettings no longer includes language

    1. **Remove left sidebar:**
       - Delete the `<aside class="w-28 shrink-0">` element and its contents
       - Remove `activeTab` state and `switchTab()` function (no longer needed)
       - Remove `tabs` and `tabSections` constants (no longer needed)
       - Remove `activeSection` used for sidebar highlighting (keep if used for scroll anchors)
       - Simplify or remove IntersectionObserver (no sidebar to sync with)

    2. **Simplify layout structure:**
       - Remove the `flex flex-row gap-4` wrapper (no sidebar)
       - Change `<main>` to full-width without sidebar offset
       - Stack all three category sections vertically in one flow

    3. **Restructure category cards:**
       - General card contains: Startup (GeneralSettings), Playback, HUD, History, Triggers+Hotkeys, Appearance (Language+Theme)
       - Advanced card contains: Pagination, Sanitization
       - About card contains: App Info, Import/Export
       - Section `<section id="section-...">` IDs can remain for anchor scrolling if useful

    4. **Compact styling:**
       - Reduce card header padding from `p-4` to `p-3`
       - Reduce header title from `text-lg` to `text-base`
       - Reduce description text to `text-xs` if helpful
       - Tighten section spacing from `space-y-8` to `space-y-6`

    5. **Keep functionality:**
       - Preserve `hasChanges`, `saveConfig`, `cancelChanges`, `resetToDefaults`
       - Preserve the save bar at bottom
       - Preserve staggered loading (`mountedCount`) for WebView2 performance
       - Preserve loading and error states

    The existing settings subcomponents (GeneralSettings, PlaybackSettings, etc.) remain unchanged except for the language move.

  </action>
  <verify>
    <automated>rtk bun run dev</automated>
    <manual>Open settings page and verify: no left sidebar, three stacked category cards, all sections visible, save bar appears on changes</manual>
  </verify>
  <done>
    - Language and Theme grouped together in AppearanceSettings component
    - AppearanceSettings rendered as separate section card (not bundled with GeneralSettings)
    - Left sidebar removed
    - Three category cards (General, Advanced, About) stacked vertically
    - Each category contains its subsections
    - Save bar still appears on configuration changes
    - Settings functionality preserved (load, save, cancel, reset)
  </done>
</task>

</tasks>

<verification>
- Settings page renders without left sidebar
- All three category sections visible in vertical stack
- Each category contains expected subsections
- Save bar appears when settings change
- No TypeScript errors (`rtk bun run check`)
</verification>

<success_criteria>

- Settings page uses a cleaner vertical card layout
- Double menu (tabs + section navigation) removed
- Layout is more compact and less cluttered
- All settings functionality preserved
  </success_criteria>

<output>
After completion, create `.planning/quick/260326-eqy-consolidate-settings-menus-remove-double/260326-eqy-SUMMARY.md`
</output>
