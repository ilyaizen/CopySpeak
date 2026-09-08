# AGENTS.md

CopySpeak: A lightwieight and modern AI text-to-speech orchestrator for Windows that reads clipboard text aloud when double-copied. Stack: Svelte 5, Tauri 2.

## Rules

- Fix causes, not symptoms. Report adjacent problems, don't fix them.
- Never stub, loosen, or comment out check to get green. Broken and loud beats silent and wrong.
- Unsure or blocked: ask. State assumptions, surface tradeoffs, push back on over-engineering.

## Workflow

- Use `i-have-adhd` skill: next action first, numbered steps, no preamble.
- Never run `bun run tauri dev` to verify change; already running. Use `bun run check && bun run test`.
- Never run checks or commit without explicit confirmation. Ask before concluding task.
- End with one concrete next step; user approves. Use shape: `If you want, I can do X next. React with ✅ to run it.` Several viable: list up to 3, one line each, best first.
- Hand-off prompts only include what next session can't access: decisions made, dead ends, ongoing states, next steps. Don't repeat AGENTS.md.
- Avoid image-previewing. Only user verifies/approves visually.
- Commits: [Conventional Commits](https://www.conventionalcommits.org/).

## Documentation

- **docs/ (Public Docs)**: **CONTRIBUTING.md** (contribution guidelines).

- **docs_internal/ (Internal Docs)**: **project-overview.md** (project context and key decisions), **requirements.md** (feature requirements and traceability), **architecture.md** (system architecture and design), **development_guide.md** (setup and development workflow), **tts_backends.md** (TTS engine integration guide), **brutalist_design.md** (UI design system and aesthetics), **roadmap.md** (development roadmap and phases), **code-patterns-reference.md** (Svelte 5, Rust, and Tauri IPC code examples).

## Keeping this file current

File logs failures, not wishlist. Every line below exists because it went wrong at least once. On mistake, correction, or undocumented discovery about codebase:

1. Add one line to active failure log below, imperative, describing correct behaviour.
2. Keep specific to this repo. General advice belongs nowhere.
3. Fix is workflow not rule: put in `.agents/skills/` and link from here.
4. Include change in same commit, mention in summary.

Keep active failure log short: entries for work not in `## Active work` move to [`.agents/failure-log.md`](.agents/failure-log.md). Loaded every session; long context makes you less reliable, not more. Architecture detail outgrows usefulness: move to `.agents/architecture.md` or a skill.

## Active failure log

- Check credential presence without printing `.env` values; never grep secret files into command output.
- Preserve native caption intervals with their generated audio through stream framing, cache/history replay, and sample-count-based fragment concatenation; never scale word timings by text weights.
- Reschedule unstarted PCM sources when playback speed or pitch changes; setting playbackRate does not move their scheduled start times.

- Drive HUD captions from the audible fragment's audio clock; synthesis events may describe a later fragment, and incoming PCM chunks must not resume a user-paused stream.
- Update `play-page.svelte`'s browser mock config when adding required `AppConfig` fields, matching backend defaults.
- Treat `git diff --check` as a check; do not run it before explicit confirmation.
- Present history batches as one reading in `recent-history.svelte`; preserve fragment order for full text, playback, and whole-reading deletion.
- Show pagination as a part count on history rows; keep Play/Stop/Replay on the reading's button instead of adding a lower playback bar.
- Resolve history voice labels by both engine and voice ID using profiles/catalog; retain raw IDs for playback and tooltips.
- Hide the playback button's decorative spinner from accessibility naming so synthesis keeps the action named “Stop”.
- Clear the shared audio queue on decode/play failure and invalidate pending decoding on Stop so history retries work and stopped readings cannot restart.
- Translate vertical mouse-wheel input into horizontal scrolling on the Play history rail, and end its five-reading preview with a link to full History.
- Keep the wheel-driven history rail free of scroll snapping and reverse wheel deltas in RTL; DOM-only tests cannot verify browser snapping behavior.
- Close the History-only Delete conditional inside its reading row, before `</li>`; do not place its closing block in the View more card.
- Local engine wrappers speak daemon protocol v2 (`READY 2`); emit 16-bit signed LE PCM, since `pcm-stream.ts` drops any other `bits_per_sample`.
- Adding a `TtsEngine` variant: also update the catalog test's engine list and entry count, and the `Record<TtsEngine, number>` fixtures in `html-templates.test.ts` and `html-export.test.ts`.
- Register CUDA DLL directories with `os.add_dll_directory` inside the wrapper; Python 3.8+ ignores `PATH` for extension-module dependencies, so setting it from Rust does nothing.
- Emit the streaming `is_final` marker on the last fragment only; an intermediate one arms the player's completion timer mid-passage.
- KittenTTS 0.8.1 (the pinned wheel) takes only `KittenTTS(model_name, cache_dir)`; select the GPU by replacing `tts.model.session`, not with a `backend=` kwarg that only exists on `main`.
- Pass `uv init` its target directory positionally (`uv init --bare --name X <dir>`); uv 0.12+ hard-errors on `uv --project <dir> init`, which only shows up when creating a fresh engine project.

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
