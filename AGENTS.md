# AGENTS.md

> For AI coding agents (Hermes-Agent, Pi, OpenCode, KiloCode, Claude Code, Cursor, etc.)

## Project

CopySpeak - A modern AI text-to-speech orchestrator for Windows that reads clipboard text aloud when double-copied. Stack: Svelte 5, Tauri 2.

## Core Development Rules

### 1. Think Before Code

- No assume. No hide confusion. Surface tradeoffs.
- State assumptions. Uncertain → ask.
- Multiple interpretations → present, no silent pick.
- Simpler path exist → say so. Push back when warranted.
- Unclear → stop. Name confusion. Ask.

### 2. Simplicity First

- Min code that solve problem. Nothing speculative.
- No features beyond ask.
- No abstractions for single-use code.
- No "flexibility"/"configurability" not requested.
- No error handling for impossible cases.
- 200 lines could be 50 → rewrite.
- Test: senior eng call this overcomplicated? Yes → simplify.

### 3. Surgical Changes

- Touch only what must. Clean only own mess.
- No "improve" adjacent code/comments/format.
- No refactor things not broken.
- Match existing style even if disagree.
- Unrelated dead code → mention, no delete.
- Own changes orphan imports/vars → remove.
- Pre-existing dead code → leave unless asked.
- Test: every changed line trace to user request.

### 4. Goal-Driven Execution

- Define success. Loop until verified.
- "Add validation" → write failing tests, make pass.
- "Fix bug" → write reproducing test, make pass.
- "Refactor X" → tests pass before and after.
- Multi-step → state plan: [step] → verify: [check].

### 5. Testing / Committing

DO NOT run checks. ALWAYS ASK USER for explicit confirmation before running any verification, linting, type-check, or build commands.

DO NOT commit changes without explicit user confirmation. Before ending a task, ask whether to run checks and commit. If the user confirms committing, generate a suitable Conventional Commits message that summarizes the diff concisely.

- `bun format` - biome + prettier hybrid format.
- `bun check` - types + svelte-check.
- `bun build` - production build.

Use running Tauri dev server.

## Efficiency

- Read before write. Each file once.
- Edit over rewrite. No write-delete-rewrite cycles.
- Test once, fix, verify once.
- Budget: 50 tool calls.
- Stuck → ask. No dead ends.
- No sycophantic openers/fluff.
- Never guess paths.

## Code Style

### Naming Conventions

- Files (kebab-case) & Svelte components (kebab-case.svelte)
- Variables/functions (camelCase) & Types/interfaces (PascalCase)
- Constants (UPPER_SNAKE_CASE) & Rust modules (snake_case)

### Formatting

- Indentation: 2 spaces
- Line width: 100 characters
- Strings: Double quotes
- Semicolons: Always
- Trailing commas: None (ES5 compatibility)

### Svelte Rules

- Use `$state`, `$derived`, `$props`, `$effect` (not `$:`)
- Use `onclick` NOT `on:click`
- Call derived signals in templates: `doubled()` not `doubled`
- Import from `$app/state` not `$app/stores`

### TypeScript Rules

- Strict mode enabled
- No unused variables
- Explicit return types for public functions
- Prefer `interface` over `type` for object shapes
- Use `satisfies` instead of type assertions
- Never use `!` non-null assertion

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

## Project Structure

```
src/                     # Svelte 5 frontend
├── lib/
│   ├── components/      # UI components
│   │   ├── effects-page.svelte
│   │   ├── engine/      # Engine settings
│   │   ├── history/     # History components
│   │   ├── hud/         # HUD overlay
│   │   ├── landing/     # Marketing landing page
│   │   ├── settings/    # Settings tabs
│   │   ├── ui/          # shadcn-svelte
│   │   ├── profiles-page.svelte
│   │   ├── play-page.svelte
│   │   └── ...
│   └── utils.ts         # Utilities (cn, portal action)
└── routes/              # SvelteKit routes
    ├── settings/        # Settings page
    ├── effects/         # Effects page
    ├── engine/          # Engine page
    ├── history/         # History page
    ├── profiles/        # Profiles page
    ├── onboarding/      # First-run setup
    └── hud/             # HUD overlay

src-tauri/src/           # Rust backend
├── main.rs              # Entry point, IPC registration
├── config/              # Persistence modules
│   └── tts.rs           # TTS config types & engine enum
├── commands/            # IPC handlers
│   └── tts/             # Synthesis commands
├── tts/                 # TTS backend implementations
│   ├── edge.rs          # Edge TTS
│   ├── openai.rs        # OpenAI
│   ├── elevenlabs.rs    # ElevenLabs
│   ├── cartesia.rs      # Cartesia
│   ├── google.rs        # Google
│   ├── microsoft.rs     # Microsoft
│   ├── http.rs          # Generic HTTP
│   ├── cli.rs           # Local CLI engines
│   └── catalog.rs       # Engine catalog types
├── clipboard.rs         # Double-copy detection
├── audio.rs             # Playback
├── post_process.rs      # LLM post-processing
└── sanitize/            # Text normalization
```

## Additional Files

- **plans/**: Contains plan files for various implementation tasks (e.g., auto-updater, Hud synthesis, etc.).
- **scripts/**: PowerShell and JavaScript scripts for automation, including install scripts for various TTS engines, chatterbox, kitten, lib, piper, etc.
- **src-tauri/src/commands/**: Rust command modules for TTS, audio, playback, post-processing, etc.
- **src-tauri/src/sanitize/**: Text normalization modules for cleanup, markdown, TTS normalization.
- **src/lib/components/**: Additional UI components for settings, effects, playback, etc.

## Documentation

- **docs/ (Public Docs)**: **CONTRIBUTING.md** (contribution guidelines).

- **docs_internal/ (Internal Docs)**: **project-overview.md** (project context and key decisions), **requirements.md** (feature requirements and traceability), **architecture.md** (system architecture and design), **development_guide.md** (setup and development workflow), **tts_backends.md** (TTS engine integration guide), **brutalist_design.md** (UI design system and aesthetics), **roadmap.md** (development roadmap and phases), **code-patterns-reference.md** (Svelte 5, Rust, and Tauri IPC code examples).

<!-- rtk-instructions v2 -->

## RTK (Rust Token Killer) - Token-Optimized Commands

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

<!-- /rtk-instructions -->
