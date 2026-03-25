# Phase 2: Engine Page - Context

**Gathered:** 2026-03-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Move all TTS engine configuration from Settings to the dedicated Engine page. The Engine page becomes the single source of truth for backend selection, credentials, and voice configuration. TTS settings are hard-deleted from Settings (not duplicated). Draft config changes survive tab switching via save bar pattern (no prompt, just unsaved indicator + Save/Cancel).

</domain>

<decisions>
## Implementation Decisions

### Page Layout

**Navigation Style**
- **Decision:** Dropdown select for backend (NOT tabbed layout, NOT sidebar)
- **Location:** Top of page, above all configuration sections
- **Style:** Full width dropdown, matches Settings dropdown pattern
- **Behavior:** Active backend is the primary thing for Engine page
- **Rationale:** Simpler than tabs/sidebar, matches existing TTS settings pattern

**Content Sections**
- **Decision:** Split sections for backend configuration (NOT single big card)
- **Styling:** Same as Settings cards — border, rounded corners, muted header with title + description
- **Example:** For Local backend: "Engine Preset" section, "Command & Arguments" section, "Voice Selection" section
- **Rationale:** Consistent with Settings page visual language

**Backend Switching**
- **Decision:** Warn before switching backends with unsaved changes
- **Behavior:** Show "Unsaved changes" warning before dropdown changes backend
- **Rationale:** Prevents accidental data loss when switching backends

**Test Engine Button**
- **Location:** Bottom of last configuration section (NOT fixed position)
- **Rationale:** Natural flow — configure, then test at end

### Locked Decisions (User-Specified)

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

### Claude's Discretion

- **Dropdown component:** Use existing Select from shadcn-svelte (same as Settings)
- **Section organization:** How many sections per backend (2-4 sections reasonable)
- **Section order:** Logical flow (preset → credentials/command → voice)
- **Warning dialog:** Reuse Settings' pattern or create simpler confirmation
- **Exact spacing and typography:** Follow Settings card spacing

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

**From Settings page (`src/routes/settings/+page.svelte`):**
- Save bar pattern (lines 364-389): Fixed bottom-right bar with "Unsaved changes" + Cancel + Save
- State model: `localConfig` / `originalConfig` with `hasChanges` derived from JSON comparison
- `saveConfig()` / `cancelChanges()` / `loadConfig()` pattern — directly reusable
- Toast notifications via `svelte-sonner`
- Card section pattern (lines 232-263): Border, rounded corners, muted header with title + description

**From `tts-settings.svelte` (675 lines):**
- All 4 backend configurations already implemented
- Backend dropdown (lines 242-252) — can be reused
- Preset dropdowns with auto-fill logic (`CLI_PRESETS`, `HTTP_PRESETS`)
- ElevenLabs voice fetching via `invoke("list_elevenlabs_voices")`
- Test Engine button with health check (`testTtsEngine()` function)
- Backend-specific conditional rendering (lines 254-649)

### Cleanup Required (Synthesis Speed Removal)

**In `tts-settings.svelte`:**
- Line 69: `args: ["{input}", "{output}", "--speed", "{speed}", "--voice", "{voice}"]` — kokoro-tts preset
- Line 75: `args_template` for piper includes `"{length_scale}"` placeholder
- Line 79: chatterbox preset has `"--voice", "{voice}"`
- Lines 306-308: Placeholder hint text mentions `{speed}`
- Line 362: HTTP placeholder hint mentions `{speed}`
- **Action:** Remove `{speed}` and `{length_scale}` from all presets and placeholder hints

**In Rust (verify and remove if present):**
- Check `src-tauri/src/config/tts.rs` for `{length_scale}` in default args_template
- Check TTS command builders for speed placeholder handling
- **Note:** Placeholder removal is frontend-only if Rust just passes through args_template

### Integration Points

- **Engine route:** `src/routes/engine/+page.svelte` (stub exists from Phase 1)
- **Config types:** `src/lib/types.ts` — `TtsConfig`, `HttpTtsConfig`, `AppConfig`
- **Rust commands:** `get_config`, `set_config`, `test_tts`, `list_elevenlabs_voices` (all exist)
- **Settings deletion:** Remove TTS section from `src/routes/settings/+page.svelte` after Engine page is complete

### File Structure After Phase 2

```
src/routes/engine/
└── +page.svelte                # Engine page (dropdown + sections)

src/routes/settings/
└── +page.svelte                # TTS section REMOVED (hard delete)
```

**Note:** Component extraction from `tts-settings.svelte` is now optional. The original 675-line file can be used as a template, but reorganized with split sections instead of tabs.

</code_context>

<specifics>
## Specific Requirements

### Must-Have Behaviors

1. **Backend dropdown at top** — First thing user sees, above all configuration sections
2. **Dropdown full width** — Spans entire card width, matches Settings dropdowns
3. **Split sections per backend** — Each backend has 2-4 sections (not one big form)
4. **Section cards match Settings** — Border, rounded corners, muted header with title + description
5. **Warn on backend switch** — Show warning before switching backends if unsaved changes exist
6. **Draft survives navigation** — Edit backend config, switch to Play, return → config still in field (not saved until Save clicked)
7. **Save bar appears on any change** — Any field edit shows the unsaved bar
8. **Cancel reverts to last saved** — Cancel button restores `originalConfig` state
9. **Test Engine at bottom** — Test button in last section of active backend
10. **Test Engine works with unsaved config** — Test button uses `localConfig` (draft), not `originalConfig`
11. **Settings page has NO TTS section** — Grep audit confirms zero `tts.` references in settings route

### Synthesis Speed Cleanup

- Remove `{speed}` placeholder from kokoro-tts preset
- Remove `{length_scale}` from piper preset
- Update placeholder hint text to only list `{input}`, `{output}`, `{voice}`, `{data_dir}`, `{text}`
- Verify no synthesis speed fields exist (none found in current code)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-engine-page*
*Context gathered: 2026-03-05*
