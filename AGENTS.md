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

- `bun format` - prettier format.
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
- **Always create a new branch** before starting any new feature, fix, refactor, or version work. Never work on an existing branch that isn't yours.
- Work on feature branches: `feature/`, `fix/`, `refactor/`, `docs/`
- Use versioned dev branches for releases: `develop/0.1.0`, `develop/0.2.0`, etc.
- Open PRs targeting `main` (or `develop/*` for larger efforts)
- **Bump version when finishing a task**: after completing work and before opening a PR, run `bun run bump` (patch), `bun run bump:minor`, or `bun run bump:major` to bump all version files (package.json, Cargo.toml, tauri.conf.json, version.ts, README.md). Choose the bump type based on the scope of changes. Commit the version bump as part of the PR.

## Best Practices

- Follow existing code patterns
- Keep responses concise (1-3 sentences)
- Comments explain "why" not "what"
- Update CHANGELOG.md for notable changes (features, fixes, breaking changes). Follow [Keep a Changelog](https://keepachangelog.com/) sections: Added, Changed, Deprecated, Removed, Fixed, Security

## Reasoning Discipline

- Prefer sharp model over pretty one.
- If uncertain, say what known/inferred/open.

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

- **docs/ (Public Docs)**: **CONTRIBUTING.md** (contribution guidelines).

- **docs_internal/ (Internal Docs)**: **project-overview.md** (project context and key decisions), **requirements.md** (feature requirements and traceability), **architecture.md** (system architecture and design), **development_guide.md** (setup and development workflow), **tts_backends.md** (TTS engine integration guide), **brutalist_design.md** (UI design system and aesthetics), **roadmap.md** (development roadmap and phases), **code-patterns-reference.md** (Svelte 5, Rust, and Tauri IPC code examples).

## Final Reply Tails

When answer suggests next step, end with compact executable tail, not vague fluff.

Use shape: `If you want, I can do X next. React with ✅ to run it.`

Rules: Offer ✅ hook only if next step wired to real action. One suggested next step. Don't use tail for pure facts, refusals, one-off answers with no real follow-up.

## Final Rule

Be assistant you'd want to talk to at 2AM. Not corporate drone. Not sycophant. Just useful.

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

Full command reference (which tools have dedicated filters, and their savings): the `rtk-commands` skill in `.agents/skills/rtk-commands/`.
<!-- /rtk-instructions -->
