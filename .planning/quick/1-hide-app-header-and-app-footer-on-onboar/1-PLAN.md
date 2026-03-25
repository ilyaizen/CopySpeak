---
phase: quick-1
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/routes/+layout.svelte
  - src/routes/onboarding/+page.svelte
autonomous: true
requirements: []
must_haves:
  truths:
    - "Onboarding page is fullscreen — no AppHeader or AppFooter visible"
    - "Health check runs on app startup and result is color-coded in the onboarding UI"
    - "All other routes still show AppHeader and AppFooter unchanged"
  artifacts:
    - path: "src/routes/+layout.svelte"
      provides: "Conditional header/footer rendering (hidden on /onboarding)"
    - path: "src/routes/onboarding/+page.svelte"
      provides: "Beautified onboarding page with color-coded health check that runs on mount"
  key_links:
    - from: "src/routes/+layout.svelte"
      to: "page.url.pathname"
      via: "$app/state page store"
      pattern: "page\\.url\\.pathname"
    - from: "src/routes/onboarding/+page.svelte"
      to: "test_tts_engine"
      via: "invoke on onMount"
      pattern: "invoke.*test_tts_engine"
---

<objective>
Make the onboarding page fullscreen by hiding AppHeader and AppFooter on the /onboarding route,
beautify the onboarding page layout, and add a color-coded healthcheck that runs automatically
on startup so new users immediately see their engine status.

Purpose: First-run experience should feel polished and immersive, not crammed inside the app shell.
Output: Updated layout.svelte with conditional chrome, updated onboarding page with health check UI.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/routes/+layout.svelte
@src/routes/onboarding/+page.svelte
@src/lib/components/layout/app-header.svelte
@src/lib/components/layout/app-footer.svelte

<interfaces>
<!-- From src-tauri/src/commands/tts.rs -->
// invoke("test_tts_engine") → TtsHealthResult
interface TtsHealthResult {
  success: boolean;   // true = healthy
  message: string;    // human-readable status
  error_type: string | null; // "not_found" | "api_key_missing" | "auth_failed" | "unavailable" | null
}

// AppFooter already checks page path with window.location.pathname for /hud route suppression
// AppHeader uses `page` from "$app/state" for active nav detection

// page store (from "$app/state"):
//   page.url.pathname — reactive, works in SPA mode
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Hide AppHeader and AppFooter on /onboarding route</name>
  <files>src/routes/+layout.svelte</files>
  <action>
    Import `page` from `"$app/state"` in the script block.
    Derive `isOnboarding` as `$derived(page.url.pathname === "/onboarding")`.
    Wrap `<AppHeader />` with `{#if !isOnboarding}<AppHeader />{/if}`.
    Wrap `<AppFooter />` with `{#if !isOnboarding}<AppFooter />{/if}`.
    When on /onboarding, also remove the `<main>` padding/constraints so the page is truly fullscreen:
    change the `<main>` class to conditionally apply padding — use ternary or class binding:
    `class={isOnboarding ? "flex-1 w-full" : "flex-1 w-full px-4 sm:px-6 py-6 pb-24"}`.
    Do NOT change any other layout behavior, listeners, or lifecycle logic.
  </action>
  <verify>
    <automated>rtk bun run check</automated>
  </verify>
  <done>
    On /onboarding: AppHeader and AppFooter are absent, main fills full height.
    On all other routes: AppHeader and AppFooter render normally.
  </done>
</task>

<task type="auto">
  <name>Task 2: Beautify onboarding page and add color-coded startup healthcheck</name>
  <files>src/routes/onboarding/+page.svelte</files>
  <action>
    Add healthcheck state and auto-run on mount.

    New state variables:
    ```
    let healthResult = $state<{ success: boolean; message: string; error_type: string | null } | null>(null);
    let isCheckingHealth = $state(false);
    ```

    New function `runHealthCheck()`:
    - Set `isCheckingHealth = true`, clear `healthResult`
    - `invoke<{ success: boolean; message: string; error_type: string | null }>("test_tts_engine")`
    - Set `healthResult = result`
    - Set `isCheckingHealth = false`
    - Wrap in try/catch; on error set `healthResult = { success: false, message: String(e), error_type: "unavailable" }`
    - import `invoke` from `"@tauri-apps/api/core"` (already imported in the file)

    Call `runHealthCheck()` inside the existing `onMount` (after `loadDefaultConfig()` resolves — chain with `.then(() => runHealthCheck())`; or just call it sequentially since both are async: await loadDefaultConfig(), then runHealthCheck() without await so it runs in background).

    Health check status UI block — insert between the "Engine Configuration" section and the action buttons:
    ```svelte
    <!-- Health Check Status -->
    {#if isCheckingHealth}
      <div class="flex items-center gap-2 rounded-md border border-border bg-muted/30 px-4 py-3">
        <div class="h-2.5 w-2.5 rounded-full bg-muted animate-pulse"></div>
        <span class="text-sm text-muted-foreground">Checking engine...</span>
      </div>
    {:else if healthResult}
      <div class="flex items-center gap-2 rounded-md border px-4 py-3 {healthResult.success
        ? 'border-green-500/40 bg-green-500/10'
        : 'border-destructive/40 bg-destructive/10'}">
        <div class="h-2.5 w-2.5 rounded-full flex-shrink-0 {healthResult.success
          ? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]'
          : 'bg-destructive shadow-[0_0_8px_rgba(239,68,68,0.6)]'}"></div>
        <span class="text-sm {healthResult.success ? 'text-green-700 dark:text-green-400' : 'text-destructive'}">
          {healthResult.message}
        </span>
      </div>
    {/if}
    ```

    Beautification changes to the outer wrapper and card:
    - Outer div: change `bg-background` to use a subtle gradient — `class="min-h-screen flex items-center justify-center p-4 sm:p-6 bg-gradient-to-br from-background to-muted/30"`
    - Card: add `shadow-lg` to the card div class alongside existing classes
    - Title h1: add `bg-gradient-to-r from-foreground to-foreground/70 bg-clip-text` for a subtle gradient text effect (keep existing classes)
    - Keep the overall structure (card, sections, buttons) unchanged — only enhance aesthetics

    Use `import { invoke } from "@tauri-apps/api/core"` — note the file currently imports from `"@tauri-apps/api/core"` (line 4), so it's already correct.
  </action>
  <verify>
    <automated>rtk bun run check</automated>
  </verify>
  <done>
    On /onboarding mount: health check runs automatically, shows pulsing indicator while running,
    then shows green (success) or red (failure) color-coded pill with the engine status message.
    Page has gradient background and shadow card — visually improved over the flat version.
  </done>
</task>

</tasks>

<verification>
1. `bun run check` passes with no TypeScript/Svelte errors
2. Launch app fresh (no config): onboarding page appears fullscreen — no header nav, no footer status bar
3. Health check pill appears automatically within seconds of page load
4. Green pill with glow when engine is healthy; red pill with glow when engine is unavailable
5. After completing/skipping onboarding, navigate to "/" — AppHeader and AppFooter reappear normally
</verification>

<success_criteria>
- /onboarding route: AppHeader absent, AppFooter absent, main fills full height with no padding
- Health check invokes `test_tts_engine` on mount, result is color-coded (green/red) with status message
- All other routes: AppHeader and AppFooter present, main padding unchanged
- `bun run check` clean
</success_criteria>

<output>
After completion, create `.planning/quick/1-hide-app-header-and-app-footer-on-onboar/1-SUMMARY.md`
</output>
