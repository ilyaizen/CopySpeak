---
phase: quick-4
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src-tauri/src/hud.rs
  - src/lib/components/waveform.svelte
  - src/lib/components/hud-overlay.svelte
  - src/lib/components/settings/hud-settings.svelte
  - src/routes/hud/+page.svelte
  - src-tauri/src/main.rs
  - src-tauri/src/commands.rs
  - src-tauri/src/config.rs
  - src-tauri/tauri.conf.json
  - src-tauri/Cargo.toml
autonomous: true
requirements: []
must_haves:
  truths:
    - "HUD overlay window displays on screen with waveform visualization"
    - "HUD settings are configurable from settings panel"
    - "HUD window appears as transparent always-on-top overlay"
  artifacts:
    - path: "src/lib/components/hud-overlay.svelte"
      provides: "HUD overlay UI component"
      min_lines: 400
    - path: "src/lib/components/waveform.svelte"
      provides: "Audio waveform visualization"
      min_lines: 250
    - path: "src-tauri/src/hud.rs"
      provides: "HUD backend logic and IPC handlers"
      min_lines: 250
    - path: "src-tauri/tauri.conf.json"
      provides: "HUD window configuration"
      contains: '"label": "hud"'
  key_links:
    - from: "src-tauri/src/main.rs"
      to: "src-tauri/src/hud.rs"
      via: "module registration and window creation"
      pattern: "mod hud"
    - from: "src/routes/hud/+page.svelte"
      to: "src/lib/components/hud-overlay.svelte"
      via: "component import"
      pattern: "import.*hud-overlay"
---

<objective>
Bring HUD overlay and waveform visualization features from features-extras branch to main branch.

Purpose: Enable transparent always-on-top HUD window with real-time audio waveform visualization
Output: Working HUD overlay system integrated into main codebase
</objective>

<execution_context>
@./.opencode/get-shit-done/workflows/execute-plan.md
@./.opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
</context>

<tasks>

<task type="auto">
  <name>Cherry-pick HUD commits from features-extras branch</name>
  <files>
    src-tauri/src/hud.rs
    src/lib/components/waveform.svelte
    src/lib/components/hud-overlay.svelte
    src/lib/components/settings/hud-settings.svelte
    src/routes/hud/+page.svelte
    src-tauri/src/main.rs
    src-tauri/src/commands.rs
    src-tauri/src/config.rs
    src-tauri/tauri.conf.json
    src-tauri/Cargo.toml
  </files>
  <action>
    Identify and cherry-pick the commit that added HUD overlay and waveform visualization from features-extras branch.

    Steps:
    1. Find the commit: `git log --oneline features-extras --grep="HUD\|hud\|waveform" --all-match`
    2. If no specific HUD commit found, identify the commit that added the HUD files: `git log --oneline --all -- src-tauri/src/hud.rs src/lib/components/hud-overlay.svelte`
    3. Cherry-pick the commit: `git cherry-pick <commit-hash>`
    4. If merge conflicts occur:
       - Review conflicting files carefully
       - Keep HUD additions from features-extras
       - Preserve any newer changes from main
       - Stage resolved files: `git add <files>`
       - Continue cherry-pick: `git cherry-pick --continue`
    5. If cherry-pick fails completely, use selective file checkout instead:
       - `git checkout features-extras -- src-tauri/src/hud.rs`
       - `git checkout features-extras -- src/lib/components/waveform.svelte`
       - `git checkout features-extras -- src/lib/components/hud-overlay.svelte`
       - `git checkout features-extras -- src/lib/components/settings/hud-settings.svelte`
       - `git checkout features-extras -- src/routes/hud/+page.svelte`
       - Manually merge changes to main.rs, commands.rs, config.rs, tauri.conf.json, and Cargo.toml

    Key files to ensure are present:
    - src-tauri/src/hud.rs (HUD backend logic)
    - src/lib/components/waveform.svelte (Waveform visualization)
    - src/lib/components/hud-overlay.svelte (HUD overlay component)
    - src/lib/components/settings/hud-settings.svelte (HUD settings UI)
    - src/routes/hud/+page.svelte (HUD route page)

    Key integrations to verify:
    - main.rs: `mod hud;` module declaration and HUD window creation
    - commands.rs: HUD-related IPC commands (show_hud, hide_hud, etc.)
    - config.rs: HUD configuration structs (HudPosition, HudPresetPosition, etc.)
    - tauri.conf.json: HUD window configuration with transparent, alwaysOnTop, decorations: false
    - Cargo.toml: tauri-plugin-global-shortcut dependency (if needed for HUD hotkeys)
  </action>
  <verify>
    <automated>
      # Verify all HUD files exist
      test -f src-tauri/src/hud.rs && \
      test -f src/lib/components/waveform.svelte && \
      test -f src/lib/components/hud-overlay.svelte && \
      test -f src/lib/components/settings/hud-settings.svelte && \
      test -f src/routes/hud/+page.svelte && \
      # Verify HUD module is registered in main.rs
      grep -q "mod hud;" src-tauri/src/main.rs && \
      # Verify HUD window config exists in tauri.conf.json
      grep -q '"label": "hud"' src-tauri/tauri.conf.json && \
      echo "All HUD files and configurations verified"
    </automated>
  </verify>
  <done>
    All HUD overlay and waveform visualization files present in main branch with proper module registrations and window configuration
  </done>
</task>

<task type="auto">
  <name>Build and verify HUD integration</name>
  <files>
    src-tauri/src/main.rs
    src-tauri/Cargo.lock
  </files>
  <action>
    Build the project and verify HUD functionality compiles correctly.

    Steps:
    1. Install any new dependencies: `bun install`
    2. Build Rust backend: `cd src-tauri && cargo build`
    3. Fix any compilation errors:
       - Missing imports: Add use statements for HUD types
       - Missing dependencies: Add to Cargo.toml if needed
       - Type mismatches: Update function signatures to match HUD module
    4. Build frontend: `bun run build`
    5. Verify no TypeScript errors in HUD components

    Common issues to watch for:
    - HUD config types not imported in commands.rs
    - HUD module functions not registered in main.rs invoke_handler
    - Missing HUD window creation in main.rs setup
    - Svelte component import paths incorrect
    - Missing HUD-related types in frontend TypeScript
  </action>
  <verify>
    <automated>
      cd src-tauri && cargo check && echo "Rust build successful"
    </automated>
  </verify>
  <done>
    Project builds successfully with HUD overlay and waveform visualization integrated
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
    HUD overlay system with waveform visualization brought from features-extras to main branch, including:
    - Transparent always-on-top HUD window
    - Real-time audio waveform visualization component
    - HUD settings panel in settings UI
    - HUD backend logic and IPC commands
  </what-built>
  <how-to-verify>
    1. Run the app: `bun run tauri dev`
    2. Navigate to Settings and find HUD settings section
    3. Enable HUD overlay in settings
    4. Trigger audio playback (double-copy some text)
    5. Verify HUD window appears as transparent overlay with waveform visualization
    6. Check that HUD settings (position, size, opacity) can be adjusted
    7. Verify HUD window behavior (always-on-top, transparent, no decorations)
  </how-to-verify>
  <resume-signal>Type "approved" if HUD overlay displays correctly with waveform, or describe issues</resume-signal>
</task>

</tasks>

<verification>
- All HUD files present in codebase
- Rust backend compiles without errors
- Frontend builds without TypeScript errors
- HUD window configuration present in tauri.conf.json
- HUD module registered in main.rs
- HUD commands available in commands.rs
</verification>

<success_criteria>
- HUD overlay window displays with waveform visualization
- HUD settings accessible from settings panel
- HUD window appears as transparent always-on-top overlay
- Project builds and runs successfully with HUD integration
</success_criteria>

<output>
After completion, create `.planning/quick/4-bring-hud-overlay-waveform-visualization/4-SUMMARY.md`
</output>
