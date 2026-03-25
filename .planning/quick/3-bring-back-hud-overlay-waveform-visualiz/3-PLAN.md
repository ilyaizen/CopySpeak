---
phase: quick-03
plan: 01
type: execute
wave: 1
depends_on: []
files_modified: [.planning/PROJECT.md]
autonomous: true
requirements: []
must_haves:
  truths:
    - "HUD overlay documentation no longer marks feature as deferred"
    - "HUD feature is documented as available in current milestone"
  artifacts:
    - path: ".planning/PROJECT.md"
      provides: "Project scope documentation"
      contains: "Out of Scope section"
  key_links:
    - from: ".planning/PROJECT.md"
      to: "HUD implementation"
      via: "documentation reference"
      pattern: "HUD overlay.*waveform"
---

<objective>
Remove deferred status from HUD overlay / waveform visualization feature

Purpose: The HUD overlay feature is already fully implemented in the codebase but marked as deferred in documentation. This plan removes the deferred status to make the feature officially available.
Output: Updated PROJECT.md with HUD removed from Out of Scope section
</objective>

<execution_context>
@./.opencode/get-shit-done/workflows/execute-plan.md
@./.opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/STATE.md

# Current State
- HUD overlay fully implemented in codebase:
  - src/lib/components/hud-overlay.svelte (HUD component)
  - src/lib/components/waveform.svelte (waveform visualization)
  - src/lib/components/settings/hud-settings.svelte (settings UI)
  - src/routes/hud/+page.svelte (HUD route)
  - src-tauri/src/hud.rs (backend logic)
  - src-tauri/tauri.conf.json (window config)
- Feature controlled by config.hud.enabled flag
- Only documentation marks it as deferred (PROJECT.md line 49)
</context>

<tasks>

<task type="auto">
  <name>Remove HUD from Out of Scope section</name>
  <files>.planning/PROJECT.md</files>
  <action>
    Edit `.planning/PROJECT.md` to remove the HUD overlay line from the "Out of Scope" section (line 49).

    Change from:
    ```
    - HUD overlay / waveform visualization — deferred to features-extras branch
    ```

    To: (remove this line entirely)

    Keep all other items in the Out of Scope section unchanged. The HUD is fully implemented and functional, so it should not be listed as out of scope.
  </action>
  <verify>
    <automated>grep -n "HUD overlay.*deferred" .planning/PROJECT.md</automated>
    <manual>Command should return no matches, confirming the line was removed</manual>
    <sampling_rate>run after this task commits</sampling_rate>
  </verify>
  <done>HUD overlay line removed from Out of Scope section in PROJECT.md</done>
</task>

</tasks>

<verification>
- HUD overlay no longer listed as deferred in PROJECT.md
- Other out-of-scope items remain unchanged
- Documentation accurately reflects implemented features
</verification>

<success_criteria>
- PROJECT.md updated with HUD removed from Out of Scope section
- Feature is no longer marked as deferred
- All other documentation unchanged
</success_criteria>

<output>
After completion, create `.planning/quick/3-bring-back-hud-overlay-waveform-visualiz/quick-03-SUMMARY.md`
</output>
