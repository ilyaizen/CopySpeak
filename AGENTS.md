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
- Branches: squash-merge into `main`, then delete the branch (local and origin) in the same pass — never leave merged branches behind.

## Documentation

- **Public (tracked)**: README.md, CHANGELOG.md, BROWSER_EXTENSION.md, DESIGN.md (historical spec), docs/CONTRIBUTING.md, docs/engines.md, docs/profile-engine-settings.md, docs/agent-voice.md.
- **Internal (untracked)**: `docs_internal/` — start from `docs_internal/README.md` (the index). Superseded and historical docs live in `docs_internal/archive/`.

## Keeping this file current

File logs failures, not wishlist. Every line below exists because it went wrong at least once. On mistake, correction, or undocumented discovery about codebase:

1. Add one line to active failure log below, imperative, describing correct behaviour.
2. Keep specific to this repo. General advice belongs nowhere.
3. Fix is workflow not rule: put in `.agents/skills/` and link from here.
4. Include change in same commit, mention in summary.

Keep the active log short — it loads every session; long context makes you less reliable, not more. One imperative line per entry; narrative (causes, dead ends, evidence) belongs in [`.agents/failure-log.md`](.agents/failure-log.md), not here. When a feature area ships and stabilizes, move its entries there verbatim. Architecture detail outgrows usefulness: move to `docs_internal/architecture.md` or a skill.

## Active failure log

- Pass Kokoro exporter options by their long names through `Invoke-Uv`; PowerShell binds `-o` as an ambiguous common parameter before uv runs.
- Export Kokoro's duration-capable model under isolated Python 3.12; Kokoro 0.8.4's NumPy 1.x dependency cannot use the engine's Python 3.13 wheels.
- Check credential presence without printing `.env` values; never grep secret files into command output.
- Route playback speed and pitch through `TimeStretcher` (SoundTouch `pitch` setter, then `stretch.tempo = speed / pitch`); keep `playbackRate` at 1 on PCM sources.
- On a rate change, re-stretch only the native chunks of unstarted PCM sources; rendered audio keeps the rate it was rendered at.
- Give `TimeStretcher`'s first output after every reset a ~5 ms fade-in and hold the last ~5 ms of input out of the WSOLA flush; cold-start or hard speech-to-silence steps click at fragment seams.
- Drive HUD captions from the audible fragment's audio clock; synthesis events may describe a later fragment, and incoming PCM must not resume a user-paused stream.
- Update `play-page.svelte`'s browser mock config when adding required `AppConfig` fields, matching backend defaults.
- Re-sync `playbackStore` from its own `config-changed` listener; pages that only `set_config` never update it.
- Treat `git diff --check` as a check; do not run it before explicit confirmation.
- Local engine wrappers speak daemon protocol v2 (`READY 2`) and emit 16-bit signed LE PCM; `pcm-stream.ts` drops any other `bits_per_sample`.
- Gate streaming playback off when the active profile has an effect: warm daemons stream via `CliTtsBackend::supports_streaming`, and the PCM scheduler has no effect chain — only the `audio-fragment-ready` path applies effects.
- Adding a `TtsEngine` variant: update the catalog test's engine list and entry count, plus the `Record<TtsEngine, number>` fixtures in `html-templates.test.ts` and `html-export.test.ts`.
- Register CUDA DLL directories with `os.add_dll_directory` AND prepend them to `PATH` inside the wrapper: Python 3.8+ ignores `PATH` for extension-module deps, while onnxruntime's `LoadLibrary` of `cudnn64_9.dll` ignores `add_dll_directory`.
- Pin torchaudio beside torch in CUDA profiles (same version, same cu126 index); unpinned PyPI torchaudio (2.11) beside torch 2.8.0+cu126 dies at `import torchaudio` with WinError 127 (ABI mismatch). Windows cu126 wheels bundle the CUDA DLLs in `torch\lib`; nvidia-* wheels are Linux-only.
- Emit the streaming `is_final` marker on the last fragment only; an intermediate one arms the player's completion timer mid-passage.
- KittenTTS 0.8.1 takes only `KittenTTS(model_name, cache_dir)`; select the GPU by replacing `tts.model.session`, not with a `backend=` kwarg that exists only on `main`.
- Pass `uv init` its target directory positionally (`uv init --bare --name X <dir>`); uv 0.12+ hard-errors on `uv --project <dir> init`.
- Allow `createWaveShaper` only as a directly-called member; reject local declarations and every other symbol containing "shape".
- Double-copy and browser readings run `speak_queued`; Play page, hotkey and control server run `speak_now` — change both paths together.
- Treat `speak_now`'s return as playback-START, not completion: only the webview store's `finishPlayback`/`handleStop` → `playback-finished` → `playback_signal` chain means completion, and every terminal path must emit it or blocking `/speak --wait` callers hang until the 600 s cap.
- `hud:*` events fire only while the HUD is enabled; main-window UI reads `playbackStore.caption` and `reading-started`, never HUD events.
- Filter Kokoro's misaki phonemes through the model vocab before inference; misaki emits unpronounceable punctuation as literal phonemes, and only a missing _letter_ phoneme is a real pronunciation gap worth aborting on.
- Captions dying on every surface at once means the active engine's provider stopped emitting timing metadata: check the logs for "Disabling ... captions", verify other engines still write `.captions.json`, and run the `#[ignore]`d `live_caption_probe` before releases.
- The app loads secrets from `<exe-dir>/.env` (`src-tauri/target/debug/.env` in dev), not the repo-root `.env`.
- `bun run bump` derives the new version only from `src/lib/version.ts`; verify all seven ✅ lines after bumping.
- Pass `draft: true` to `softprops/action-gh-release` when attaching extra assets to tauri-action's draft release, or the step publishes the release unreviewed.
- "Beeps" in generated audio: suspect unpronounceable glyphs in the text before the playback code; keep `remove_unpronounceable`'s symbol ranges ahead of what vendors confound.
- Run `rustfmt` only on touched Rust files; `cargo fmt --all` marks unrelated CRLF-tracked Rust files modified on this worktree.
- Bundle each installer wrapper directory in `tauri.conf.json`; a lone `install-*.ps1` breaks app-driven installs in dev and release builds.
- Run Engines-page health checks in one-shot mode; daemon prewarming plus the immediate fallback launches two copies of large local models.
- Pass the install dialog's GPU-runtime choice as `-Cuda`; a CUDA profile cannot run against the default CPU-only install.
- Keep the cu126-index CUDA setup Windows-only in `Add-CudaRuntime`; the `install-*.sh` Linux ports pin plain PyPI torch/torchaudio.
- On Linux launch with `GDK_BACKEND=x11` and show() the HUD window before `set_position`; Wayland toplevels cannot self-position.
- Hide the HUD's native window when the frontend emits global `hud:stop` on Linux; webview playback never trips the Rust AudioPlayer monitor.
- Run frontend tests as `bun run test` (vitest); bare `bun test` fails ~39 document-dependent tests that pass in CI.
- Send installer voice ids as one comma-joined `-Voices` argument and split them with `ConvertTo-VoiceIds`; `powershell -File` binds neither `-Voices a,b` nor `-Voices a b` as an array.
- Treat a Windows-green `cargo check` as silent about `cfg(not(windows))` code; grep cfg-gated blocks for a trait's methods before deleting an "unused" import.
- Drop CPU `onnxruntime` from GPU engine projects with `[tool.uv] exclude-dependencies` + `uv sync --reinstall-package onnxruntime-gpu`; `uv remove` is a no-op (transitive dep) and both wheels write the same `onnxruntime/` files.
- Wrapper fixes reach an installed engine only when its installer re-runs (for wrapper-only fixes, `cp` the repo file into `engines/<e>/scripts/`), and shared wrapper functions must be backported to EVERY engine — `diff` the engine wrappers against each other when touching one.

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
