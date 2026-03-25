# Phase 5: Fix CLI Preset Apply - Research

**Researched:** 2026-03-05
**Domain:** Svelte 5 component prop patterns, parent-child event wiring
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

None — all implementation choices are left to Claude's discretion.

### Claude's Discretion

- **Preset logic location** — Keep `applyCliPreset()` in `engine-tabs.svelte` and pass it as a callback prop to `local-engine.svelte`, OR move `CLI_PRESETS` and the function into `local-engine.svelte` for self-containment. Either approach is acceptable.
- **Field display when preset selected** — Readonly `command` and `args_template` fields should show the applied preset values immediately (not stale values). The display should reflect what will be saved.
- **Switching to Custom** — When switching from a preset to "Custom", fields retain the last preset's values as the editable starting point (user can then modify freely). No reset to blank.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ENG-02 | User can configure backend-specific credentials and command settings | The preset dropdown currently sets `localConfig.tts.preset` but never updates `command` or `args_template`. Wiring `applyCliPreset()` into the preset `onchange` handler is the direct fix. `CLI_PRESETS` already contains the correct data for all 6 presets; no new data needed. |
</phase_requirements>

## Summary

Phase 5 is a minimal frontend wiring fix. The bug is a one-line omission: the preset dropdown `onchange` handler in `local-engine.svelte` (line 114-119) sets `localConfig.tts.preset` but never calls `applyCliPreset()`, which means `command` and `args_template` stay stale. The function exists and is correct — it just is never invoked.

Two valid approaches exist. Option A: pass `applyCliPreset` as a callback prop from `engine-tabs.svelte` to `local-engine.svelte`, calling it inside the existing `onchange`. Option B: move `CLI_PRESETS` into `local-engine.svelte` and inline the apply logic there, making the component self-contained. Option B is architecturally cleaner because `local-engine.svelte` already imports its own `INSTALL_COMMANDS` and `PIPER_EN_VOICES` — the preset data belongs with the component that uses it.

No Rust changes, no new UI, no new presets. The save bar already triggers on any `localConfig` mutation via `$derived(JSON.stringify...)`, so saving after preset selection will work automatically once command/args_template are mutated correctly.

**Primary recommendation:** Move `CLI_PRESETS` into `local-engine.svelte` and call the apply logic inline in the `onchange` handler — this removes the dead code in `engine-tabs.svelte` and makes `local-engine.svelte` self-contained.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Svelte 5 | (project version) | Component reactivity, `$bindable`, `$props` | Project stack |
| TypeScript | (project version) | Type safety for prop interfaces | Project stack |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@testing-library/svelte` | (project version) | Component unit tests | Existing test pattern for all engine components |
| vitest | (project version) | Test runner | Already configured, used across all test files |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Move CLI_PRESETS to local-engine.svelte | Pass callback prop from parent | Callback prop keeps parent in control but increases coupling; self-containment is simpler for a component that already owns its own preset-related constants |

## Architecture Patterns

### Pattern 1: `$bindable` prop mutation (existing pattern)

**What:** Child component receives `localConfig` via `$bindable` and mutates it directly. Parent's `$derived` detects the change and shows the save bar.

**When to use:** All engine sub-components use this. `LocalEngine` already uses it for `voice`, `command`, `args_template`.

**Existing usage in `engine-tabs.svelte` line 170:**
```svelte
<LocalEngine bind:localConfig />
```

**Existing prop declaration in `local-engine.svelte` line 11:**
```typescript
let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();
```

This means any mutation to `localConfig.tts.*` inside `local-engine.svelte` is automatically reflected in the parent's `localConfig` state and triggers `hasChanges`.

### Pattern 2: Self-contained preset application (recommended)

**What:** Move `CLI_PRESETS` from `engine-tabs.svelte` into `local-engine.svelte`. Call the apply logic directly in the `onchange` handler.

**Current broken handler (local-engine.svelte lines 114-119):**
```svelte
onchange={(e) => {
  const preset = (e.target as HTMLSelectElement).value;
  localConfig.tts.preset = preset;
  // Apply preset configuration would be handled by parent  ← never wired
}}
```

**Fixed handler (after moving CLI_PRESETS into local-engine.svelte):**
```svelte
onchange={(e) => {
  const preset = (e.target as HTMLSelectElement).value;
  localConfig.tts.preset = preset;
  if (preset !== "custom") {
    const cfg = CLI_PRESETS[preset];
    if (cfg) {
      localConfig.tts.command = cfg.command;
      localConfig.tts.args_template = cfg.args;
    }
  }
  // "custom": retain existing values as editable starting point (per user decision)
}}
```

### Pattern 3: Callback prop approach (alternative)

**What:** Add an `onPresetChange` callback prop to `local-engine.svelte`. Parent passes `applyCliPreset` as the prop value.

**engine-tabs.svelte:**
```svelte
<LocalEngine bind:localConfig onPresetChange={applyCliPreset} />
```

**local-engine.svelte props:**
```typescript
let {
  localConfig = $bindable(),
  onPresetChange,
}: {
  localConfig: AppConfig;
  onPresetChange?: (preset: string) => void;
} = $props();
```

**Tradeoff vs Option B:** This is valid Svelte 5 pattern but requires two-file changes and the parent still owns logic the child is better positioned to own. The `// @ts-expect-error ts(6133)` suppression in `engine-tabs.svelte` line 123 exists solely because `applyCliPreset` is unused — Option B removes the dead code entirely.

### Anti-Patterns to Avoid

- **Keeping `// @ts-expect-error ts(6133)` suppression:** This is a TypeScript "unused variable" warning suppressor — it exists only because `applyCliPreset` is never called. Whichever approach is chosen, this suppression should be removed as part of the fix.
- **Resetting fields to blank on "Custom":** Per user decision, switching to Custom retains last preset values as editable starting point. Do not clear `command` or `args_template` when preset === "custom".
- **Not removing CLI_PRESETS from engine-tabs.svelte if moving to local-engine:** After moving, the parent copy becomes unused dead code.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reactive save bar detection | Custom event bus or store | Existing `JSON.stringify` comparison in `hasChanges` $derived | Already works for any localConfig mutation |
| Field readonly enforcement | New validation logic | Existing `readonly={localConfig.tts.preset !== "custom"}` on Input elements | Already correct in local-engine.svelte lines 129, 145 |

**Key insight:** The readonly display logic is already correct — `command` and `args_template` inputs are readonly when preset !== "custom". Once `applyCliPreset()` mutates those fields, the readonly inputs will automatically display the new values. No display logic changes needed.

## Common Pitfalls

### Pitfall 1: Stale display after preset change

**What goes wrong:** If `command` and `args_template` Input elements use `bind:value` instead of controlled `value=`, stale DOM values may persist even after `localConfig` mutation.

**Why it happens:** The existing inputs use controlled `value={localConfig.tts.command}` (line 128) and `value={localConfig.tts.args_template.join(" ")}` (line 144) — not `bind:value`. In Svelte 5, controlled value props re-render on state change, so this is correct.

**How to avoid:** Keep the existing controlled pattern. After mutating `localConfig.tts.command` and `localConfig.tts.args_template`, Svelte's reactivity will re-render the Input values.

**Warning signs:** If the Command field still shows the old value after preset selection, check whether the mutation is actually reaching `localConfig` (verify `$bindable` is in use and not a stale copy).

### Pitfall 2: `args` vs `args_template` key mismatch

**What goes wrong:** `CLI_PRESETS` stores args under the key `args` (e.g., `cfg.args`). `TtsConfig` stores it as `args_template`. The existing `applyCliPreset()` in `engine-tabs.svelte` correctly maps `cfg.args → localConfig.tts.args_template`. This mapping must be preserved.

**Why it happens:** The preset data structure uses `args` as shorthand; the config type uses `args_template` as the canonical field name.

**How to avoid:** When copying `CLI_PRESETS` into `local-engine.svelte`, preserve the `{ command: string; args: string[] }` type and keep the `cfg.args → args_template` assignment.

**Warning signs:** TypeScript will catch this if types are declared correctly. If `CLI_PRESETS` is typed as `Record<string, { command: string; args: string[] }>`, assigning `cfg.args_template` would be a type error.

### Pitfall 3: `// @ts-expect-error` left in place after fix

**What goes wrong:** If Option A (callback prop) is used, `applyCliPreset` is now called — but the `// @ts-expect-error ts(6133)` suppressor remains, masking a future real type error on the same line.

**How to avoid:** Remove the `// @ts-expect-error ts(6133)` comment when the function is no longer unused. TypeScript should not need suppression for a used function.

## Code Examples

### Complete CLI_PRESETS constant (verified from engine-tabs.svelte lines 31-83)

```typescript
// Source: engine-tabs.svelte lines 31-83 (to be moved to local-engine.svelte)
const CLI_PRESETS: Record<string, { command: string; args: string[] }> = {
  "kokoro-tts": {
    command: "kokoro-tts",
    args: ["{input}", "{output}", "--voice", "{voice}"],
  },
  piper: {
    command: "python3",
    args: ["-m", "piper", "--data-dir", "{data_dir}", "-m", "{voice}", "-f", "{output}", "--input-file", "{input}"],
  },
  chatterbox: {
    command: "chatterbox-tts",
    args: ["--text", "{input}", "--output", "{output}", "--voice", "{voice}"],
  },
  "coqui-tts": {
    command: "tts",
    args: ["--text", "{input}", "--out_path", "{output}", "--model_name", "{voice}"],
  },
  espeak: {
    command: "espeak-ng",
    args: ["-w", "{output}", "-v", "{voice}", "{input}"],
  },
  "edge-tts": {
    command: "edge-tts",
    args: ["--text", "{input}", "--write-media", "{output}", "--voice", "{voice}"],
  },
};
```

### Verified preset apply logic (from engine-tabs.svelte lines 124-131)

```typescript
// Source: engine-tabs.svelte lines 124-131
// @ts-expect-error ts(6133)   ← REMOVE this suppressor once function is used
function applyCliPreset(preset: string) {
  if (preset === "custom") return;
  const cfg = CLI_PRESETS[preset];
  if (cfg && localConfig) {
    localConfig.tts.command = cfg.command;
    localConfig.tts.args_template = cfg.args;
  }
}
```

### How the save bar detects the change automatically (engine-tabs.svelte lines 24-28)

```typescript
// Source: engine-tabs.svelte lines 24-28
const hasChanges = $derived(
  originalConfig !== null &&
    localConfig !== null &&
    JSON.stringify(localConfig) !== JSON.stringify(originalConfig),
);
```

Mutating `localConfig.tts.command` and `localConfig.tts.args_template` via `$bindable` will cause this derived to re-evaluate and show the save bar. No extra work needed.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Svelte 4 `export let` props + `createEventDispatcher` | Svelte 5 `$props()` + `$bindable()` + callback props | Svelte 5 (project already uses this) | Direct mutation via $bindable replaces event dispatch for two-way data flow |
| `on:change` directive | `onchange` attribute | Svelte 5 | Project already uses `onchange` (see CLAUDE.md) |

**Deprecated/outdated:**
- `on:change`, `on:click` etc.: Svelte 4 event directives — project uses Svelte 5 `onchange`, `onclick` attribute style per CLAUDE.md

## Open Questions

1. **Which approach does the planner prefer: Option A (callback prop) or Option B (move CLI_PRESETS)?**
   - What we know: Both are valid Svelte 5 patterns; both fix the bug in a single wave
   - What's unclear: Team preference for co-location vs. parent-owns-logic
   - Recommendation: Option B (self-containment) — removes dead code in parent, aligns with how `local-engine.svelte` already owns `INSTALL_COMMANDS` and `PIPER_EN_VOICES`

## Sources

### Primary (HIGH confidence)

- Direct source read: `engine-tabs.svelte` — verified `CLI_PRESETS`, `applyCliPreset()`, `hasChanges`, `bind:localConfig` usage
- Direct source read: `local-engine.svelte` — verified broken `onchange`, readonly field logic, `$bindable` prop declaration
- Direct source read: `src/lib/types.ts` — verified `TtsConfig` interface fields (`preset`, `command`, `args_template`)
- Direct source read: `local-engine.test.ts`, `engine-tabs.test.ts` — verified test patterns and existing mock structure
- Direct source read: `.planning/config.json` — confirmed `nyquist_validation: false`

### Secondary (MEDIUM confidence)

- CLAUDE.md: Confirmed Svelte 5 runes (`$state`, `$derived`, `$props`), `onchange` not `on:change` style requirement

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — read directly from source files
- Architecture patterns: HIGH — verified from existing component code
- Pitfalls: HIGH — identified from direct code inspection (type mismatch, stale display, ts-expect-error)

**Research date:** 2026-03-05
**Valid until:** 2026-04-05 (stable — no external dependencies, pure internal wiring)
