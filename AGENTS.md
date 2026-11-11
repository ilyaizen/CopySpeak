# AGENTS.md

CopySpeak: A lightweight and modern cross-platform AI text-to-speech orchestrator that reads clipboard text aloud when double-copied. Stack: Svelte 5, Tauri 2.

## Rules

- Fix causes, not symptoms. Report adjacent problems, don't fix them.
- Never stub, loosen, or comment out check to get green. Broken and loud beats silent and wrong.
- Unsure or blocked: ask. State assumptions, surface tradeoffs, push back on over-engineering.

## Plain English

- Use ASD-STE100 style: write active sentences under 20 words. State one action per sentence. Put conditions before commands.
- Use common verbs: use, fix, show, change, must, can. Avoid utilize, leverage, delve, robust, seamless, crucial, paradigm, should, would, and may.
- Lead with facts or results. Skip preambles and recaps. After code edits, list changed paths only, one per line, unless asked otherwise.
- Use spaced en dashes (' – ') for parenthetical dashes. Never use em dashes in copy text.

## Current priorities

- Next release: low time-to-first-audio (TTFA) and a small VRAM footprint, so local engines run on low-VRAM GPUs and CPU-only machines.
- Chatterbox is dropped (2026-10-20): too large, and not coming back. Its WIP lived on `feat/chatterbox`, now deleted. Do not reintroduce it without asking.
- Judge new engines by TTFA and VRAM footprint before adding them.

## Workflow

- Use `i-have-adhd` skill: next action first, numbered steps, no preamble.
- During edits, run focused tests relevant to the change; batch related frontend test files in one invocation. Reserve full gates for the final changes before merging.
- Before merging, run `bun run check && bun run test` (vitest; bare `bun test` fails ~39 document-dependent tests) and relevant backend gates. `bun run tauri dev` is already running.
- Run checks (`git diff --check` included) and commits only after explicit confirmation. Ask before concluding task.
- Run the checks yourself before merging: all three workflows are `workflow_dispatch`, so PRs and `main` pushes trigger no CI.
- Run `rustfmt` on touched Rust files only; `cargo fmt --all` marks unrelated CRLF-tracked files modified.
- Check credential presence without printing `.env` values or `%APPDATA%\CopySpeak\config.json` fields; both hold plaintext provider keys, so keep secret files out of command output and read only named non-secret fields.
- Hand-off prompts carry only what next session can't access: decisions made, dead ends, ongoing states, next steps.
- Leave visual verification to the user; skip image previews.
- Commits: [Conventional Commits](https://www.conventionalcommits.org/). Branches: squash-merge into `main`, then delete the branch (local and origin) in the same pass.

Internal docs (untracked): start at `docs_internal/README.md`.

## Design and review (EGM)

EGM is installed as the `egm@egm` Claude Code plugin (user scope). Source: https://github.com/google/elephant-goldfish-model.

EGM artifact directory: docs_internal/egm

- Use `/egm:egm-brainstorm` to test an idea and write `concepts_brief.md`.
- Use `/egm:egm-draft` to turn a brief into a design doc, section by section.
- Use `/egm:egm-review <draft path>` to run the Goldfish review on a draft.
- Use `/egm:egm-resolve` to work through review findings.
- Use `/egm:egm-drift-report` to compare design docs with the current code.
- Write drafts and reports to `docs_internal/egm`. Ilya approves before any spec moves to `docs/designs/`.
- Goldfish reviews need fresh subagents. If the harness cannot spawn them, stop the review and say so.
- Skip EGM for one-line fixes. Use it for features and architecture changes only.

## Next step

- End with one concrete next step; user approves. Use shape: `If you want, I can do X next. React with ✅ to run it.` Several viable: list up to 3, one line each, best first.

## Agent skills

### Issue tracker

GitHub Issues on `ilyaizen/CopySpeak`, via `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`, created lazily. See `docs/agents/domain.md`.

## Failure log

Every line exists because it went wrong at least once. On mistake, correction, or undocumented discovery about the codebase, add one imperative line describing correct behaviour: here if every session needs it, else under its area in [`docs/agents/failure-log.md`](docs/agents/failure-log.md). A workflow fix goes in a skill under `.agents/skills/` instead. Include it in the same commit and mention it in the summary. Periodically shorten this section to save tokens.

- In a fresh worktree, run `bun install` in `browser-extension/` as well as the root; without its `@types/chrome`, `vp check` fails with `Cannot find type definition file for 'chrome'`.
- Use three-way `cfg(target_os = ...)` (windows/macos/linux) for platform seams; `cfg(not(windows))` makes macOS compile Linux-only code (wl-clipboard, gtk-layer-shell) that cannot build or run on darwin.
- Treat a Windows-green `cargo check` as silent about `cfg(not(windows))` code; grep cfg-gated blocks for a trait's methods before deleting an "unused" import.
- Verify a subsystem is actually wired before extending it — grep `mod` declarations and component mounts; the parity checklist once described a post-processing enum system that was never compiled or mounted.
- Feature-detect an optional browser or DOM API with a `!== undefined` guard or `try/catch`; `vp check`'s anti-slop `no-runtime-typeof` rejects `typeof` checks.
- Treat a `cargo test` link failure `LNK1104` on the test exe as another session's test binary still running: wait for that process, do not add a second target dir.

### Area gotchas

Before editing an area, read its section in [`docs/agents/failure-log.md`](docs/agents/failure-log.md):

- **Settings**: stacked sections, scroll-spy, row tooltips, Experimental-gated sections
- **Playback**: webview audio, `pcm-stream.ts`, `TimeStretcher`, effects, streaming, HUD events, ducking
- **Captions**: word timings, caption metadata, `.captions.json`
- **Engines**: installers, Python wrappers, daemon protocol, CUDA, adding a `TtsEngine`
- **Browser companion**: native host, extension protocol, page highlights
- **History & cache**: history UI, replay, speech cache
- **Text pipeline**: sanitizer (`tts_normalize.rs`), pagination, non-English text
- **Landing page**: `static/` asset references, version strings in locales
- **Linux**: HUD window, Wayland
- **Release**: version bump, GitHub release workflow
- **i18n**: locale strings, `$_`
- **Frontend checks**: component tests, mocks, `vp check`
- **Icons**: tray artwork and build assets
- **Experimental gating**: feature gates and saved settings
- **DSH plugin (`copyspeak-dsh`)**: selection surface, keyed-slot shadowing, interaction dispatch
