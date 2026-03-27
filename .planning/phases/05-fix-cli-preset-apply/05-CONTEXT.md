# Phase 5: Fix CLI Preset Apply - Context

**Gathered:** 2026-03-05
**Updated:** 2026-03-08 (post-implementation)
**Status:** Complete

<domain>
## Phase Boundary

Wire the CLI preset dropdown so that selecting a preset (e.g. "Kokoro TTS") immediately updates `localConfig.tts.command` and `localConfig.tts.args_template` in addition to `localConfig.tts.preset`. `applyCliPreset()` currently exists in `engine-tabs.svelte` but is never called — this phase fixes that orphan. No new features; no new presets; no UI redesign.

</domain>

<decisions>
## Implementation Decisions

### Architecture (resolved)

- **Preset logic location** — `CLI_PRESETS` constant and apply logic moved into `local-engine.svelte` (self-contained). The standalone `applyCliPreset()` function was removed entirely — preset application is now inline in the onchange handler. No callback prop pattern used.
- **Dead code cleanup** — `CLI_PRESETS` constant and `applyCliPreset()` (with its `@ts-expect-error` suppressor) removed from `engine-tabs.svelte`. Parent no longer owns any preset logic.
- **Field display when preset selected** — Readonly `command` and `args_template` fields update immediately on preset selection via direct `localConfig` mutation. Save bar triggers automatically via existing `$bindable` reactivity.
- **Switching to Custom** — Fields retain the last preset's values as editable starting point. No reset to blank.

### Test Infrastructure (resolved)

- **Minimal regression tests** — 3 ENG-02 tests created in `eng02-minimal.test.ts` (separate from existing `local-engine.test.ts` to avoid vi.mock hoisting issues)
- **vi.mock hoisting fix** — `vi.hoisted()` pattern required for Tauri IPC mocks in Svelte component tests
- **svelteTesting() plugin** — Was missing from `vitest.config.ts`; added `svelteTesting()` from `@testing-library/svelte/vite` to plugins array
- **TDD approach confirmed** — Write failing tests first, then minimal code to pass, then dead code cleanup

</decisions>

<code_context>
## Existing Code Insights

### Current State (post-fix)

**`local-engine.svelte` — preset apply logic (lines ~63-95, ~174-182):**
```svelte
const CLI_PRESETS: Record<string, { command: string; args: string[] }> = {
  "kokoro-tts": { command: "kokoro-tts", args: ["{input}", "{output}", "--voice", "{voice}"] },
  piper: { command: "python3", args: ["-m", "piper", ...] },
  // ... 4 more presets
};

// In onchange handler:
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
}}
```

**`engine-tabs.svelte` — cleaned up:**
- No longer contains `CLI_PRESETS` or `applyCliPreset()`
- Still owns `localConfig` state and save bar pattern

### Established Patterns

- Child components receive `localConfig` via `$bindable` prop and mutate directly — preset apply follows this pattern
- `hasChanges` derived in parent auto-detects mutations through `$bindable` reactivity
- Save bar appears automatically on any `localConfig` change

### Integration Points

- `local-engine.svelte` is fully self-contained for CLI preset logic
- `engine-tabs.svelte` orchestrates tab switching and save/cancel
- No Rust changes were needed — purely frontend wiring

</code_context>

<specifics>
## Specific Ideas

No specific requirements — the fix was minimal and self-contained.

</specifics>

<residual>
## Residual Issues

### applyCliPreset in tts-settings.svelte
- `src/lib/components/settings/tts-settings.svelte` still contains an `applyCliPreset()` function (line 189, called at line 295)
- This is in the **old Settings page TTS section** which should have been hard-deleted in Phase 2 (SET-01)
- Not blocking — the Settings page no longer renders TTS config — but the file still exists as dead code

### Pre-existing Phase 3 test failures
- 6 test failures remain in the engine test suite from Phase 3 (health check UI tests)
- Root causes: ambiguous query selectors, espeak text matching assertions
- Unrelated to Phase 5 — preset wiring tests all pass

</residual>

<deferred>
## Deferred Ideas

- Clean up `tts-settings.svelte` dead file — should have been removed in Phase 2 SET-01
- Fix 6 pre-existing Phase 3 test failures (ambiguous selectors, espeak matching)

</deferred>

---

*Phase: 05-fix-cli-preset-apply*
*Context gathered: 2026-03-05*
*Post-implementation update: 2026-03-08*
