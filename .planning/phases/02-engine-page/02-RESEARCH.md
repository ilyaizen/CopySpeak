# Phase 2: Engine Page - Research

**Researched:** 2026-03-05
**Domain:** Svelte 5 component architecture, state management, TTS configuration UI
**Confidence:** HIGH

## Summary

Phase 2 extracts TTS configuration from Settings into a dedicated Engine page with tabbed navigation for 4 backends (Local/CLI, HTTP, OpenAI, ElevenLabs). The existing `tts-settings.svelte` (675 lines) contains all required functionality that needs component extraction. The save bar pattern from Settings (`localConfig`/`originalConfig` with JSON comparison) is directly reusable. Synthesis speed placeholders (`{speed}`, `{length_scale}`) need removal from presets and Rust defaults, while playback speed remains in Settings → Playback.

**Primary recommendation:** Extract the 4 backend sections from tts-settings.svelte into focused components under `src/lib/components/engine/`, reuse the existing shadcn-svelte Tabs component for tab navigation, and hard-delete the TTS section from Settings after Engine page is complete.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**1. Unsaved Changes Strategy**
- **Decision:** Save bar pattern (same as Settings) — no beforeNavigate prompt, no discard dialog
- **Behavior:** Unsaved changes show a floating bar with "Unsaved changes" + Cancel + Save buttons
- **State model:** `localConfig` / `originalConfig` pattern with `hasChanges` derived comparison
- **Rationale:** Simple, consistent with existing Settings pattern, no complex navigation guards

**2. Speed Control Scope — Synthesis Speed Removed**
- **Decision:** Remove ALL synthesis speed / length-scale logic from codebase
- **What gets deleted:**
  - `{speed}` and `{length_scale}` placeholders from CLI presets in `tts-settings.svelte`
  - Any synthesis speed UI fields (none exist today, but verify)
  - Related placeholder handling in Rust if present
- **What stays:** Playback speed (`playback.playback_speed`) remains in Settings → Playback (NOT moved to Engine)
- **ENG-04 clarification:** "select voice and speed" refers to voice only — speed is client-side playback, not engine synthesis
- **Rationale:** Speed adjustment happens client-side via audio player, not passed to TTS engines

**3. Engine Page Layout — Tabbed Sections**
- **Decision:** Tabbed layout with sub-tabs for each backend
- **Tabs:**
  - Local (CLI) — preset dropdown, command, args, voice
  - HTTP Server — preset dropdown, URL, body, headers, timeout, voice
  - OpenAI — API key, model, voice
  - ElevenLabs — API key, voice dropdown, model, output format, voice settings
- **File organization:** Parent directory for split components
  - `src/lib/components/engine/` — parent directory
  - `src/lib/components/engine/local-engine.svelte`
  - `src/lib/components/engine/http-engine.svelte`
  - `src/lib/components/engine/openai-engine.svelte`
  - `src/lib/components/engine/elevenlabs-engine.svelte`
  - `src/lib/components/engine/engine-tabs.svelte` — tab switcher + shared Test button
- **Test Engine button:** Shared at bottom of tab container (not per-backend)
- **Rationale:** Each backend has distinct configuration needs; tabs keep UI focused

### Claude's Discretion

- **Tab order:** Local → HTTP → OpenAI → ElevenLabs (local first as default, cloud options after)
- **Active tab persistence:** Use URL query param `?backend=local|http|openai|elevenlabs` or local state (Claude chooses cleaner approach)
- **Tab styling:** Follow existing brutalist pattern — hard edges, muted palette, mono fonts
- **Component extraction:** Split `tts-settings.svelte` (675 lines) into 4 focused components

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ENG-01 | User can select TTS backend (CLI, ElevenLabs, OpenAI, HTTP) | Backend selector dropdown exists in tts-settings.svelte:36-40; Tab state via shadcn-svelte Tabs component |
| ENG-02 | User can configure backend-specific credentials and command settings | All 4 backend configurations fully implemented in tts-settings.svelte:254-649; extract into components |
| ENG-04 | User can select voice and speed from the Engine page | Voice selection per backend exists; "speed" is playback speed (Settings), not synthesis speed |
| SET-01 | TTS engine settings are removed from the Settings page (hard delete) | Settings page imports TtsSettings component at line 4, 315; section wrapper at lines 304-331 |
| STA-01 | Draft config changes survive tab switching without being lost | `localConfig`/`originalConfig` pattern from Settings page preserves drafts across navigation |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Svelte 5 | runes ($state, $derived, $props) | Reactive state management | Project standard, existing codebase |
| shadcn-svelte Tabs | context-based | Tab navigation | Already installed, provides accessible tab UI |
| @tauri-apps/api | invoke() | IPC to Rust backend | Required for config persistence and TTS testing |
| svelte-sonner | toast() | User notifications | Existing in Settings pattern |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|------------|
| $app/state | page.url.searchParams | URL query param persistence | If Claude chooses URL-based tab persistence |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| shadcn-svelte Tabs | Custom tab implementation | Custom would require more code, less accessible; shadcn provides context-based state |
| URL query param | Local state only | URL params survive page reload; local state is simpler but lost on refresh |

**Installation:** No new dependencies required — all libraries already in project.

## Architecture Patterns

### Recommended Project Structure

```
src/lib/components/engine/
├── engine-tabs.svelte          # Tab switcher + save bar + Test button container
├── local-engine.svelte         # CLI backend config (preset, command, args, voice)
├── http-engine.svelte          # HTTP server config (preset, URL, body, headers, timeout, voice)
├── openai-engine.svelte        # OpenAI config (API key, model, voice)
└── elevenlabs-engine.svelte    # ElevenLabs config (API key, voice dropdown, model, format, voice settings)

src/routes/engine/
└── +page.svelte                # Engine page (imports engine-tabs)

src/routes/settings/
└── +page.svelte                # TTS section REMOVED (lines 304-331 deleted)
```

### Pattern 1: Save Bar Pattern (Reuse from Settings)

**What:** Floating bar with "Unsaved changes" + Cancel + Save buttons
**When to use:** Any page with config editing that needs persistence control
**Example:**
```svelte
<script lang="ts">
  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  
  const hasChanges = $derived(
    originalConfig !== null &&
    localConfig !== null &&
    JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );
  
  async function saveConfig() {
    if (!localConfig) return;
    await invoke("set_config", { newConfig: localConfig });
    originalConfig = JSON.parse(JSON.stringify(localConfig));
    toast.success("Settings saved successfully");
  }
  
  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = JSON.parse(JSON.stringify(originalConfig));
  }
</script>

{#if hasChanges}
  <div class="fixed bottom-12 right-4 z-60 border border-border bg-card px-4 py-2.5 flex items-center gap-3 shadow-lg">
    <span class="text-xs text-muted-foreground whitespace-nowrap">Unsaved changes</span>
    <Button size="sm" variant="ghost" onclick={cancelChanges}>Cancel</Button>
    <Button size="sm" onclick={saveConfig}>Save Changes</Button>
  </div>
{/if}
```
**Source:** `src/routes/settings/+page.svelte:86-90, 430-456`

### Pattern 2: State Lifting for Tab Components

**What:** Parent holds `localConfig`, children receive via `$bindable()` or `bind:` directive
**When to use:** When child components need to modify parent state reactively
**Example:**
```svelte
<!-- Parent: engine-tabs.svelte -->
<script lang="ts">
  let localConfig = $state<AppConfig | null>(null);
</script>

<Tabs value={activeTab} onchange={setActiveTab}>
  <TabsList>
    <TabsTrigger value="local">Local (CLI)</TabsTrigger>
    <!-- ... -->
  </TabsList>
  <TabsContent value="local">
    <LocalEngine bind:localConfig />
  </TabsContent>
</Tabs>

<!-- Child: local-engine.svelte -->
<script lang="ts">
  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();
</script>

<Input bind:value={localConfig.tts.command} />
```

### Pattern 3: shadcn-svelte Tabs Usage

**What:** Context-based tab state management
**When to use:** Tab navigation with controlled state
**Example:**
```svelte
<script lang="ts">
  import { Tabs, TabsList, TabsTrigger, TabsContent } from "$lib/components/ui/tabs";
  
  let activeTab = $state("local");
</script>

<Tabs value={activeTab} onchange={(v) => activeTab = v}>
  <TabsList>
    <TabsTrigger value="local">Local (CLI)</TabsTrigger>
    <TabsTrigger value="http">HTTP Server</TabsTrigger>
    <TabsTrigger value="openai">OpenAI</TabsTrigger>
    <TabsTrigger value="elevenlabs">ElevenLabs</TabsTrigger>
  </TabsList>
  
  <TabsContent value="local">
    <LocalEngine bind:localConfig />
  </TabsContent>
  <!-- ... -->
</Tabs>
```
**Source:** `src/lib/components/ui/tabs/tabs.svelte:39-72`

### Anti-Patterns to Avoid

- **Per-backend Test buttons:** CONTEXT.md specifies single shared Test button at bottom of tab container, not per-backend
- **beforeNavigate guard:** CONTEXT.md explicitly rejects navigation guards in favor of save bar pattern
- **Duplicating TTS in Settings:** Hard-delete required, not duplication

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tab navigation | Custom tab component | shadcn-svelte Tabs | Already installed, accessible, context-based state |
| Unsaved changes detection | Deep comparison logic | JSON.stringify comparison | Simple, handles nested objects, already proven in Settings |
| Config persistence | Custom save logic | `invoke("set_config", { newConfig })` + `invoke("get_config")` | Existing IPC pattern, works with Rust Mutex state |
| Toast notifications | Custom alert system | svelte-sonner `toast.success()` / `toast.error()` | Already integrated in Settings |

**Key insight:** The entire tts-settings.svelte component can be extracted into focused pieces — no new patterns needed, just reorganization.

## Common Pitfalls

### Pitfall 1: Losing Draft Changes on Tab Switch
**What goes wrong:** User edits OpenAI API key, switches to Local tab, returns to OpenAI → API key lost
**Why it happens:** Tab components use local state instead of shared parent state
**How to avoid:** Parent (`engine-tabs.svelte`) holds `localConfig`, children receive via `$bindable()`. All edits mutate the same config object.
**Warning signs:** Each tab component has its own `$state()` for config fields

### Pitfall 2: Test Button Uses Saved Config Instead of Draft
**What goes wrong:** User edits API key, clicks Test → test fails because it uses old saved key
**Why it happens:** Test function reads from Rust state instead of `localConfig`
**How to avoid:** Pass `localConfig` to test function, not read from backend. Use `invoke("test_tts_with_config", { config: localConfig })` or similar.
**Warning signs:** Test button ignores unsaved changes in fields

### Pitfall 3: Synthesis Speed Placeholders Remain in Presets
**What goes wrong:** kokoro-tts preset still includes `--speed {speed}` in args
**Why it happens:** Only removing from UI but not from preset definitions
**How to avoid:** Update `CLI_PRESETS` in extracted component AND Rust default config (`src-tauri/src/config/tts.rs:120-133`)
**Warning signs:** Presets show `{speed}` in args_template, HTTP presets mention `{speed}` in hints

### Pitfall 4: Settings Still Imports TtsSettings After Deletion
**What goes wrong:** TypeScript error or runtime crash after removing TTS section
**Why it happens:** Import statement remains after deleting section markup
**How to avoid:** Remove import at line 4 AND section wrapper at lines 304-331 AND settingsCategories array entry (lines 60-64)
**Warning signs:** `import TtsSettings` still present in settings page

## Code Examples

### Extracting CLI Presets (Cleaned)

```typescript
// src/lib/components/engine/local-engine.svelte
const CLI_PRESETS: Record<string, { command: string; args: string[] }> = {
  "kokoro-tts": {
    command: "kokoro-tts",
    args: ["{input}", "{output}", "--voice", "{voice}"],  // REMOVED --speed {speed}
  },
  piper: {
    command: "python3",
    args: [
      "-m", "piper",
      "--data-dir", "{data_dir}",
      "-m", "{voice}",
      "-f", "{output}",
      "--input-file", "{input}"
    ],  // REMOVED --length-scale {length_scale}
  },
  // ... other presets without speed/length_scale
};
```
**Source:** `src/lib/components/settings/tts-settings.svelte:66-93` (modified)

### Updated Rust Default Config

```rust
// src-tauri/src/config/tts.rs:114-133
impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            active_backend: TtsEngine::Local,
            preset: "piper".into(),
            command: "python3".into(),
            args_template: vec![
                "-m".into(),
                "piper".into(),
                "--data-dir".into(),
                "{data_dir}".into(),
                "-m".into(),
                "{voice}".into(),
                // REMOVED: "--length-scale".into(), "{length_scale}".into(),
                "-f".into(),
                "{output}".into(),
                "--input-file".into(),
                "{input}".into(),
            ],
            voice: "en_US-joe-medium".into(),
            // ...
        }
    }
}
```
**Source:** `src-tauri/src/config/tts.rs:114-133` (modified)

### Placeholder Hint Text (Cleaned)

```svelte
<p class="text-xs text-muted-foreground">
  Use {input}, {output}, {voice}, {data_dir}, {text} as placeholders
</p>
```
**Source:** `src/lib/components/settings/tts-settings.svelte:305-308` (modified)

### Engine Page Structure

```svelte
<!-- src/routes/engine/+page.svelte -->
<script lang="ts">
  import EngineTabs from "$lib/components/engine/engine-tabs.svelte";
</script>

<svelte:head>
  <title>Engine - CopySpeak</title>
</svelte:head>

<div class="w-full">
  <section class="scroll-mt-32">
    <div class="border border-border rounded-lg overflow-hidden">
      <div class="p-4 bg-muted/50 border-b border-border">
        <h2 class="text-lg font-semibold font-mono">TTS Engine</h2>
        <p class="text-sm text-muted-foreground">
          Configure your text-to-speech backend and voice settings
        </p>
      </div>
      <div class="p-4">
        <EngineTabs />
      </div>
    </div>
  </section>
</div>
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Monolithic tts-settings.svelte (675 lines) | Split into 5 focused components | Phase 2 | Easier maintenance, clearer separation |
| Synthesis speed via `{speed}` placeholder | Client-side playback speed only | Phase 2 | Simpler mental model, no engine coupling |
| TTS config in Settings | Dedicated Engine page | Phase 2 | Single source of truth, cleaner navigation |

**Deprecated/outdated:**
- `{speed}` placeholder in CLI/HTTP templates: Remove from presets and hints. Playback speed is client-side only.
- `{length_scale}` placeholder in piper preset: Remove from presets. Piper handles this internally.

## Open Questions

1. **Tab Persistence Strategy**
   - What we know: Claude has discretion to choose URL query param or local state
   - What's unclear: Which approach is cleaner for this use case
   - Recommendation: Use **local state** (simpler). URL params add complexity for bookmarking a config page, which has low value. The `active_backend` is already persisted in config, so restoring tab on reload is trivial: read `localConfig.tts.active_backend`.

2. **Test Button Implementation**
   - What we know: Test button exists in tts-settings.svelte:651-673
   - What's unclear: Does `test_tts` command accept config parameter or use saved state?
   - Recommendation: Check `src-tauri/src/commands.rs` for `test_tts` signature. If it reads from saved state, create `test_tts_with_config` variant that accepts `localConfig.tts` as parameter.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Vitest |
| Config file | `vitest.config.ts` |
| Quick run command | `bun run test` |
| Full suite command | `bun run test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENG-01 | User can select TTS backend | unit | `bun run test engine-tabs` | ❌ Wave 0 |
| ENG-02 | User can configure backend-specific settings | unit | `bun run test local-engine http-engine openai-engine elevenlabs-engine` | ❌ Wave 0 |
| ENG-04 | User can select voice from Engine page | unit | `bun run test voice-selection` | ❌ Wave 0 |
| SET-01 | TTS section removed from Settings | integration | Manual verification + grep | ❌ Wave 0 |
| STA-01 | Draft config survives tab switching | unit | `bun run test draft-persistence` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `bun run test` (quick smoke)
- **Per wave merge:** `bun run test` (full suite)
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `src/lib/components/engine/engine-tabs.test.ts` — ENG-01, STA-01 (tab switching, draft persistence)
- [ ] `src/lib/components/engine/local-engine.test.ts` — ENG-02 (CLI config)
- [ ] `src/lib/components/engine/http-engine.test.ts` — ENG-02 (HTTP config)
- [ ] `src/lib/components/engine/openai-engine.test.ts` — ENG-02 (OpenAI config)
- [ ] `src/lib/components/engine/elevenlabs-engine.test.ts` — ENG-02 (ElevenLabs config)
- [ ] `src/routes/settings/+page.svelte` — SET-01 verification via grep for `tts` references

## Sources

### Primary (HIGH confidence)
- `src/routes/settings/+page.svelte` — Save bar pattern, state model
- `src/lib/components/settings/tts-settings.svelte` — All 4 backend configurations, presets, test button
- `src/lib/components/ui/tabs/tabs.svelte` — Tab context implementation
- `src-tauri/src/config/tts.rs` — Rust config types, default args_template
- `.planning/phases/02-engine-page/02-CONTEXT.md` — User decisions

### Secondary (MEDIUM confidence)
- `src/lib/types.ts` — TypeScript config interfaces
- `vitest.config.ts` — Test framework configuration
- `src/test-setup.ts` — Mock patterns for $app/state

### Tertiary (LOW confidence)
- N/A — All findings verified against source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — All libraries already in project, patterns proven in existing code
- Architecture: HIGH — Clear extraction path from tts-settings.svelte, save bar pattern reusable
- Pitfalls: HIGH — Identified from CONTEXT.md and source code analysis

**Research date:** 2026-03-05
**Valid until:** 30 days (stable patterns, Svelte 5 runes stable)
