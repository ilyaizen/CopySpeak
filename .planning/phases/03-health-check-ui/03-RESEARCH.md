# Phase 3: Health Check UI - Research

**Researched:** 2026-03-05
**Domain:** TTS Engine Health Check UI for Engine Page
**Confidence:** HIGH

## Summary

Phase 3 implements inline health check testing for each TTS backend on the Engine page. Users can test their TTS engine directly from each backend configuration section, seeing immediate inline feedback about whether their engine is working and exactly what's wrong if it isn't.

The `test_tts_engine` IPC command already exists with comprehensive error type categorization. All backends implement `health_check()` methods that return categorized errors (command not found, API key missing, auth failure, rate limit, permission denied, etc.). The phase requires adding per-backend test buttons that call this IPC command, displaying inline alert banners with success/failure feedback, and showing install guidance cards for CLI backends when commands are missing.

**Primary recommendation:** Use the existing `test_tts_engine` IPC command with shadcn-svelte Alert component for inline feedback, adding test buttons to each backend component file (local-engine.svelte, elevenlabs-engine.svelte, openai-engine.svelte, http-engine.svelte) with conditional visibility based on `localConfig.tts.active_backend`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Test Button Placement
- **Per-backend test buttons** — Each backend section (CLI, ElevenLabs, OpenAI, HTTP) has its own "Test Engine" button
- Button only appears/active when that backend is the **currently selected backend**
- Positioned near the backend's configuration fields for contextual relevance

#### Test Content
- Uses the **user's configured voice** from that backend
- Reads a **default sample phrase** ("This is a test of the text-to-speech engine.")
- Provides realistic validation — same voice/audio pipeline the app uses during clipboard triggering

#### Feedback UI
- **Inline alert banner** appears in the same backend section as the test button
- Success: Green banner with checkmark and "Engine is working" message
- Failure: Red banner with error type and actionable message
- No toasts, no modals — feedback stays where user configures

#### Error Messaging
- **All backend errors** get specific, human-readable messages:
  - Command not found → "kokoro-tts not found. Install CLI backend."
  - Auth failure → "Invalid API key. Check your ElevenLabs credentials."
  - Network error → "Cannot connect to OpenAI. Check your internet connection."
  - Timeout → "Request timed out. Server may be unavailable."
  - Parsing error → "Invalid response format from HTTP endpoint."
  - etc.

#### Install Guidance
- **Inline install card** appears below the error when the cause is a missing dependency
- Shows copy-paste command (e.g., `pip install kokoro-tts`)
- Includes a copy button for convenience
- Only shows for CLI backends with clear install paths (kokoro-tts, piper)

### Claude's Discretion

Implementation details for:
- Svelte 5 component structure for test buttons and alert banners
- Error message mapping from Rust error types to human-readable strings
- Install command lookup table for CLI backends
- Loading state handling during health check execution
- Copy-to-clipboard implementation for install commands

### Deferred Ideas (OUT OF SCOPE)

None explicitly marked in CONTEXT.md.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ENG-03 | User can test the engine and see pass/fail with specific error diagnosis (command not found, auth failure, stderr, etc.) | `test_tts_engine` IPC command returns `TtsHealthResult` with `error_type` field. All backends implement `health_check()` with categorized error types. Alert component available for inline feedback. |
| ENG-05 | Engine page shows inline install guidance when engine is broken (e.g. `pip install kokoro-tts` with copy button) | CLI backends have documented install commands. Install guidance can be shown when `error_type === "not_found"` for CLI backend. Copy-to-clipboard pattern exists in codebase. |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| @tauri-apps/api | ^2 | IPC communication with Rust backend (`invoke` command) | Tauri's official frontend API for Rust interop |
| svelte-sonner | ^1.0.7 | Toast notifications (existing in project) | Already imported and used for error/success toasts |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| shadcn-svelte | latest | Alert component (alert.svelte, alert-title.svelte, alert-description.svelte) | Inline feedback banners |
| @lucide/svelte | ^0.561.0 | Icons (CheckCircle, XCircle, Copy) | Visual indicators for success/error states |
| tailwind-variants | ^3.2.2 | Component variants system | Alert variant management |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| shadcn-svelte Alert | Custom div with Tailwind classes | shadcn-svelte provides consistent styling, accessibility, and project convention |

**Installation:**
```bash
# shadcn-svelte alert component already installed
# All other dependencies are in package.json
```

## Architecture Patterns

### Recommended Component Structure
```
src/lib/components/engine/
├── engine-tabs.svelte           # Parent component (move test button state here)
├── local-engine.svelte          # Add test button + alert + install card
├── elevenlabs-engine.svelte     # Add test button + alert
├── openai-engine.svelte         # Add test button + alert
├── http-engine.svelte          # Add test button + alert
└── health-test/               # New shared component (optional)
    ├── test-button.svelte       # Reusable test button with loading state
    ├── test-alert.svelte        # Reusable alert banner component
    └── install-card.svelte     # Reusable install guidance card
```

### Pattern 1: Per-Backend Test Button
**What:** Add "Test Engine" button to each backend component with conditional visibility
**When to use:** When implementing inline health check for a backend
**Example:**
```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Alert, AlertTitle, AlertDescription } from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { CheckCircle, XCircle } from "@lucide/svelte";
  import type { AppConfig } from "$lib/types";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  let isTesting = $state(false);
  let testResult = $state<{ success: boolean; message: string; error_type?: string } | null>(null);

  const isActiveBackend = $derived(localConfig.tts.active_backend === "local");

  async function testEngine() {
    isTesting = true;
    testResult = null;
    try {
      const result = await invoke("test_tts_engine") as {
        success: boolean;
        message: string;
        error_type?: string;
      };
      testResult = result;
    } catch (e) {
      testResult = { success: false, message: String(e) };
    } finally {
      isTesting = false;
    }
  }
</script>

<div class="space-y-4">
  <!-- Existing config fields here -->

  {#if isActiveBackend}
    <Button
      variant="outline"
      onclick={testEngine}
      disabled={isTesting}
    >
      {#if isTesting}
        Testing...
      {:else}
        Test Engine
      {/if}
    </Button>

    {#if testResult}
      <Alert variant={testResult.success ? "default" : "destructive"}>
        {#if testResult.success}
          <CheckCircle class="text-emerald-600" />
        {:else}
          <XCircle />
        {/if}
        <AlertTitle>
          {testResult.success ? "Engine is working" : "Engine test failed"}
        </AlertTitle>
        <AlertDescription>
          {testResult.message}
        </AlertDescription>
      </Alert>
    {/if}
  {/if}
</div>
```

### Pattern 2: Install Guidance Card
**What:** Show install card with copy button when CLI command not found
**When to use:** When `error_type === "not_found"` and backend is CLI
**Example:**
```svelte
{#if testResult && !testResult.success && testResult.error_type === "not_found"}
  <div class="mt-4 rounded-lg border border-border bg-card p-4">
    <h4 class="text-sm font-medium mb-2">Install TTS Engine</h4>
    <div class="flex items-center gap-2">
      <code class="flex-1 rounded bg-muted px-2 py-1.5 text-sm font-mono">
        pip install kokoro-tts
      </code>
      <Button
        variant="ghost"
        size="icon-sm"
        onclick={async () => {
          await navigator.clipboard.writeText("pip install kokoro-tts");
        }}
      >
        <Copy class="size-4" />
      </Button>
    </div>
    <p class="text-xs text-muted-foreground mt-2">
      Install the TTS engine, then test again.
    </p>
  </div>
{/if}
```

### Pattern 3: Error Message Mapping
**What:** Map Rust error types to human-readable messages
**When to use:** When displaying test results
**Example:**
```typescript
const ERROR_MESSAGES: Record<string, string> = {
  api_key_missing: "API key is missing or invalid",
  auth_failed: "Authentication failed. Check your API key.",
  rate_limit: "Rate limit exceeded. Please try again later.",
  http_error: "Network error. Check your internet connection.",
  not_found: "Command not found. Install the TTS engine.",
  permission_denied: "Permission denied. Check file permissions.",
  unavailable: "Engine unavailable. Check configuration.",
  io_error: "I/O error. Check file paths and permissions.",
  unknown: "Unknown error. Check logs for details.",
};

function getErrorMessage(errorType?: string): string {
  return errorType ? ERROR_MESSAGES[errorType] || ERROR_MESSAGES.unknown : "Unknown error";
}
```

### Anti-Patterns to Avoid
- **Using toasts for test results:** Context.md specifies no toasts, use inline alerts instead
- **Placing test button at bottom of page:** Context.md specifies per-backend test buttons in each section
- **Showing test button for inactive backends:** Button should only appear for the currently selected backend
- **Generic error messages:** All backend errors should have specific, actionable messages as per Context.md
- **Showing install guidance for non-CLI backends:** Only show for CLI backends with clear install paths

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Alert component | Custom div with Tailwind classes | shadcn-svelte Alert component | Provides consistent styling, accessibility attributes, and project convention |
| Error message mapping | Hardcoded if-else chains | Lookup table (Record) | Easier to maintain, add new error types, and test |
| Copy to clipboard | Custom clipboard logic | `navigator.clipboard.writeText()` | Browser-native API, no external dependencies needed |
| Loading state | Custom spinner logic | Svelte 5 reactive state (`isTesting`) | Simple and idiomatic Svelte pattern |

**Key insight:** Reusing existing UI components and patterns reduces code duplication and ensures consistency across the application.

## Common Pitfalls

### Pitfall 1: Test Button Always Visible
**What goes wrong:** Test button appears in all backend sections regardless of which backend is active
**Why it happens:** Missing conditional check for `localConfig.tts.active_backend`
**How to avoid:** Wrap test button in `{#if isActiveBackend}` block using derived state
**Warning signs:** Test button appears in backend sections that are not the currently selected backend

### Pitfall 2: Toast Instead of Inline Alert
**What goes wrong:** Test results show in toast notifications instead of inline banners
**Why it happens:** Using existing `toast.success()` and `toast.error()` pattern from codebase
**How to avoid:** Use shadcn-svelte Alert component inline in the same section as the test button
**Warning signs:** Feedback appears in top-right corner instead of near the test button

### Pitfall 3: Generic Error Messages
**What goes wrong:** All errors show the same "Test failed" message without specifics
**Why it happens:** Not using the `error_type` field from `TtsHealthResult`
**How to avoid:** Map each `error_type` to a specific, actionable message using lookup table
**Warning signs:** Error message doesn't mention the specific problem (missing command, API key, network, etc.)

### Pitfall 4: Missing Install Guidance
**What goes wrong:** CLI backend shows "not found" error but no install guidance
**Why it happens:** Not checking for `error_type === "not_found"` for CLI backends
**How to avoid:** Add install guidance card when error type is `not_found` and backend has documented install command
**Warning signs:** User sees error message but doesn't know how to fix it

### Pitfall 5: Test Button Disabled When Not Needed
**What goes wrong:** Test button disabled even when backend is active and configured
**Why it happens:** Incorrect disabled state logic
**How to avoid:** Only disable button when `isTesting` is true
**Warning signs:** Test button grayed out when it should be clickable

### Pitfall 6: Incorrect Preset Detection for Install Commands
**What goes wrong:** Wrong install command shown for CLI preset (e.g., showing piper install for kokoro-tts)
**Why it happens:** Using `localConfig.tts.preset` instead of checking `localConfig.tts.command`
**How to avoid:** Map install commands to `command` field values (e.g., `"kokoro-tts"` → `pip install kokoro-tts`)
**Warning signs:** Install command doesn't match the selected preset

## Code Examples

### Common Operation 1: Test Engine with Error Handling
**Source:** Existing codebase pattern (`invoke()` with try-catch)
```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  async function testEngine() {
    isTesting = true;
    testResult = null;
    try {
      const result = await invoke("test_tts_engine") as {
        success: boolean;
        message: string;
        error_type?: string;
      };
      testResult = result;
    } catch (e) {
      testResult = { success: false, message: String(e) };
    } finally {
      isTesting = false;
    }
  }
</script>
```

### Common Operation 2: Copy to Clipboard
**Source:** Existing codebase pattern (`navigator.clipboard.writeText()`)
```svelte
<script lang="ts">
  let { copyCommand }: { copyCommand: string } = $props();

  async function copyInstallCommand() {
    try {
      await navigator.clipboard.writeText(copyCommand);
      // Optional: Show brief feedback
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  }
</script>

<Button variant="ghost" size="icon-sm" onclick={copyInstallCommand}>
  <Copy class="size-4" />
</Button>
```

### Common Operation 3: Conditional Backend Visibility
**Source:** Existing codebase pattern (derived state for active backend)
```svelte
<script lang="ts">
  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  const isActiveBackend = $derived(
    localConfig.tts.active_backend === "local"
  );
</script>

{#if isActiveBackend}
  <Button onclick={testEngine}>Test Engine</Button>
{/if}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Central test button at bottom | Per-backend test buttons in each section | This phase | Feedback appears where user configures, better UX |
| Toast notifications | Inline alert banners | This phase | No toasts, feedback stays in context |
| Generic error messages | Specific, actionable error messages per error type | This phase | Users know exactly what's wrong and how to fix |
| No install guidance | Inline install cards with copy button | This phase | Reduces friction for CLI backend setup |

**Deprecated/outdated:**
- `test_tts_engine` IPC command is current and working — no changes needed
- `TtsHealthResult` struct is current with all needed fields

## Open Questions

None — all requirements and constraints are clear from CONTEXT.md and existing codebase.

## Sources

### Primary (HIGH confidence)
- `src-tauri/src/commands/tts.rs` - `test_tts_engine` command implementation, `TtsHealthResult` struct, error type categorization (lines 52-133)
- `src-tauri/src/tts/mod.rs` - `TtsBackend` trait with `health_check()` method (lines 1-53)
- `src-tauri/src/tts/cli.rs` - CLI backend health check implementation, error handling (lines 333-354)
- `src-tauri/src/tts/openai.rs` - OpenAI backend health check implementation (lines 117-127)
- `src-tauri/src/tts/elevenlabs.rs` - ElevenLabs backend health check implementation (lines 403-414)
- `src-tauri/src/tts/http.rs` - HTTP backend health check implementation (lines 178-203)
- `src/lib/components/engine/engine-tabs.svelte` - Existing test button implementation (lines 129-144, 209-232)
- `src/lib/components/engine/local-engine.svelte` - CLI backend configuration component
- `src/lib/components/engine/elevenlabs-engine.svelte` - ElevenLabs backend configuration component
- `src/lib/components/engine/openai-engine.svelte` - OpenAI backend configuration component
- `src/lib/components/engine/http-engine.svelte` - HTTP backend configuration component
- `src/lib/components/ui/alert/alert.svelte` - Alert component with variants (default, destructive)
- `src/lib/components/ui/button/button.svelte` - Button component with variants (default, outline, ghost, destructive)
- `src/lib/types.ts` - TypeScript type definitions including `TtsEngine`, `TtsConfig`, etc.

### Secondary (MEDIUM confidence)
- `docs_internal/tts_backends.md` - CLI backend installation commands (pip install commands for kokoro-tts, piper, chatterbox, coqui-tts, espeak, edge-tts)
- `docs_internal/architecture.md` - Project architecture and IPC command documentation

### Tertiary (LOW confidence)
- None — all findings verified from primary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies and components verified from package.json and codebase
- Architecture: HIGH - Existing codebase patterns and IPC command structure verified from source files
- Pitfalls: HIGH - Identified from CONTEXT.md constraints and existing codebase anti-patterns

**Research date:** 2026-03-05
**Valid until:** 2026-04-04 (30 days for stable project dependencies)
