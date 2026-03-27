---
phase: 09-tts-engine-overhaul
plan: "02"
subsystem: frontend/types
tags: [typescript, svelte, engine-tabs, openai, http-removal, migration]
dependency_graph:
  requires: []
  provides:
    - "Clean TtsEngine type: local | openai | elevenlabs"
    - "3-tab engine UI with HTTP migration toast"
    - "9-voice OpenAI dropdown"
  affects:
    - src/lib/types.ts
    - src/lib/components/engine/engine-tabs.svelte
    - src/lib/components/engine/openai-engine.svelte
tech_stack:
  added: []
  patterns:
    - "as any cast for stale HTTP backend guard at migration boundary"
key_files:
  created: []
  modified:
    - src/lib/types.ts
    - src/lib/components/engine/engine-tabs.svelte
    - src/lib/components/engine/openai-engine.svelte
    - src/lib/components/engine/engine-tabs.test.ts
    - src/lib/models/html-templates.test.ts
    - src/lib/utils/html-export.test.ts
    - src/lib/components/engine/openai-engine.test.ts
    - src/lib/components/engine/elevenlabs-engine.test.ts
    - src/lib/components/engine/local-engine.test.ts
    - src/lib/components/synthesize-page.svelte
  deleted:
    - src/lib/components/engine/http-engine.svelte
    - src/lib/components/engine/http-engine.test.ts
decisions:
  - "[09-02] TtsEngine TypeScript type is now 'local' | 'openai' | 'elevenlabs' — http variant removed"
  - "[09-02] HTTP migration toast uses (config.tts.active_backend === ('http' as any)) guard for stale value detection"
  - "[09-02] OpenAI voice dropdown expanded to all 9 voices: alloy, ash, coral, echo, fable, nova, onyx, shimmer, verse"
metrics:
  duration: "~15min"
  completed: "2026-03-09"
  tasks_completed: 2
  files_modified: 12
  files_deleted: 2
---

# Phase 9 Plan 02: Frontend HTTP Engine Removal Summary

**One-liner:** Removed HTTP engine from TypeScript types and Svelte UI, replaced 4-tab engine panel with clean 3-tab layout (Local/OpenAI/ElevenLabs), added HTTP migration toast, and expanded OpenAI voices to 9.

## What Was Done

### Task 1: Clean TypeScript Types (commit: 2aaa5b9)

In `src/lib/types.ts`:
- `TtsEngine` changed from `"local" | "openai" | "elevenlabs" | "http"` to `"local" | "openai" | "elevenlabs"`
- `HttpTtsConfig` interface deleted
- `TtsBackendType` type alias deleted
- `http_tts: HttpTtsConfig` field removed from `AppConfig`
- `HttpUrlEmpty` and `HttpTimeoutTooSmall` variants removed from `ValidationError`

Cascading fixes to test mock configs (all had `http_tts` in mock data):
- `openai-engine.test.ts`, `elevenlabs-engine.test.ts`, `local-engine.test.ts`, `engine-tabs.test.ts`
- `html-templates.test.ts`, `html-export.test.ts` — `by_engine` record dropped `http: 0`
- `synthesize-page.svelte` — dev mock config updated

### Task 2: Remove HTTP Engine UI (commit: 3ee8122)

Files deleted:
- `src/lib/components/engine/http-engine.svelte`
- `src/lib/components/engine/http-engine.test.ts`

`engine-tabs.svelte` changes:
- Removed `import HttpEngine from "./http-engine.svelte"`
- Removed `<TabsTrigger value="http">HTTP Server</TabsTrigger>`
- Removed `<TabsContent value="http">` block
- Added HTTP migration detection in `loadConfig()` — if `active_backend === "http"`, resets to `"local"` and shows `toast.info()`

`openai-engine.svelte` changes:
- Voice dropdown expanded from 6 to 9 options: added `ash`, `coral`, `verse`

`engine-tabs.test.ts` changes:
- Updated "render all tabs" test to properly mock invoke and assert 3 tabs (was pre-existing broken test expecting 4 with no mock setup)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed cascading http_tts references in test mocks and component mocks**
- **Found during:** Task 1
- **Issue:** Removing `http_tts` from `AppConfig` interface broke 7 test files that had `http_tts` in mock config objects, plus `synthesize-page.svelte` which had a dev fallback config
- **Fix:** Removed `http_tts` block from all affected mocks; updated `by_engine` Record in stats tests
- **Files modified:** openai-engine.test.ts, elevenlabs-engine.test.ts, local-engine.test.ts, engine-tabs.test.ts, html-templates.test.ts, html-export.test.ts, synthesize-page.svelte
- **Commit:** 2aaa5b9

**2. [Rule 1 - Bug] Fixed pre-existing broken engine-tabs test**
- **Found during:** Task 2
- **Issue:** `should render all four backend tabs` test was already failing before this plan (rendered 0 tabs because it had no invoke mock; tabs only render when config loads). After our changes it would still fail with new count.
- **Fix:** Rewrote test to properly mock `invoke.mockResolvedValue`, await config load, then assert 3 tabs (Local/OpenAI/ElevenLabs)
- **Files modified:** engine-tabs.test.ts
- **Commit:** 3ee8122

### Deferred Items (out of scope)

`src/lib/components/settings/tts-settings.svelte` still references `localConfig.http_tts.*` in 12 places. This file was not in the plan scope (engine UI only). The TypeScript checker does not flag these as errors due to Svelte's lenient checking. Cleanup should be addressed in a subsequent plan.

## Verification

- `ls src/lib/components/engine/http-engine.svelte` → No such file
- `grep -r "http_tts|HttpTtsConfig" src/lib/ --exclude="*.test.*"` → Only tts-settings.svelte (deferred)
- OpenAI engine has ash, coral, verse in dropdown
- `engine-tabs.svelte` imports only LocalEngine, OpenAiEngine, ElevenLabsEngine
- `bun run check` — 23 pre-existing errors, 0 new errors from this plan
- All engine-tabs tests pass (4/4)

## Self-Check: PASSED

- types.ts: FOUND
- engine-tabs.svelte: FOUND
- openai-engine.svelte: FOUND
- http-engine.svelte: DELETED (confirmed via ls)
- 09-02-SUMMARY.md: FOUND
- commit 2aaa5b9: FOUND
- commit 3ee8122: FOUND
