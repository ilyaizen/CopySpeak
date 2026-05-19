
# CopySpeak Agent Guide

> Guidance for Claude Code (claude.ai/code) when working with code in this repository.

## Core Development Rules

### 1. Think Before Code

No assume. No hide confusion. Surface tradeoffs.
State assumptions. Uncertain → ask.
Multiple interpretations → present, no silent pick.
Simpler path exist → say so. Push back when warranted.
Unclear → stop. Name confusion. Ask.

### 2. Simplicity First

Min code that solve problem. Nothing speculative.
No features beyond ask.
No abstractions for single-use code.
No "flexibility"/"configurability" not requested.
No error handling for impossible cases.
200 lines could be 50 → rewrite.
Test: senior eng call this overcomplicated? Yes → simplify.

### 3. Surgical Changes

Touch only what must. Clean only own mess.
No "improve" adjacent code/comments/format.
No refactor things not broken.
Match existing style even if disagree.
Unrelated dead code → mention, no delete.
Own changes orphan imports/vars → remove.
Pre-existing dead code → leave unless asked.
Test: every changed line trace to user request.

### 4. Goal-Driven Execution

Define success. Loop until verified.
"Add validation" → write failing tests, make pass.
"Fix bug" → write reproducing test, make pass.
"Refactor X" → tests pass before and after.
Multi-step → state plan: [step] → verify: [check].

### 5. Testing / Committing

DO NOT run checks. ALWAYS ASK USER for explicit confirmation before running any verification, linting, type-check, or build commands.
DO NOT commit changes without explicit user confirmation. Before ending a task, ask whether to run checks and commit. If the user confirms committing, generate a suitable Conventional Commits message that summarizes the diff concisely.

bun format — biome + prettier hybrid format.
bun check — types + svelte-check.
bun build — production build.

Use running dev server. Test scroll, theme toggle, cursor.

## Commands

```bash
# Development
bun run tauri dev           # Full app with hot-reload
bun run dev                 # Frontend only (port 5173)

# Build
bun run tauri build         # Production build

bun run check               # TypeScript/Svelte type checking
bun run check:watch         # Watch mode for type checking

# Testing (Frontend)
bun run test                # Run all frontend tests (vitest)
bun run test <name>         # Run single frontend test
bun run test:watch          # Watch mode

# Testing (Rust)
cd src-tauri && cargo test             # Run all Rust tests
cd src-tauri && cargo test <name>      # Run single Rust test (e.g., cargo test double_copy)
cd src-tauri && cargo check            # Type check Rust
cd src-tauri && cargo clippy           # Lint Rust

# Version Bumping
bun run bump                # Patch version bump (0.0.x)
bun run bump:minor          # Minor version bump (0.x.0)
bun run bump:major          # Major version bump (x.0.0)
```

## Code Style

### Naming Conventions

| Type                | Convention        | Example               |
| ------------------- | ----------------- | --------------------- |
| Files               | kebab-case        | `audio-player.ts`     |
| Svelte Components   | kebab-case.svelte | `user-profile.svelte` |
| Variables/Functions | camelCase         | `playbackState`       |
| Types/Interfaces    | PascalCase        | `AppConfig`           |
| Constants           | UPPER_SNAKE_CASE  | `DEFAULT_WINDOW_MS`   |
| Rust modules        | snake_case        | `clipboard_listener`  |

### Formatting

- Indentation: 2 spaces
- Line width: 100 characters
- Strings: Double quotes
- Semicolons: Always
- Trailing commas: None (ES5 compatibility)

### Svelte 5

```svelte
<script lang="ts">
  import { page } from "$app/state";

  let count = $state(0);
  let doubled = $derived(count * 2);
  let { title, onClick } = $props<{ title: string; onClick: () => void }>();

  $effect(() => {
    console.log("Count changed:", count);
  });

  function handleClick() {
    count++;
  }
</script>

<button onclick={handleClick}>{title}</button><p>Doubled: {doubled()}</p>
```

**Key rules:**

- Use `$state`, `$derived`, `$props`, `$effect` (not `$:`)
- Use `onclick` NOT `on:click`
- Call derived signals in templates: `doubled()` not `doubled`
- Import from `$app/state` not `$app/stores`

**Slider bindings with optional config values:**

For optional config sliders (e.g., `voice_style?: number`), use local `$state` + `$effect` for cancel support, `onchange` for user changes:

```ts
let styleValue = $state(localConfig.tts.elevenlabs.voice_style ?? 0);

// Sync FROM config when parent cancels/resets localConfig
$effect(() => {
  const cfg = localConfig;
  const configValue = cfg.tts.elevenlabs.voice_style ?? 0;
  if (styleValue !== configValue) {
    styleValue = configValue;
  }
});
```

```svelte
<!-- Sync TO config via onchange (NOT $effect - avoids race condition with cancel) -->
<Slider
  bind:value={styleValue}
  onchange={(v) => {
    localConfig.tts.elevenlabs.voice_style = v;
  }}
/>
```

**Why this pattern?** `$effect` sync TO config can race: parent cancel replaces `localConfig`, effect runs old `styleValue`, overwrites reset. `onchange` syncs only explicit user interaction.

### Rust

```rust
#[tauri::command]
pub async fn speak_now(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<(), String> {
    let cfg = config.lock().map_err(|e| e.to_string())?;
    Ok(())
}
```

### Imports

- Frontend: `$lib/` local imports
- Backend: `crate::module` relative imports
- Group order: external → internal aliases → relative
- **Lucide Icons**: Always use `@lucide/svelte` (NOT `lucide-svelte`)
  - Correct: `import { Upload } from "@lucide/svelte";`
  - Wrong: `import { Upload } from "lucide-svelte";`

### Error Handling

- Rust: `Result<T, String>` for IPC, `?` with `map_err`
- TypeScript: Discriminated unions `{ ok: true; value: T } | { ok: false; error: string }`

### TypeScript Rules

- Strict mode enabled
- No unused variables
- Explicit return types for public functions
- Prefer `interface` over `type` for object shapes
- Use `satisfies` instead of type assertions
- Never use `!` non-null assertion

## Project Structure

```
copyspeak/
├── src/                     # Svelte 5 frontend
│   ├── lib/
│   │   ├── components/      # UI components
│   │   │   ├── ui/          # shadcn-svelte
│   │   │   └── *.svelte     # Custom
│   │   └── utils.ts         # cn() utility
│   └── routes/              # SvelteKit routes
├── src-tauri/src/           # Rust backend
│   ├── main.rs              # Entry, IPC registration
│   ├── config.rs            # Persistence
│   ├── commands.rs          # IPC handlers
│   ├── clipboard.rs         # Double-copy detection
│   ├── audio.rs             # Playback
│   ├── tts/                 # TTS backend abstraction
│   ├── config/              # Config modules (directory-based)
│   ├── commands/            # Command modules (directory-based)
│   ├── sanitize/            # Text normalization modules
│   └── ...
├── index.html               # Main window
└── hud.html                 # HUD overlay
```

## Tauri IPC Pattern

1. Define in `commands.rs`:

```rust
#[tauri::command]
pub async fn my_command(state: State<'_, Mutex<MyState>>) -> Result<T, String> {
    // implementation
}
```

2. Register in `main.rs`:

```rust
.invoke_handler(tauri::generate_handler![commands::my_command])
```

3. Call from frontend:

```typescript
import { invoke } from "@tauri-apps/api/core";
await invoke("my_command");
```

## Git Workflow

- **NEVER commit directly to `main`** - all changes via PRs
- Work on feature branches: `feature/`, `fix/`, `refactor/`, `docs/`
- Use versioned dev branches for releases: `develop/0.1.0`, `develop/0.2.0`, etc.
- Open PRs targeting `main` (or `develop/*` for larger efforts)
- Current development: `develop/0.1.0` (settings consolidation)

## Best Practices

- Do NOT initiate `bun run check` or `cargo check`. User handles type checking manually.
- Follow existing code patterns
- Use Svelte 5 runes ($state, $derived, $props)
- Keep responses concise (1-3 sentences)
- Comments explain "why" not "what"
- Update CHANGELOG.md for notable changes (features, fixes, breaking changes). Follow [Keep a Changelog](https://keepachangelog.com/) sections: Added, Changed, Deprecated, Removed, Fixed, Security

## Changelog Maintenance

**For all PRs and commits affecting functionality:**

- Update `CHANGELOG.md` under `[Unreleased]`
- Use categories: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`, `Breaking Changes`
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

## Documentation

### Internal Docs (`docs_internal/`)

| Document                                                     | Purpose                               |
| ------------------------------------------------------------ | ------------------------------------- |
| [project-overview.md](./docs_internal/project-overview.md)   | Project context and key decisions     |
| [requirements.md](./docs_internal/requirements.md)           | Feature requirements and traceability |
| [architecture.md](./docs_internal/architecture.md)           | System architecture and design        |
| [development_guide.md](./docs_internal/development_guide.md) | Setup and development workflow        |
| [tts_backends.md](./docs_internal/tts_backends.md)           | TTS engine integration guide          |
| [brutalist_design.md](./docs_internal/brutalist_design.md)   | UI design system and aesthetics       |
| [roadmap.md](./docs_internal/roadmap.md)                     | Development roadmap and phases        |

### Public Docs (`docs/`)

| Document                                  | Purpose                 |
| ----------------------------------------- | ----------------------- |
| [CONTRIBUTING.md](./docs/CONTRIBUTING.md) | Contribution guidelines |

## ⚠️ DO NOT TOUCH - Translation Files

**AI Agents: Do NOT modify or touch files in `src-web/src/lib/locales/DO_NOT_TOUCH/`**

Directory contains non-English translations (Arabic, Spanish, Hebrew), externally managed. Pre-production translation keys change often.

- Translation keys unstable during development
- External translators handle these files
- Manual edits overwritten
- Only modify English locale file (`en.json`) if needed

<!-- rtk-instructions v2 -->

# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has dedicated filter, it uses it. Else passthrough unchanged. RTK always safe. No `rtk bun`; see commands.

**Important**: Even in command chains with `&&`, use `rtk`:

```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)

```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (90-99% savings)

```bash
rtk cargo test          # Cargo test failures only (90%)
rtk vitest run          # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)

```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Git passthrough works for ALL subcommands, including ones not listed.

### GitHub (26-87% savings)

```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)

```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
```

### Files & Search (60-75% savings)

```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%)
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)

```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)

```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)

```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands

```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category         | Commands                       | Typical Savings |
| ---------------- | ------------------------------ | --------------- |
| Tests            | vitest, playwright, cargo test | 90-99%          |
| Build            | next, tsc, lint, prettier      | 70-87%          |
| Git              | status, log, diff, add, commit | 59-80%          |
| GitHub           | gh pr, gh run, gh issue        | 26-87%          |
| Package Managers | pnpm, npm, npx                 | 70-90%          |
| Files            | ls, read, grep, find           | 60-75%          |
| Infrastructure   | docker, kubectl                | 85%             |
| Network          | curl, wget                     | 65-70%          |

Overall average: **60-90% token reduction** on common dev ops.

<!-- /rtk-instructions -->