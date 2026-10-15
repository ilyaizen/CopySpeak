# AGENTS.md

CopySpeak: A lightweight and modern cross-platform AI text-to-speech orchestrator that reads clipboard text aloud when double-copied. Stack: Svelte 5, Tauri 2.

## Rules

- Fix causes, not symptoms. Report adjacent problems, don't fix them.
- Never stub, loosen, or comment out check to get green. Broken and loud beats silent and wrong.
- Unsure or blocked: ask. State assumptions, surface tradeoffs, push back on over-engineering.

## Workflow

- Use `i-have-adhd` skill: next action first, numbered steps, no preamble.
- Verify with `bun run check && bun run test` (vitest; bare `bun test` fails ~39 document-dependent tests). `bun run tauri dev` is already running.
- Run checks (`git diff --check` included) and commits only after explicit confirmation. Ask before concluding task.
- Run the checks yourself before merging: all three workflows are `workflow_dispatch`, so PRs and `main` pushes trigger no CI.
- Run `rustfmt` on touched Rust files only; `cargo fmt --all` marks unrelated CRLF-tracked files modified.
- Check credential presence without printing `.env` values; keep secret files out of command output.
- User skims long messages: repeat every unanswered question under a bold **Open questions** heading right above the next-step line, in every message, until answered.
- End with one concrete next step; user approves. Use shape: `If you want, I can do X next. React with ✅ to run it.` Several viable: list up to 3, one line each, best first.
- Hand-off prompts carry only what next session can't access: decisions made, dead ends, ongoing states, next steps.
- Leave visual verification to the user; skip image previews.
- Commits: [Conventional Commits](https://www.conventionalcommits.org/). Branches: squash-merge into `main`, then delete the branch (local and origin) in the same pass.

Internal docs (untracked): start at `docs_internal/README.md`.

## Agent skills

### Issue tracker

GitHub Issues on `ilyaizen/CopySpeak`, via `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`, created lazily. See `docs/agents/domain.md`.

## Failure log

Every line exists because it went wrong at least once. On mistake, correction, or undocumented discovery about the codebase, add one imperative line describing correct behaviour: here if every session needs it, else under its area in [`docs/agents/failure-log.md`](docs/agents/failure-log.md). A workflow fix goes in a skill under `.agents/skills/` instead. Include it in the same commit and mention it in the summary.

- Double-copy and browser readings run `speak_queued`; Play page, hotkey and control server run `speak_now` — change both paths together.
- Wire playback controls through `playbackStore` (webview audio); Rust `toggle_pause`/`skip_forward`/`skip_backward` commands drive only the Rodio safety-net player, which is never what you hear.
- Treat `speak_now`'s return as playback-START, not completion: only the webview store's `finishPlayback`/`handleStop` → `playback-finished` → `playback_signal` chain means completion, and every terminal path must emit it or blocking `/speak --wait` callers hang until the 600 s cap.
- Branch every synthesis save site on `history.enabled`: on logs and saves to the history folder, off calls `save_speech_cache` and logs nothing; `find_cached_speech` must read the same store.
- Update `play-page.svelte`'s browser mock config when adding required `AppConfig` fields, matching backend defaults.
- The app loads secrets from `<exe-dir>/.env` (`src-tauri/target/debug/.env` in dev), not the repo-root `.env`.
- In a fresh worktree, run `bun install` in `browser-extension/` as well as the root; without its `@types/chrome`, `vp check` fails with `Cannot find type definition file for 'chrome'`.
- Use three-way `cfg(target_os = ...)` (windows/macos/linux) for platform seams; `cfg(not(windows))` makes macOS compile Linux-only code (wl-clipboard, gtk-layer-shell) that cannot build or run on darwin.
- Treat a Windows-green `cargo check` as silent about `cfg(not(windows))` code; grep cfg-gated blocks for a trait's methods before deleting an "unused" import.
- Verify a subsystem is actually wired before extending it — grep `mod` declarations and component mounts; the parity checklist once described a post-processing enum system that was never compiled or mounted.
- Import test APIs from `vite-plus/test`, not `vitest`; `vp check`'s `prefer-vite-plus-imports` rejects the bare import.
- Hold a mocked async API on a promise the test opens when proving a race; `vi.waitFor` polls, so an ungated mock finishes the whole setup before the test's next step and the race never happens.
- Type test catalog maps with `satisfies Record<string, Catalog>`, not a `: Record<…>` annotation; `vp check`'s anti-slop `no-known-value-widening` rejects the annotation.
- Let one dynamic `import()` of a `vi.mock`ed module settle before code under test starts another: concurrent ones can resolve the real module, so the mock never sees the race.
- Unlisten a Tauri `listen` that resolves after its component's `onDestroy`: HMR can destroy a component mid-`onMount`, and the orphaned listener double-fires (one double-copy spoke twice).

### Area gotchas

Before editing an area, read its section in [`docs/agents/failure-log.md`](docs/agents/failure-log.md):

- **Playback**: webview audio, `pcm-stream.ts`, `TimeStretcher`, effects, streaming, HUD events, ducking
- **Captions**: word timings, caption metadata, `.captions.json`
- **Engines**: installers, Python wrappers, daemon protocol, CUDA, adding a `TtsEngine`
- **Browser companion**: native host, extension protocol, page highlights
- **History & cache**: history UI, replay, speech cache
- **Config**: `migrate_tts_config`, schema versions, profile migrations
- **Text pipeline**: sanitizer (`tts_normalize.rs`), pagination, non-English text
- **Landing page**: `static/` asset references, version strings in locales
- **Linux**: HUD window, Wayland
- **Release**: version bump, GitHub release workflow
- **i18n**: locale strings, `$_`
