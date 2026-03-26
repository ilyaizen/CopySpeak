---
phase: quick
plan: 01
subsystem: frontend
tags: [settings, ui, refactor, navigation]
completed: 2026-03-26
duration: 5min
key_files_modified:
  - src/lib/components/settings-page.svelte
  - src/lib/components/settings/general-settings.svelte
  - src/lib/components/settings/appearance-settings.svelte
---

# Quick Task 260326-eqy: Consolidate Settings Menus Remove Double

## Summary

Removed the double left menu (tabs + section navigation) and consolidated the settings page into a cleaner vertical card layout with three stacked category cards: General, Advanced, and About. Language and Theme settings are now grouped together in the Appearance section.

## Changes Made

### SettingsPage (`settings-page.svelte`)

- **Removed** left sidebar navigation (`<aside class="w-28 shrink-0">` element)
- **Removed** tab state (`activeTab`, `switchTab()`) and section navigation (`activeSection`, `scrollToSection()`)
- **Removed** IntersectionObserver for scroll-aware sidebar tracking
- **Removed** `tabs` and `tabSections` constants
- **Restructured** layout to three vertically-stacked category cards:
  - **General**: Startup, Appearance (Language+Theme), Playback, HUD, History, Triggers+Hotkeys
  - **Advanced**: Pagination, Sanitization
  - **About**: App Info, Import/Export
- **Compacted** card header styling (`p-3`, `text-base`) for cleaner appearance
- **Preserved** all functionality: hasChanges, saveConfig, cancelChanges, resetToDefaults, save bar, staggered loading

### AppearanceSettings (`appearance-settings.svelte`)

- **Added** Language setting (moved from GeneralSettings)
- **Now contains** both Language and Theme settings in a single component
- **Uses** `bind:localConfig` pattern for direct config binding

### GeneralSettings (`general-settings.svelte`)

- **Removed** Language setting (moved to AppearanceSettings)
- **Removed** unused imports (Label, Select, svelte-i18n `$_`, getSupportedLocales, SupportedLocale type)
- **Simplified** to only Startup settings (Start with Windows, Start Minimized, Show Notifications, Minimize to Tray on Close, Check for Updates, Debug Mode)

## Deviations from Plan

None - plan executed exactly as written.

## Verification

- Settings page renders without left sidebar
- Three category cards (General, Advanced, About) visible in vertical stack
- Each category contains expected subsections
- Language and Theme grouped together in Appearance section
- Save bar appears when settings change
- All settings functionality preserved (load, save, cancel, reset)

## Files Modified

| File                         | Lines Changed | Description                                         |
| ---------------------------- | ------------- | --------------------------------------------------- |
| `settings-page.svelte`       | +190 -373     | Removed sidebar, restructured to stacked categories |
| `appearance-settings.svelte` | +46 -31       | Added Language setting, uses bind:localConfig       |
| `general-settings.svelte`    | +1 -24        | Removed Language setting, simplified imports        |

## Commit

```
288953a refactor(quick-260326-eqy): consolidate settings menus and remove double navigation
```

## Self-Check: PASSED

- [x] Files created/modified exist
- [x] Commit exists in git history
- [x] All must_haves satisfied:
  - [x] User sees General, Advanced, About as three stacked category cards
  - [x] Each category card contains its subsection cards inside
  - [x] No left sidebar navigation - all content visible in vertical stack
  - [x] Language and Theme settings grouped together in Appearance section
  - [x] Save bar still appears on configuration changes
