# Failure log by area

Area gotchas from shipped work, one imperative line each. `AGENTS.md` keeps the lines every session needs and points here by area; read an area's section before editing it.

## Playback

- Route playback speed and pitch through `TimeStretcher` (SoundTouch `pitch` setter, then `stretch.tempo = speed / pitch`); keep `playbackRate` at 1 on PCM sources.
- On a rate change, re-stretch only the native chunks of unstarted PCM sources; rendered audio keeps the rate it was rendered at.
- Give `TimeStretcher`'s first output after every reset a ~5 ms fade-in and hold the last ~5 ms of input out of the WSOLA flush; cold-start or hard speech-to-silence steps click at fragment seams.
- Anchor PCM skip targets to `_stallPosition` in `pcm-stream.ts`: `_mediaNow()` collapses to 0 whenever no scheduled source covers the playhead (synthesis stalls, final drain, the START_DELAY window after a rebuild), so `mediaNow - n` alone makes ±5s jump to the timeline start.
- Re-sync `playbackStore` from its own `config-changed` listener; pages that only `set_config` never update it.
- Gate streaming playback off when the active profile has an effect: warm daemons stream via `CliTtsBackend::supports_streaming`, and the PCM scheduler has no effect chain — only the `audio-fragment-ready` path applies effects.
- Emit the streaming `is_final` marker on the last fragment only; an intermediate one arms the player's completion timer mid-passage.
- Signal a failed streamed reading with `playback-stream-failed` and its `session_id`, never `playback-stop` (which cuts every reading): a dead stream never sends the final marker that `playback-finished` waits for.
- Allow `createWaveShaper` only as a directly-called member; reject local declarations and every other symbol containing "shape".
- Drive HUD captions from the audible fragment's audio clock; synthesis events may describe a later fragment, and incoming PCM must not resume a user-paused stream.
- `hud:*` events fire only while the HUD is enabled; main-window UI reads `playbackStore.caption` and `reading-started`, never HUD events.
- Duck other apps per-session and skip our whole process tree, not our PID: playback is in the webview, so the audio session belongs to a `msedgewebview2` child; persist originals to `duck-state.json` because per-app mixer volumes survive a crash.
- "Beeps" in generated audio: suspect unpronounceable glyphs in the text before the playback code; keep `remove_unpronounceable`'s symbol ranges ahead of what vendors confound.

## Captions

- Captions dying on every surface at once means the active engine's provider stopped emitting timing metadata: check the logs for "Disabling ... captions", verify other engines still write `.captions.json`, and run the `#[ignore]`d `live_caption_probe` before releases.
- Preserve native caption intervals with their generated audio through stream framing, cache/history replay, and sample-count-based fragment concatenation; never scale word timings by text weights.
- Emit daemon caption frames as cumulative request snapshots (text and ms covering every sentence so far): live, batch, and history consumers each keep only the newest frame, so sentence-local frames validate but kill highlights after the first chunk.
- Reject unusable caption metadata without rejecting valid PCM; propagate caption clears to live/cache/history consumers, keep audio/protocol errors fatal, and retain source text for untimed fragments when joining readings.
- Round estimated caption milliseconds once, after accumulating sentence offsets; per-sentence rounding re-rounds sentence-boundary pairs apart by 0.1 ms, and a `start_ms < audio_end` verdict clears the whole reading's captions.
- ElevenLabs with-timestamps streams carry alignment on the leading record(s) and stream later records as audio alone; hard-degrade only when no aligned record ever arrives.

## Engines

- Local engine wrappers speak daemon protocol v2 (`READY 2`) and emit 16-bit signed LE PCM; `pcm-stream.ts` drops any other `bits_per_sample`.
- Adding a `TtsEngine` variant: update the catalog test's engine list and entry count, plus the `Record<TtsEngine, number>` fixtures in `html-templates.test.ts` and `html-export.test.ts`.
- Wrapper fixes reach an installed engine only when its installer re-runs (for wrapper-only fixes, `cp` the repo file into `engines/<e>/scripts/`), and shared wrapper functions must be backported to EVERY engine — `diff` the engine wrappers against each other when touching one.
- Bundle each installer wrapper directory in `tauri.conf.json`; a lone `install-*.ps1` breaks app-driven installs in dev and release builds.
- Run Engines-page health checks in one-shot mode; daemon prewarming plus the immediate fallback launches two copies of large local models.
- Pass the install dialog's GPU-runtime choice as `-Cuda`; a CUDA profile cannot run against the default CPU-only install.
- Send installer voice ids as one comma-joined `-Voices` argument and split them with `ConvertTo-VoiceIds`; `powershell -File` binds neither `-Voices a,b` nor `-Voices a b` as an array.
- Pass `uv init` its target directory positionally (`uv init --bare --name X <dir>`); uv 0.12+ hard-errors on `uv --project <dir> init`.
- Keep the cu126-index CUDA setup Windows-only in `Add-CudaRuntime`; the `install-*.sh` Linux ports pin plain PyPI torch/torchaudio.
- Register CUDA DLL directories with `os.add_dll_directory` AND prepend them to `PATH` inside the wrapper: Python 3.8+ ignores `PATH` for extension-module deps, while onnxruntime's `LoadLibrary` of `cudnn64_9.dll` ignores `add_dll_directory`.
- Pin torchaudio beside torch in CUDA profiles (same version, same cu126 index); unpinned PyPI torchaudio (2.11) beside torch 2.8.0+cu126 dies at `import torchaudio` with WinError 127 (ABI mismatch). Windows cu126 wheels bundle the CUDA DLLs in `torch\lib`; nvidia-* wheels are Linux-only.
- Drop CPU `onnxruntime` from GPU engine projects with `[tool.uv] exclude-dependencies` + `uv sync --reinstall-package onnxruntime-gpu`; `uv remove` is a no-op (transitive dep) and both wheels write the same `onnxruntime/` files.
- KittenTTS 0.8.1 takes only `KittenTTS(model_name, cache_dir)`; select the GPU by replacing `tts.model.session`, not with a `backend=` kwarg that exists only on `main`.
- Pass Kokoro exporter options by their long names through `Invoke-Uv`; PowerShell binds `-o` as an ambiguous common parameter before uv runs.
- Export Kokoro's duration-capable model under isolated Python 3.12; Kokoro 0.8.4's NumPy 1.x dependency cannot use the engine's Python 3.13 wheels.
- Filter Kokoro's misaki phonemes through the model vocab before inference; misaki emits unpronounceable punctuation as literal phonemes, and only a missing _letter_ phoneme is a real pronunciation gap worth aborting on.

## Browser companion

- Gate every browser-protocol addition on a capability: the native host's `ClientMessage` is `deny_unknown_fields` and drops the relay on any unknown field or action, and older extensions disconnect on any app event type they cannot parse.
- Bind the browser follow to each clip's own `session_id` when it activates, never to a `browser-reading` event: a queued speak starts while an earlier clip still plays, and that clip's drain would complete the queued session. Gate trigger word events on `playback-fragment-audible` too, so a queued speak projects nothing until its own tile is audible.
- Match a page selection to copied text ignoring whitespace (`matchCopiedText`): the Windows clipboard has CRLF and Chrome's blank lines between paragraphs, while `captureSelection` keeps source whitespace and joins blocks with one `\n`.
- Map browser caption words onto the raw selection with `text_map::align`'s word alignment; never re-derive the sanitizer's rewrites in a second place, and keep the highlight verdict per word and per fragment.
- Release the old browser reading before asking the document to capture its replacement; a later disconnect erases the new paragraph/selection anchor.
- Start the browser session's state ticker at accept time, not after `speak_queued` returns; a post-synthesis ticker pins the companion panel at `buffering` for the whole generation while audio already plays.
- The browser pipe serves one client at a time; the native host retries `ERROR_PIPE_BUSY` with `WaitNamedPipeW` instead of failing the reading.

## History & cache

- Present history batches as one reading in `recent-history.svelte`; preserve fragment order for full text, playback, and whole-reading deletion.
- Show pagination as a part count on history rows; keep Play/Stop/Replay on the reading's button instead of adding a lower playback bar.
- Keep Recent History as three vertical, text-first readings with View all history; let mouse-wheel input scroll normally on both pages.
- Keep actions inside each reading's menu and bulk selection on the full History page; filtering must preserve complete reading batches.
- Resolve history voice labels by both engine and voice ID using profiles/catalog; retain raw IDs for playback and tooltips.
- Restore only history's saved engine, voice and speed; retain current profile pitch/effects, and report a missing voice profile instead of generating with a different voice.
- Hide the playback button's decorative spinner from accessibility naming so synthesis keeps the action named “Stop”.
- Clear the shared audio queue on decode/play failure and invalidate pending decoding on Stop so history retries work and stopped readings cannot restart.
- When a cache test asserts an exact pagination count, set `fragment_size` above each intended sentence length; overlong sentences are correctly force-split into extra fragments.

## Linux

- On Linux launch with `GDK_BACKEND=x11` and show() the HUD window before `set_position`; Wayland toplevels cannot self-position.
- Hide the HUD's native window when the frontend emits global `hud:stop` on Linux; webview playback never trips the Rust AudioPlayer monitor.

## Release

- `bun run bump` derives the new version only from `src/lib/version.ts`; verify all six ✅ lines after bumping. It never touches `browser-extension/` — bump the extension's `manifest.json` + `package.json` by hand on its own cadence.
- Pass `draft: true` to `softprops/action-gh-release` when attaching extra assets to tauri-action's draft release, or the step publishes the release unreviewed.

## i18n

- Grep components for `>Text<` and `label="Text"` after an i18n pass; literal UI text bypasses `$_` and passes locale parity.
- Pass caught errors to `$_` values as `String(e)`; a catch binding is `unknown` and svelte-check rejects it.
- Run `vp fmt src/lib/i18n/types.ts` after `node scripts/generate-i18n-types.js`; raw generator output reflows `NestedKeyOf` and adds trailing whitespace, so the diff picks up noise.
