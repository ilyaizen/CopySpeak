# CopySpeak Development Guide

> **Version:** v0.2.0
> **Last Updated:** 2026-02-24
> **Note:** Six features deferred to `features-extras` branch (2026-02-24). See roadmap.md for details.

---

## Table of Contents

- [CopySpeak Development Guide](#copyspeak-development-guide)
  - [Table of Contents](#table-of-contents)
  - [Purpose](#purpose)
  - [Quick Start](#quick-start)
  - [Prerequisites](#prerequisites)
    - [Required Tools](#required-tools)
    - [Optional Tools](#optional-tools)
  - [Project Structure](#project-structure)
  - [Development Workflows](#development-workflows)
    - [Frontend Development](#frontend-development)
    - [Full Application Development](#full-application-development)
    - [Type Checking](#type-checking)
    - [Testing](#testing)
  - [Coding Standards](#coding-standards)
    - [General Principles](#general-principles)
    - [Formatting](#formatting)
      - [Svelte-Specific Rules (`.svelte` files)](#svelte-specific-rules-svelte-files)
      - [TypeScript/JavaScript Rules](#typescriptjavascript-rules)
      - [AI Model Formatting Requirements](#ai-model-formatting-requirements)
    - [Naming Conventions](#naming-conventions)
    - [Commenting Standards](#commenting-standards)
      - [What to Comment](#what-to-comment)
      - [Commenting Guidelines](#commenting-guidelines)
      - [Examples](#examples)
    - [Svelte 5 Specific](#svelte-5-specific)
      - [Rune Scope](#rune-scope)
      - [Event Handlers](#event-handlers)
      - [Avoiding Reactive Loops](#avoiding-reactive-loops)
      - [Performance in Loops](#performance-in-loops)
      - [Svelte 5 Migration](#svelte-5-migration)
      - [Component Patterns](#component-patterns)
      - [Example Svelte 5 Component](#example-svelte-5-component)
    - [TypeScript Rules](#typescript-rules)
      - [Type System](#type-system)
      - [Functions](#functions)
      - [Imports \& Exports](#imports--exports)
      - [Lucide Icons](#lucide-icons)
      - [Null \& Error Handling](#null--error-handling)
      - [Patterns to Prefer](#patterns-to-prefer)
      - [What to Avoid](#what-to-avoid)
  - [Tauri IPC Pattern](#tauri-ipc-pattern)
    - [Defining Commands (Rust)](#defining-commands-rust)
    - [Registering Commands](#registering-commands)
    - [Calling from Frontend](#calling-from-frontend)
  - [Multi-Page Vite Configuration](#multi-page-vite-configuration)
  - [Configuration System](#configuration-system)
    - [Where Configuration Lives](#where-configuration-lives)
    - [Changing the Default TTS Engine](#changing-the-default-tts-engine)
  - [Configuration System](#configuration-system-1)
    - [Where Configuration Lives](#where-configuration-lives-1)
    - [Changing the Default TTS Engine](#changing-the-default-tts-engine-1)
    - [Adding a New Config Option](#adding-a-new-config-option)
  - [TTS Backend Extension](#tts-backend-extension)
    - [Adding a New Backend](#adding-a-new-backend)
  - [Content Filtering](#content-filtering)
    - [Filter Rules (Deferred)](#filter-rules-deferred)
    - [How to Access (Deferred Feature)](#how-to-access-deferred-feature)
  - [Speech History](#speech-history)
    - [History Structure](#history-structure)
    - [TTS Audio Caching and Storage](#tts-audio-caching-and-storage)
    - [Accessing History](#accessing-history)
  - [Audio Save Mode](#audio-save-mode)
    - [Filename Templates](#filename-templates)
    - [Enabling Save Mode](#enabling-save-mode)
  - [Common Tasks](#common-tasks)
    - [Adding a New IPC Command](#adding-a-new-ipc-command)
    - [Adding a New UI Component](#adding-a-new-ui-component)
      - [Decision Tree: shadcn-svelte vs Custom](#decision-tree-shadcn-svelte-vs-custom)
      - [Installing shadcn-svelte Components](#installing-shadcn-svelte-components)
      - [shadcn-svelte Best Practices](#shadcn-svelte-best-practices)
      - [Creating Custom Components](#creating-custom-components)
      - [Key Resources](#key-resources)
    - [Debugging Clipboard Issues](#debugging-clipboard-issues)
    - [Adding a New TTS Backend](#adding-a-new-tts-backend)
  - [Troubleshooting](#troubleshooting)
    - [Common Issues](#common-issues)
    - [Dev Tools](#dev-tools)
    - [Important Agent Rules](#important-agent-rules)
    - [Available Skills](#available-skills)
    - [Using AI Agent Teams](#using-ai-agent-teams)
      - [When to Use Teams](#when-to-use-teams)
      - [Agent Types Available](#agent-types-available)
      - [Example: Multi-Perspective Code Review](#example-multi-perspective-code-review)
      - [Team Coordination Pattern](#team-coordination-pattern)
      - [Best Practices for AI-Assisted Development](#best-practices-for-ai-assisted-development)
    - [Project-Specific AI Context](#project-specific-ai-context)
    - [Example Workflows](#example-workflows)
      - [Workflow 1: Security Hardening](#workflow-1-security-hardening)
      - [Workflow 2: Performance Optimization](#workflow-2-performance-optimization)
    - [Limitations and Considerations](#limitations-and-considerations)
    - [Getting Help with Claude Code](#getting-help-with-claude-code)
  - [Changelog Maintenance](#changelog-maintenance)
  - [Release Process](#release-process)
    - [Automated Release (Recommended)](#automated-release-recommended)
    - [Release Requirements](#release-requirements)
    - [Troubleshooting Releases](#troubleshooting-releases)
      - [`latest.json` not generated /404 error on update check](#latestjson-not-generated-404-error-on-update-check)
      - [Release notes show fallback text instead of changelog](#release-notes-show-fallback-text-instead-of-changelog)
      - [Release created as draft](#release-created-as-draft)
    - [Manual Release (Emergency)](#manual-release-emergency)
    - [Key Rotation](#key-rotation)

---

## Purpose

This document provides guidelines for contributing to the project.

---

## Quick Start

```bash
# Install frontend dependencies (Svelte 5 + Tailwind)
bun install

# Run Svelte frontend only (for UI development)
bun run dev

# Run full Tauri app (requires Rust toolchain)
bun run tauri dev

# Build for production
bun run tauri build
```

---

## Prerequisites

### Required Tools

| Tool           | Version        | Purpose                          |
| -------------- | -------------- | -------------------------------- |
| **Rust**       | Latest stable  | Backend compilation              |
| **Bun**        | 1.3+           | Package management, frontend dev |
| **Windows 11** | Primary target | Platform-specific APIs           |

### Optional Tools

| Tool           | Purpose                                         |
| -------------- | ----------------------------------------------- |
| **kokoro-tts** | Local TTS engine (optional if using Cloud APIs) |
| **OpenAI API** | Cloud TTS provider                              |
| **ElevenLabs** | Premium Cloud TTS provider                      |
| **VS Code**    | Recommended IDE with Svelte/Rust extensions     |

---

## Project Structure

```
copyspeak/
├── docs_internal/           # Internal documentation (you are here)
├── docs/                    # Public documentation
├── src/                     # Frontend (Svelte 5 + SvelteKit)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── ui/         # shadcn-svelte components
│   │   │   ├── settings/   # Settings panel components
│   │   │   └── *.svelte    # Other components
│   │   └── utils.ts        # Utilities
│   └── routes/             # SvelteKit routes
├── src-tauri/               # Backend (Rust + Tauri v2)
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── clipboard.rs    # Clipboard monitoring
│   │   ├── autostart.rs    # Windows startup
│   │   ├── history.rs      # Speech history
│   │   ├── history_manager.rs # History management
│   │   ├── pagination.rs    # Text pagination
│   │   ├── fragment_queue.rs # Fragment queue
│   │   ├── logging.rs      # Application logging
│   │   ├── audio/          # Audio playback (directory-based)
│   │   ├── commands/        # IPC commands (directory-based)
│   │   ├── config/         # Configuration (directory-based)
│   │   ├── sanitize/       # Text normalization (directory-based)
│   │   └── tts/            # TTS backends (directory-based)
│   ├── capabilities/       # Tauri security permissions
│   └── tauri.conf.json     # Tauri configuration
├── index.html               # Main window entry
├── vite.config.js           # Vite single-page config
├── CLAUDE.md                # AI assistant context file
└── file-structure.txt       # Complete file structure reference

**Deferred Modules** (available on `features-extras` branch):
├── src-tauri/src/hud.rs        # HUD window
├── src-tauri/src/filter.rs     # Content filtering
├── src-tauri/src/language.rs   # Language detection
├── src-tauri/src/app_source.rs # Application filter
├── hud.html                    # HUD window entry
```

> **Note for AI Coding Assistants:** The [`file-structure.txt`](../file-structure.txt) file in the project root contains the complete, up-to-date directory structure of the entire project. This file is automatically maintained and provides a comprehensive reference for understanding the codebase layout.

---

## Development Workflows

### Frontend Development

For rapid UI iteration without Tauri:

```bash
bun run dev
```

This runs only the Vite dev server. Note that Tauri IPC calls will fail - use mock data for UI development.

### Full Application Development

```bash
bun run tauri dev
```

This compiles Rust and launches the full application with hot-reload for frontend changes.

### Type Checking

**🤖 AI Assistant Rule:** NEVER initiate `bun check`, `bun run check`, or `cargo check` yourself. The user will run these checks manually.

```bash
# Frontend: TypeScript + Svelte (DO NOT INITIATE YOURSELF - USER WILL DO IT)
bun run check

# Backend: Rust (DO NOT INITIATE YOURSELF - USER WILL DO IT)
cd src-tauri && cargo check
```

### Testing

```bash
# Rust tests
cd src-tauri && cargo test

# Run specific test
cd src-tauri && cargo test test_double_copy_detection
```

---

## Coding Standards

### General Principles

- **SOLID, YAGNI, KISS, DRY** - No premature abstractions
- Comments explain **"why"** not **"what"**
- Error messages should be meaningful and actionable

### Formatting

| Rule            | Value                    |
| --------------- | ------------------------ |
| Indentation     | 2 spaces                 |
| Line width      | 100 characters           |
| Strings         | Double quotes            |
| Semicolons      | Always                   |
| Trailing commas | None (ES5 compatibility) |

#### Svelte-Specific Rules (`.svelte` files)

| Rule                           | Value                                       |
| ------------------------------ | ------------------------------------------- |
| Indent script and style blocks | `svelteIndentScriptAndStyle: true`          |
| Tailwind class sorting         | `prettier-plugin-tailwindcss`               |
| Svelte syntax                  | Svelte 5 runes (`$state`, `$derived`, etc.) |
| Format with errors present     | `formatWithErrors: true`                    |

#### TypeScript/JavaScript Rules

| Rule                           | Value                         |
| ------------------------------ | ----------------------------- |
| Quote properties               | `quoteProperties: "asNeeded"` |
| Format with errors present     | `formatWithErrors: true`      |
| Tailwind class sorting (Biome) | `useSortedClasses` rule       |

#### AI Model Formatting Requirements

When generating code, always follow these formatting rules exactly. The `bun format` command runs both `biome format` and `prettier` in sequence to ensure all files are properly formatted.

### Naming Conventions

| Type                | Convention        | Example               |
| ------------------- | ----------------- | --------------------- |
| Files               | kebab-case        | `audio-player.ts`     |
| Svelte Components   | kebab-case.svelte | `user-profile.svelte` |
| Variables/Functions | camelCase         | `playbackState`       |
| Types/Interfaces    | PascalCase        | `AppConfig`           |
| Constants           | UPPER_SNAKE_CASE  | `DEFAULT_WINDOW_MS`   |
| Rust modules        | snake_case        | `clipboard_listener`  |

> **IMPORTANT:** All component files must use `kebab-case.svelte` naming convention. This applies to both new components and existing components that need to be renamed.

### Commenting Standards

Comments should make code more understandable for both AI systems and human developers. Focus on explaining **"why"** and **"how"** rather than just **"what"**.

#### What to Comment

- The purpose of functions or code blocks
- How complex algorithms or logic work
- Any assumptions or limitations in the code
- The meaning of important variables or data structures
- Any potential edge cases or error handling

#### Commenting Guidelines

| Guideline                | Description                                                              |
| ------------------------ | ------------------------------------------------------------------------ |
| **Clear and concise**    | Use simple, direct language                                              |
| **Avoid the obvious**    | Don't restate what the code does (e.g., `// increment counter` on `i++`) |
| **Focus on intent**      | Explain the reasoning behind decisions                                   |
| **Single-line comments** | Use for brief explanations                                               |
| **Multi-line comments**  | Use for longer explanations or function/class descriptions               |

#### Examples

**Good comment (explains why):**

```rust
// Use a 800ms window for double-copy detection to balance
// responsiveness with accidental copy prevention
const DOUBLE_COPY_WINDOW_MS: u64 = 800;
```

**Bad comment (states the obvious):**

```rust
// Set the window to 800 milliseconds
const DOUBLE_COPY_WINDOW_MS: u64 = 800;
```

**Good comment (explains complex logic):**

```typescript
// Normalize amplitude to 0.0-1.0 range by dividing by the
// maximum RMS value found in the audio chunk. This ensures
// consistent visualization regardless of input volume.
const normalized = rms / maxRms;
```

**Good comment (documents assumptions):**

```rust
// Assumes the TTS engine outputs 16-bit PCM WAV format.
// Other formats will require additional conversion logic.
fn parse_wav_header(data: &[u8]) -> Result<WavHeader> {
    // ...
}
```

### Svelte 5 Specific

#### Rune Scope

Runes (`$effect`, `$state`, `$derived`, etc.) are only valid inside `.svelte` files. For reactive logic in stores, expose methods that can be called from a component's `$effect`.

#### Event Handlers

Use `onclick` attribute instead of `on:click` directive for HTML elements. Apply this pattern to all event handlers:

- `onclick` (not `on:click`)
- `onsubmit` (not `on:submit`)
- `onchange` (not `on:change`)
- `oninput` (not `on:input`)
- etc.

#### Avoiding Reactive Loops

To prevent `effect_update_depth_exceeded` errors:

- Use `$derived` for computing new state from existing signals
- Avoid chains of `$effect`s that write to signals read by other effects
- Remember that derived signals are functions and must be called (e.g., `myDerived()`) in templates to resolve their value, especially for TypeScript

#### Performance in Loops

Avoid rendering many complex components (e.g., `Tooltip` from a UI library) inside an `#each` loop. The overhead from numerous component instances can cause performance bottlenecks and reactive crashes. When possible, use lightweight, native HTML alternatives like the `title` attribute for tooltips.

#### Svelte 5 Migration

Use new imports from `$app/state` instead of deprecated `$app/stores`:

| Old Import                                 | New Import                                |
| ------------------------------------------ | ----------------------------------------- |
| `import { page } from "$app/stores"`       | `import { page } from "$app/state"`       |
| `import { navigating } from "$app/stores"` | `import { navigating } from "$app/state"` |
| `import { updated } from "$app/stores"`    | `import { updated } from "$app/state"`    |

Access stores directly without the `$` prefix:

- Old: `$page.url.pathname`
- New: `page.url.pathname`

#### Component Patterns

- Favor functional patterns and hooks over classes
- Keep components focused and single-purpose
- Use `kebab-case.svelte` for component file names

#### Example Svelte 5 Component

```svelte
<script lang="ts">
  import { page } from "$app/state";

  // Use runes for reactive state
  let count = $state(0);
  let doubled = $derived(count * 2);

  // Use $props for component props
  let { title, onClick } = $props<{ title: string; onClick: () => void }>();

  // Use $effect for side effects
  $effect(() => {
    console.log("Count changed:", count);
  });

  // Derived signals must be called to resolve their value
  const displayValue = $derived(`Count: ${count}, Doubled: ${doubled()}`);
</script>

<!-- Use onclick NOT on:click -->
<button onclick={() => count++}>Increment</button>

<!-- Call derived signal to get its value -->
<p>{displayValue()}</p>
```

### TypeScript Rules

- **Strict mode** enabled
- No unused variables or parameters
- Use `PascalCase` for types/interfaces
- Explicit return types for public functions
- Wrap async calls in try/catch
- Bubble meaningful error messages

#### Type System

- Prefer `interface` over `type` for object shapes that may be extended; use `type` for unions, intersections, and aliases
- Always explicitly type function return values — never rely on inference for public-facing functions
- Avoid `any`; use `unknown` when the type is genuinely unknown and narrow it before use
- Use `satisfies` to validate a value against a type without widening it
- Prefer readonly properties and `Readonly<T>` / `ReadonlyArray<T>` for data that should not be mutated
- Use `const` assertions (`as const`) for literal types and enum-like objects instead of TypeScript `enum`
- Never use non-null assertion (`!`) — handle nullability explicitly with guards or optional chaining

#### Functions

- Prefer named functions over anonymous arrow functions for top-level declarations (aids stack traces)
- Keep functions small and single-purpose (KISS/SRP) — if a function needs a comment to explain what it does, it should be split
- Use optional parameters (`param?: T`) over overloads when possible; avoid overloads unless the type variation is meaningful
- Destructure parameters for functions with more than two arguments; use a typed options object instead

```typescript
// Good
function createUser({ name, role, active }: CreateUserOptions): User {}

// Avoid
function createUser(name: string, role: string, active: boolean): User {}
```

#### Imports & Exports

- Use named exports by default; use default exports only for Svelte components
- Group imports in this order, separated by a blank line: external packages → internal aliases → relative paths
- Use `import type` for type-only imports to keep runtime bundles clean

```typescript
import { writable } from "svelte/store";

import { db } from "$lib/server/db";

import type { AppConfig } from "./types";
```

#### Lucide Icons

**Always use `@lucide/svelte`** (NOT `lucide-svelte`) for the Lucide Icon library import:

- Correct: `import { Upload } from "@lucide/svelte";`
- Wrong: `import { Upload } from "lucide-svelte";`

#### Null & Error Handling

- Use discriminated unions to model success/failure instead of throwing in business logic

```typescript
type Result<T> = { ok: true; value: T } | { ok: false; error: string };
```

- Reserve `throw` for truly unexpected/unrecoverable errors; always throw `Error` instances with meaningful messages
- Use early returns and guard clauses to reduce nesting — never nest more than two levels deep

#### Patterns to Prefer

- Use `Map` and `Set` over plain objects when keys are dynamic or order/uniqueness matters
- Prefer `Array.prototype` methods (`map`, `filter`, `reduce`) over imperative loops for transformations; use `for...of` when side effects are needed
- Use `structuredClone` for deep copies instead of `JSON.parse(JSON.stringify(...))`
- Avoid barrel files (`index.ts` re-exports) unless the module surface is intentionally abstracted — they cause circular dependency issues

#### What to Avoid

| Anti-pattern                        | Preferred alternative                        |
| ----------------------------------- | -------------------------------------------- |
| `as SomeType` type assertions       | Type guards or `satisfies`                   |
| `enum`                              | `as const` objects with a derived value type |
| Deeply nested ternaries             | Early returns or named variables             |
| `Function` / `object` / `{}` types  | Explicit signatures or `Record<K, V>`        |
| Mutating function arguments         | Return new values; treat inputs as readonly  |
| `namespace` / `module` declarations | ES modules only                              |

---

## Tauri IPC Pattern

### Defining Commands (Rust)

```rust
// src-tauri/src/commands.rs
#[tauri::command]
pub async fn speak_now(
    config: State<'_, Mutex<AppConfig>>,
    audio: State<'_, Mutex<AudioPlayer>>,
) -> Result<(), String> {
    let cfg = config.lock().map_err(|e| e.to_string())?;
    // ... implementation
    Ok(())
}
```

### Registering Commands

```rust
// src-tauri/src/main.rs
.invoke_handler(tauri::generate_handler![
    commands::get_config,
    commands::set_config,
    commands::speak_now,
    commands::stop_speaking,
    commands::get_playback_state,
    commands::set_listening,
    commands::get_history,
    commands::clear_history,
    commands::test_tts,
])
```

### Calling from Frontend

```typescript
import { invoke } from "@tauri-apps/api/core";

async function speakNow() {
  try {
    await invoke("speak_now");
  } catch (error) {
    console.error("Failed to speak:", error);
  }
}
```

---

## Multi-Page Vite Configuration

CopySpeak uses two HTML entry points for the two Tauri windows:

```javascript
// vite.config.js
export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        hud: resolve(__dirname, "hud.html")
      }
    }
  }
});
```

---

## Configuration System

### Where Configuration Lives

- **Runtime**: JSON at `%APPDATA%/CopySpeak/config.json`
- **Defaults**: Hardcoded in `src-tauri/src/config/`

### Changing the Default TTS Engine

When changing the default TTS engine for new installations, update **both** locations:

1. **`src-tauri/src/config/tts.rs`** — `TtsConfig::default()` defines the default engine preset, command, args, and voice
2. **`src-tauri/src/config/mod.rs`** — `AppConfig::default()` must delegate to `TtsConfig::default()` (not duplicate values)

```rust
// src-tauri/src/config/mod.rs - CORRECT
tts: TtsConfig::default(),

// WRONG - don't duplicate defaults here
tts: TtsConfig {
    preset: "piper".into(),  // ❌ Duplicates TtsConfig::default()
    // ...
},
```

This ensures a single source of truth for default engine settings.

## Configuration System

### Where Configuration Lives

- **Runtime**: JSON at `%APPDATA%/CopySpeak/config.json`
- **Defaults**: Hardcoded in `src-tauri/src/config/` directory

### Changing the Default TTS Engine

When changing the default TTS engine for new installations, you there are **two location to update:** `src-tauri/src/config/tts.rs` - `TtsConfig::default()` — defines the default engine preset, command, args, and voice

2.  **Ensure consistency:** `src-tauri/src/config/mod.rs`
    - `AppConfig::default()` must delegate to `TtsConfig::default()` for the `TtsConfig` field
    - **DO NOT** duplicate engine settings in `AppConfig::default()` — this causes inconsistencies

Example:

```rust
// src-tauri/src/config/tts.rs
impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            active_backend: TtsEngine::Local,
            preset: "kitten-tts".into(),  // Change this
            command: "py".into(),
            args_template: vec![/* ... */],
            voice: "Jasper".into(),  // Change this
            // ...
        }
    }
}

// src-tauri/src/config/mod.rs
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // ... other fields ...
            tts: TtsConfig::default(),  // Delegates to TtsConfig::default()
            // ...
        }
    }
}
```

This ensures a single source of truth for default engine settings.

**Frontend files to update when adding/changing default engines:**

| File                                            | What to update                                          |
| ----------------------------------------------- | ------------------------------------------------------- |
| `src/lib/components/layout/app-footer.svelte`   | `ENGINES` array (add/remove entry, change order)        |
| `src/lib/components/layout/app-footer.svelte`   | `DEFAULT_VOICES` map (add/remove default voice)         |
| `src/lib/components/engine/local-engine.svelte` | Voice arrays (e.g., `KITTEN_VOICES`, `PIPER_EN_VOICES`) |
| `src/routes/onboarding/+page.svelte`            | Recommendation callout, installer button                |

### Adding a New Config Option

---

## TTS Backend Extension

### Adding a New Backend

1. Create new file in `src-tauri/src/tts/`
2. Implement `TtsBackend` trait:

```rust
#[async_trait]
impl TtsBackend for MyBackend {
    async fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        // Return WAV bytes
    }

    async fn health_check(&self) -> Result<bool, TtsError> {
        // Check if backend is available
    }
}
```

3. Add to `mod.rs` exports
4. Wire up in configuration system

---

## Content Filtering

> **⏸️ DEFERRED (v0.3+)** — This feature is preserved on the `features-extras` branch and has been deferred from v0.2 for focus on core functionality.

### Filter Rules (Deferred)

Filter rules use regex patterns to match and transform text before TTS:

```json
{
  "name": "Passwords",
  "pattern": "password\\s*[:=]\\s*\\S+",
  "action": "replace",
  "replacement": "[REDACTED]"
}
```

### How to Access (Deferred Feature)

To work with content filtering:

```bash
git checkout features-extras
```

All code for this feature is intact and ready for integration in v0.3+.

---

## Speech History

### History Structure

Each history entry contains:

```rust
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub text: String,
    pub tts_engine: String,
    pub voice: String,
    pub speed: f32,
    pub output_path: Option<String>, // Defines paths for Dual Storage cached playback audio
    pub duration_ms: u64,
    pub success: bool,
    // ... metadata and batch flags omitted for brevity
}
```

### TTS Audio Caching and Storage

Audio files synthesized for history entries are cached locally by default via the **Dual Storage Mode** architecture. This helps to eliminate redundant LLM/TTS generation processes if identical parameters (`text`, `voice`, and `tts_engine`) are encountered. Playback speed and pitch are controlled via browser frontend playback rate, not at synthesis time.

1.  **StorageMode**: Defined under `AppConfig.history.storage_mode` as either `Temp` (uses the system's temporary directory for transient caching) or `Persistent` (uses `%APPDATA%` or a tailored `persistent_dir`).
2.  **AutoDeleteMode**: Defines when history logs and their accompanying cached `.wav` files should be removed (`KeepLatest(u32)`, `AfterDays(u32)`, or `Never`).
3.  **Playback & Re-Synthesis**: The UI dynamically polls for `output_path`. If cached audio is not found due to a manual cache clear or temp clearance, the `play_history_entry` IPC throws an error. The UI catches this and initiates `speak_history_entry` to re-synthesize missing audio.

### Accessing History

```typescript
// Get history
const history = await invoke<HistoryEntry[]>("get_history");

// Clear history
await invoke("clear_history");
```

---

## Audio Save Mode

### Filename Templates

Supported placeholders:

| Placeholder | Description                 | Example      |
| ----------- | --------------------------- | ------------ |
| `{date}`    | Current date (YYYY-MM-DD)   | `2026-02-11` |
| `{time}`    | Current time (HH-MM-SS)     | `14-30-45`   |
| `{hash}`    | Text content hash (8 chars) | `a1b2c3d4`   |
| `{voice}`   | Voice name                  | `af_nicole`  |

### Enabling Save Mode

```json
{
  "playback": {
    "save_mode": {
      "enabled": true,
      "output_dir": "C:\\Users\\User\\Audio",
      "filename_template": "{date}_{time}_{hash}.wav"
    }
  }
}
```

---

## Common Tasks

### Adding a New IPC Command

1. Define command in `commands.rs`
2. Register in `main.rs` `invoke_handler`
3. Add to `capabilities/default.json` if new permissions needed
4. Create TypeScript wrapper in frontend

### Adding a New UI Component

#### Decision Tree: shadcn-svelte vs Custom

| If you need...                                      | Use...                              | Location                             |
| --------------------------------------------------- | ----------------------------------- | ------------------------------------ |
| Common UI elements (buttons, inputs, dialogs, etc.) | **shadcn-svelte**                   | `src/lib/components/ui/<component>/` |
| App-specific components                             | **Custom**                          | `src/lib/components/<Name>.svelte`   |
| Reusable compound patterns                          | **Custom** in `src/lib/components/` |

#### Installing shadcn-svelte Components

**IMPORTANT: Always use the shadcn CLI to add components - never create them manually.**

```bash
# Add a component
bun x shadcn-svelte@latest add <component>

# Add multiple components
bun x shadcn-svelte@latest add button input select
```

**Available shadcn-svelte UI Components:**
`accordion`, `alert`, `alert-dialog`, `aspect-ratio`, `avatar`, `badge`, `breadcrumb`, `button-group`, `button`, `calendar`, `card`, `carousel`, `chart`, `checkbox`, `collapsible`, `combobox`, `command`, `context-menu`, `data-table`, `date-picker`, `dialog`, `drawer`, `dropdown-menu`, `empty`, `field`, `formsnap`, `hover-card`, `input-group`, `input-otp`, `input`, `item`, `kbd`, `label`, `menubar`, `native-select`, `navigation-menu`, `pagination`, `popover`, `progress`, `radio-group`, `range-calendar`, `resizable`, `scroll-area`, `select`, `separator`, `sheet`, `sidebar`, `skeleton`, `slider`, `sonner`, `spinner`, `switch`, `table`, `tabs`, `textarea`, `toggle-group`, `toggle`, `tooltip`, `typography`

#### shadcn-svelte Best Practices

**1. Import Patterns**

```svelte
<!-- Import from the ui folder (NEVER from $lib/components/ui) -->
<script>
  import Button from "$lib/components/ui/button";
  import Input from "$lib/components/ui/input";
  import Label from "$lib/components/ui/label";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Tabs from "$lib/components/ui/tabs";
</script>
```

**2. Using Component Variants**

```svelte
<!-- Use variant prop for styling variations -->
<Button variant="default">Primary</Button>
<Button variant="secondary">Secondary</Button>
<Button variant="destructive">Danger</Button>
<Button variant="ghost">Ghost</Button>
<Button variant="link">Link</Button>
<Button variant="outline">Outline</Button>

<!-- Size variants -->
<Button size="default">Default</Button>
<Button size="sm">Small</Button>
<Button size="lg">Large</Button>
<Button size="icon">
  <Icon name="settings" />
</Button>
```

**3. Class Merging with `cn()`**

Always use the `cn()` utility when extending shadcn component classes:

```svelte
<script>
  import Button from "$lib/components/ui/button";
  import { cn } from "$lib/utils";

  let { class: className, ...props } = $props();
</script>

<Button class={cn("w-full", className)} {...props}>
  <slot />
</Button>
```

**4. Compound Components Pattern**

Many shadcn-svelte components use compound patterns:

```svelte
<!-- Dialog compound pattern -->
<Dialog.Root bind:open={isOpen}>
  <Dialog.Trigger>
    <Button>Open Dialog</Button>
  </Dialog.Trigger>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Confirm Action</Dialog.Title>
      <Dialog.Description>This action cannot be undone.</Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button variant="outline" onclick={() => (isOpen = false)}>Cancel</Button>
      <Button variant="destructive" onclick={handleConfirm}>Confirm</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- Tabs compound pattern -->
<Tabs.Root value="account">
  <Tabs.List>
    <Tabs.Trigger value="account">Account</Tabs.Trigger>
    <Tabs.Trigger value="password">Password</Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content value="account">
    <AccountForm />
  </Tabs.Content>
  <Tabs.Content value="password">
    <PasswordForm />
  </Tabs.Content>
</Tabs.Root>
```

**5. Form Components with `formsnap`**

For forms, use shadcn-svelte form components with Formsnap:

```svelte
<script>
  import * as Form from "$lib/components/ui/form";
  import Input from "$lib/components/ui/input";

  let { form } = $props();
</script>

<form method="POST" use:form>
  <Form.Field {form} name="email">
    <Form.Control let:attrs>
      <Form.Label>Email</Form.Label>
      <Input {...attrs} bind:value={$form.email} type="email" />
    </Form.Control>
    <Form.FieldErrors />
  </Form.Field>
</form>
```

**6. Theming Best Practices**

- **Never override CSS variables directly** - use Tailwind classes
- **Dark mode is automatic** - the app uses `mode-watcher`
- **Use semantic color tokens**:
  - `bg-background`, `text-foreground` for base surfaces
  - `bg-primary`, `text-primary-foreground` for primary actions
  - `bg-destructive`, `text-destructive-foreground` for destructive actions
  - `bg-muted`, `text-muted-foreground` for secondary/placeholder text

**7. Creating Custom shadcn-Style Components**

When creating components that match shadcn's style:

```svelte
<!-- src/lib/components/ui/my-component/my-component.svelte -->
<script>
  import { cn } from "$lib/utils";
  import type { HTMLAttributes } from "svelte/elements";

  type Props = HTMLAttributes<HTMLDivElement> & {
    variant?: "default" | "outline";
  };

  let { variant = "default", class: className, children, ...props }: Props = $props();

  const variantStyles = {
    default: "bg-primary text-primary-foreground",
    outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground"
  };
</script>

<div class={cn("rounded-md p-4", variantStyles[variant], className)} {...props}>
  {@render children?.()}
</div>
```

**8. Common Anti-Patterns to Avoid**

```svelte
<!-- DON'T: Import from wrong path -->
<script>
  import Button from "$lib/components/ui/button/button.svelte"; // WRONG
  import Button from "$lib/components/ui/button"; // CORRECT
</script>

<!-- DON'T: Use arbitrary Tailwind values excessively -->
<div class="w-30.5 h-11.25"> <!-- AVOID -->
<div class="w-32 h-11"> <!-- PREFER standard values -->

<!-- DON'T: Override component internals -->
<!-- Instead, pass classes via props -->
<Button class="my-custom-class">Text</Button>

<!-- DON'T: Mix on:click and onclick -->
<!-- Use onclick (Svelte 5) consistently -->
<button on:click={handler}> <!-- AVOID -->
<button onclick={handler}> <!-- CORRECT -->
```

**9. Common Component Combinations**

```svelte
<!-- Card layout -->
<Card.Root>
  <Card.Header>
    <Card.Title>Settings</Card.Title>
    <Card.Description>Manage your preferences</Card.Description>
  </Card.Header>
  <Card.Content>
    <Label for="email">Email</Label>
    <Input id="email" type="email" />
  </Card.Content>
  <Card.Footer>
    <Button>Save</Button>
  </Card.Footer>
</Card.Root>

<!-- Dropdown menu -->
<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    <Button variant="outline">Options</Button>
  </DropdownMenu.Trigger>
  <DropdownMenu.Content>
    <DropdownMenu.Item onclick={handleEdit}>
      <Pencil class="mr-2 h-4 w-4" />
      Edit
    </DropdownMenu.Item>
    <DropdownMenu.Separator />
    <DropdownMenu.Item onclick={handleDelete} class="text-destructive">
      <Trash class="mr-2 h-4 w-4" />
      Delete
    </DropdownMenu.Item>
  </DropdownMenu.Content>
</DropdownMenu.Root>
```

**10. Responsive Patterns**

```svelte
<!-- Use shadcn-svelte with Tailwind responsive classes -->
<Dialog.Root>
  <!-- Use Drawer on mobile, Dialog on desktop -->
  <Drawer.Root class="md:hidden">
    <!-- Mobile drawer content -->
  </Drawer.Root>

  <Dialog.Content class="hidden md:flex">
    <!-- Desktop dialog content -->
  </Dialog.Content>
</Dialog.Root>
```

#### Creating Custom Components

When shadcn-svelte doesn't have what you need:

1. **Location**: Create in `src/lib/components/<ComponentName>.svelte`
2. **Naming**: Use PascalCase for component files
3. **Props**: Use Svelte 5 `$props()` rune
4. **Styling**: Use Tailwind classes with `cn()` for merging
5. **Events**: Use Svelte 5 event handler attributes (`onclick`, `onchange`)
6. **Export types**: Export component prop types when needed

```svelte
<!-- src/lib/components/CustomCard.svelte -->
<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    description?: string;
    class?: string;
    children?: Snippet;
  }

  let { title, description, class: className, children }: Props = $props();
</script>

<div class={cn("bg-card text-card-foreground rounded-lg border p-6 shadow-sm", className)}>
  <h3 class="text-lg leading-none font-semibold tracking-tight">{title}</h3>
  {#if description}
    <p class="text-muted-foreground mt-2 text-sm">{description}</p>
  {/if}
  <div class="mt-4">
    {@render children?.()}
  </div>
</div>
```

#### Key Resources

- **shadcn-svelte docs**: https://www.shadcn-svelte.com/docs/components
- **Svelte 5 docs**: https://svelte.dev/docs
- **Tailwind CSS v4.2**: https://tailwindcss.com/docs/installation/using-vite
- **Formsnap**: https://formsnap.dev/docs
- **mode-watcher**: https://github.com/svecosystem/mode-watcher

### Debugging Clipboard Issues

1. Check if `AddClipboardFormatListener` is registered
2. Verify clipboard permissions in Windows settings
3. Test with simple text (avoid rich text/images initially)
4. Check state machine logs in dev console

### Adding a New TTS Backend

See [TTS Backend Extension](#tts-backend-extension) section above.

---

## Troubleshooting

### Common Issues

| Issue                           | Solution                               |
| ------------------------------- | -------------------------------------- |
| "TTS command not found"         | Ensure TTS engine is in PATH           |
| HUD window not showing          | Check `hud.enabled` in config          |
| Double-copy not triggering      | Verify `double_copy_window_ms` timing  |
| Frontend changes not reflecting | Clear `.svelte-kit` cache              |
| Rust compilation slow           | Use `cargo check` for quick validation |
| Filter rules not working        | Check regex syntax and escaping        |
| History not saving              | Verify `history.enabled` in config     |
| Audio save failing              | Check output directory permissions     |

### Dev Tools

- **Tauri Dev Tools**: Right-click in app → Inspect
- **Rust Logging**: Set `RUST_LOG=debug` environment variable
- **VS Code**: Install Svelte, Rust-analyzer extensions

---

### Important Agent Rules

- **WSL2**: `wsl` is available in a working Ubuntu environment, if needed.
- **Type Checking**: NEVER initiate `bun check`, `bun run check`, or `cargo check` yourself. The user will run these checks manually.

### Available Skills

**Skills** are specialized instructions loaded via the Skill tool for specific tasks. Project skills are defined in `.agents/skills/`:

| Skill                              | Purpose                                                               |
| ---------------------------------- | --------------------------------------------------------------------- |
| `calling-rust-from-tauri-frontend` | Call Rust backend functions from Tauri frontend using invoke          |
| `configuring-tauri-permissions`    | Configure Tauri permissions, allow/deny lists, capabilities           |
| `integrating-tauri-js-frontends`   | Configure JS frameworks (Next.js, Nuxt, SvelteKit, Vite) for Tauri v2 |
| `svelte-code-writer`               | Svelte 5 documentation lookup and code analysis                       |
| `svelte-core-bestpractices`        | Guidance on writing fast, robust Svelte 5 code                        |
| `openspec-propose`                 | Propose a new change with design, specs, and tasks                    |
| `openspec-apply-change`            | Implement tasks from an OpenSpec change                               |
| `openspec-explore`                 | Thinking partner for exploring ideas before changes                   |
| `openspec-archive-change`          | Archive a completed change                                            |
| `mcp2cli`                          | Turn MCP servers or OpenAPI specs into CLI commands                   |
| `smithery-ai-cli`                  | Discover and connect to MCP tools/skills via Smithery                 |
| `find-skills`                      | Discover and install agent skills for extended capabilities           |
| `vercel`                           | Vercel Platform and API documentation                                 |

Skills are automatically loaded when a task matches their description.

### Using AI Agent Teams

For complex, multi-perspective analysis, you can spawn **agent teams** (also called "swarms") that work together:

#### When to Use Teams

- **Multi-perspective analysis**: UX, architecture, security reviews
- **Parallel research**: Exploring multiple approaches simultaneously
- **Complex audits**: Code review, security assessment, performance analysis
- **Feature design**: Planning implementations from different angles

#### Agent Types Available

| Agent Type        | Capabilities                                | Best For                                             |
| ----------------- | ------------------------------------------- | ---------------------------------------------------- |
| `general-purpose` | Full access (Read, Edit, Write, Bash, etc.) | Implementation tasks, bug fixes, feature development |
| `Explore`         | Read-only (Glob, Grep, Read)                | Code exploration, research, analysis                 |
| `Plan`            | Read-only + planning tools                  | Designing implementation strategies                  |
| `Bash`            | Command execution only                      | Git operations, build tasks, testing                 |

#### Example: Multi-Perspective Code Review

```typescript
// This is pseudocode showing the pattern - actual implementation uses Claude Code's Task tool

// 1. Create a team
await createTeam({
  name: "security-review",
  description: "Comprehensive security audit"
});

// 2. Spawn specialized teammates
await spawnAgent({
  type: "Explore",
  name: "security-analyst",
  task: "Review authentication and authorization patterns"
});

await spawnAgent({
  type: "Explore",
  name: "privacy-expert",
  task: "Audit data handling and GDPR compliance"
});

await spawnAgent({
  type: "Explore",
  name: "dependency-checker",
  task: "Scan for vulnerable dependencies"
});

// 3. Agents work autonomously and report findings
// 4. Coordinator synthesizes results
```

#### Team Coordination Pattern

When using teams:

1. **Task-based coordination**: Create tasks that teammates claim and complete
2. **Direct messaging**: Teammates can message each other for collaboration
3. **Progress tracking**: Monitor task completion and idle states
4. **Result synthesis**: Team lead combines findings into actionable recommendations

#### Best Practices for AI-Assisted Development

**DO:**

- ✅ Use agent teams for complex, multi-faceted problems
- ✅ Let Explore agents do read-only research and analysis
- ✅ Use general-purpose agents for actual implementation work
- ✅ Create clear, specific task descriptions for teammates
- ✅ Review agent findings critically before implementing

**DON'T:**

- ❌ Spawn too many agents (3-5 is usually optimal)
- ❌ Give implementation tasks to read-only agents (Explore, Plan)
- ❌ Blindly trust agent output without validation
- ❌ Use teams for simple, single-perspective tasks

### Project-Specific AI Context

This repository includes AI-friendly documentation:

| File                 | Purpose                                       |
| -------------------- | --------------------------------------------- |
| `CLAUDE.md`          | High-level project overview for AI assistants |
| `file-structure.txt` | Complete directory layout                     |
| `docs_internal/*.md` | Architecture, development, roadmap docs       |

**Tip**: When asking Claude Code for help, reference these docs:

```
"Review the security section in docs_internal/architecture.md and suggest improvements"
"Based on the roadmap in docs_internal/roadmap.md, help me implement Phase 7"
```

### Example Workflows

#### Workflow 1: Security Hardening

```bash
# In Claude Code conversation:
"Create an agent team to audit CopySpeak's security:
- One agent reviews authentication/authorization
- One agent reviews data privacy and clipboard handling
- One agent plays devil's advocate on attack vectors"
```

#### Workflow 2: Performance Optimization

```bash
# In Claude Code conversation:
"Spawn an Explore agent to:
1. Identify performance bottlenecks in the clipboard monitoring system
2. Review audio playback latency
3. Suggest optimizations for the TTS synthesis pipeline"
```

### Limitations and Considerations

- **Context limits**: Agents work within conversation context windows
- **Read-only agents**: Explore/Plan agents can't write code, only analyze
- **Validation required**: Always review agent suggestions before implementing
- **Resource usage**: Multiple agents consume more API resources

### Getting Help with Claude Code

- **Documentation**: Visit [claude.com/code](https://claude.com/code)
- **Issues**: Report problems at [github.com/anthropics/claude-code/issues](https://github.com/anthropics/claude-code/issues)
- **In-conversation**: Type `/help` in Claude Code

---

## Changelog Maintenance

**For all PRs and commits affecting functionality:**

- Update `CHANGELOG.md` under the `[Unreleased]` section
- Use these categories: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`, `Breaking Changes`
- List specific changes with implementation details (functions, structs, features added)
- Include `BREAKING CHANGE:` prefix for incompatible API changes

Example:

```markdown
### Added

- Feature description with implementation details
  - Specific component/function details

### Changed

- Modified existing functionality description

### Breaking Changes

- `OldClass::method()` now requires `newParam` parameter
```

## Release Process

Releases are automated via GitHub Actions. The workflow builds the installer, generates update signatures, and publishes to GitHub Releases.

### Automated Release (Recommended)

1. **Update version:**

   ```bash
   bun run bump          # Patch version (0.0.x)
   bun run bump:minor    # Minor version (0.x.0)
   bun run bump:major    # Major version (x.0.0)
   ```

2. **Update `CHANGELOG.md`:**
   - Move changes from `[Unreleased]` to new version section
   - Follow [Keep a Changelog](https://keepachangelog.com/) format

3. **Commit and push to `main`:**

   ```bash
   git add .
   git commit -m "chore(release): bump version to x.x.x"
   git push origin main
   ```

4. **GitHub Actions automatically:**
   - Builds Windows installer (.exe) and MSI
   - Extracts release notes from CHANGELOG.md
   - Generates `latest.json` with update signature
   - Creates published release (not draft)

### Release Requirements

For the auto-updater to work, these must be configured:

| Requirement                          | Location                    | Purpose                             |
| ------------------------------------ | --------------------------- | ----------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | GitHub repo secrets         | Private key for signing updates     |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | GitHub repo secrets         | Password for the private key        |
| `pubkey` in `tauri.conf.json`        | `src-tauri/tauri.conf.json` | Public key matching the private key |
| `.tauri/signing.key.pub`             | Tracked in repo             | Backup of public key                |

### Troubleshooting Releases

#### `latest.json` not generated /404 error on update check

**Cause:** Public key in `tauri.conf.json` doesn't match the private key in GitHub secrets.

**Solution:**

1. Generate a new key pair:
   ```bash
   tauri signer generate -- -w .tauri/signing.key
   ```
2. Copy the public key output to:
   - `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
   - `.tauri/signing.key.pub` (for backup)
3. Update GitHub secrets with the new private key and password
4. Commit and push changes
5. Trigger a new release

#### Release notes show fallback text instead of changelog

**Cause:** CHANGELOG.md doesn't have a section for the current version.

**Solution:** Ensure CHANGELOG.md has a `## [x.x.x]` section matching the version in `package.json`.

#### Release created as draft

**Cause:** `releaseDraft: true` in workflow.

**Solution:** The workflow sets `releaseDraft: false`. Draft releases are not accessible via `/releases/latest/` API, causing updater404 errors.

### Manual Release (Emergency)

If GitHub Actions is unavailable:

```bash
# Build locally
bun run tauri build

# Sign the update (requires TAURI_SIGNING_PRIVATE_KEY env var)
# The build process generates latest.json automatically when keys are configured

# Manually upload to GitHub Releases:
# - CopySpeak_x.x.x_x64-setup.exe
# - CopySpeak_x.x.x_x64_en-US.msi
# - latest.json (from src-tauri/target/release/bundle/)
```

### Key Rotation

To rotate signing keys:

1. Generate new keys: `tauri signer generate -- -w .tauri/new-signing.key`
2. Update `tauri.conf.json` with new public key
3. Update GitHub secrets with new private key + password
4. Replace `.tauri/signing.key.pub` with new public key
5. Delete the old private key file
6. Commit and push

> **⚠️ Important:** After key rotation, users on old versions won't be able to auto-update. They'll need to manually download the new installer.
