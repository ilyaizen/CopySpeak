# Failure log by area

Area gotchas from shipped work, one imperative line each. `AGENTS.md` keeps the lines every session needs and points here by area; read an area's section before editing it.

## Playback

- Route playback speed and pitch through `TimeStretcher` (SoundTouch `pitch` setter, then `stretch.tempo = speed / pitch`); keep `playbackRate` at 1 on PCM sources.
- On a rate change, re-stretch only the native chunks of unstarted PCM sources; rendered audio keeps the rate it was rendered at.
- Give `TimeStretcher`'s first output after every reset a ~5 ms fade-in and hold the last ~5 ms of input out of the WSOLA flush; cold-start or hard speech-to-silence steps click at fragment seams.
- Anchor PCM skip targets to `_stallPosition` in `pcm-stream.ts`: `_mediaNow()` collapses to 0 whenever no scheduled source covers the playhead (synthesis stalls, final drain, the START_DELAY window after a rebuild), so `mediaNow - n` alone makes ±5s jump to the timeline start.
- Re-sync `playbackStore` from its own `config-changed` listener; pages that only `set_config` never update it.
- Gate streaming playback off when the active profile has an effect: warm daemons stream (checked by `CliTtsBackend::runtime_ready`), and the PCM scheduler has no effect chain — only the `audio-fragment-ready` path applies effects.
- Emit the streaming `is_final` marker on the last fragment only; an intermediate one arms the player's completion timer mid-passage.
- Signal a failed streamed reading with `playback-stream-failed` and its `session_id`, never `playback-stop` (which cuts every reading): a dead stream never sends the final marker that `playback-finished` waits for.
- Give each reading its own `AbortScope::begin()` and check `scope.aborted()`; never clear the global abort state at a reading's start, since that un-aborts a stream still draining from the previous reading.
- Allow `createWaveShaper` only as a directly-called member; reject local declarations and every other symbol containing "shape".
- Drive HUD captions from the audible fragment's audio clock; synthesis events may describe a later fragment, and incoming PCM must not resume a user-paused stream.
- `hud:*` events fire only while the HUD is enabled; main-window UI reads `playbackStore.caption` and `reading-started`, never HUD events.
- Duck other apps per-session and skip our whole process tree, not our PID: playback is in the webview, so the audio session belongs to a `msedgewebview2` child; persist originals to `duck-state.json` because per-app mixer volumes survive a crash.
- "Beeps" in generated audio: suspect unpronounceable glyphs in the text before the playback code; keep `remove_unpronounceable`'s symbol ranges ahead of what vendors confound.
- Clamp the final mix of an offline effect graph with an identity WaveShaper over `[-1, 1]`: a clamp on one layer does not cap layers summed after it (static bed pushed clamped cyberpunk intro/outro to 1.012), and a gain-up without a final clamp exceeds full scale.
- Size a layer's peak after the crush gain, not before: `crushTail` multiplies by gain before its clamp, so a layer whose peak times gain exceeds 1 hard-clips into a harsh click (the cyberpunk start beep at 0.35 × 3.6).
- Gate only noise layers in an offline effect tail: a whole-tail on/off chop silences designed clicks that fall in off-windows (an older cyberpunk intro had clicks at 0.06, 0.12, 0.86 s that a 30 Hz gate chopped) and chops tones into clicks at every edge.
- Fade speech before `effect.process` in `buildPlaybackUrl`, never after: a post-effect fade ramps the effect's own t=0 onset (the cyberpunk beep lost 7 dB to a 10 ms fade).
- Keep an offline effect's start beep at 40 ms or longer: a 6 ms square burst at 0.5 peak was inaudible in playback while a 40 ms one at the same level was heard.
- Tune offline effect onset and glitch levers on real TTS output (`engines/*/output/test.wav` under `%LOCALAPPDATA%\CopySpeak`), not synthetic bursts: the old silence gate found about 0 onsets per clip on speech, and synthetic clean gaps hid it.
- Glitch mid-speech onsets in place, never by inserting copies of the grain: a 40–70 ms grain replayed before its word sounds like slap-back echo, and each inserted copy pushes captions later.
- Measure a provider WAV's duration from its parsed header clamped to the real byte count, never by iterating the container: Fish writes 0xFFFFFF00 into the data-chunk size and rodio's decode spins past EOF forever (busy tray, no audio, no error).

## Captions

- Captions dying on every surface at once means the active engine's provider stopped emitting timing metadata: check the logs for "Disabling ... captions", verify other engines still write `.captions.json`, and run the `#[ignore]`d `live_caption_probe` before releases.
- Preserve native caption intervals with their generated audio through stream framing, cache/history replay, and sample-count-based fragment concatenation; never scale word timings by text weights.
- Emit daemon caption frames as cumulative request snapshots (text and ms covering every sentence so far): live, batch, and history consumers each keep only the newest frame, so sentence-local frames validate but kill highlights after the first chunk.
- Reject unusable caption metadata without rejecting valid PCM; propagate caption clears to live/cache/history consumers, keep audio/protocol errors fatal, and retain source text for untimed fragments when joining readings.
- Round estimated caption milliseconds once, after accumulating sentence offsets; per-sentence rounding re-rounds sentence-boundary pairs apart by 0.1 ms, and a `start_ms < audio_end` verdict clears the whole reading's captions.
- ElevenLabs with-timestamps streams carry alignment on the leading record(s) and stream later records as audio alone; hard-degrade only when no aligned record ever arrives.
- Accept an ElevenLabs record with audio and an empty `characters` array as audio-only: `eleven_v4_turbo` leads with them, and rejecting one as blank alignment clears captions for the whole fragment, so the live highlight never starts.

## Engines

- A "called from X" comment is not the call site — `local_daemon.rs` claimed prewarm ran "from the set_config path" while `set_config` never called it; grep the named call site before trusting the comment.
- Local engine wrappers speak daemon protocol v2 (`READY 2`) and emit 16-bit signed LE PCM; `pcm-stream.ts` drops any other `bits_per_sample`.
- Gate every streaming branch on `supports_streaming()` AND `runtime_ready()`; the static claim alone sends a cold CLI daemon's one-shot command down the stream branch.
- Adding a `TtsEngine` variant: update the catalog test's engine list and entry count, the default-config profile count in `config/tests.rs` (`test_default_tts_config_uses_kokoro_profile`), plus the `Record<TtsEngine, number>` fixtures in `html-templates.test.ts` and `html-export.test.ts`.
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
- Change a model default in all four places together: the catalog `default_value` in `tts/catalog.rs`, the `*Config::default()` impl and the bundled `default_*_profile()` in `config/tts.rs`, and `play-page.svelte`'s mock; a catalog-only edit leaves fresh installs on the old model.
- Read an HTTP response's body inside the SAME `block_on_async` call that sends the request: with no ambient tokio runtime each call builds a fresh runtime, and a `reqwest::Response` carried across calls loses its connection driver — `.bytes()` fails with `error decoding response body`. openai/google/microsoft carry the split (masked by the app's ambient runtime); fish.rs shows the correct single-block shape.
- Emote-tag support is per (engine, model) pair in `tts/catalog.rs::supports_emote_tags`, not per engine — `eleven_v3_conversational` and Fish `drama-3` stay excluded until someone listens to a tagged read on them.
- Clamp provider WAV headers with `audio/wav.rs::clamp_chunk_sizes_to_file` before ANY in-app decode, not just duration math: xAI ships `data` = `i32::MAX` and a sign-bit-set RIFF size, Fish ships `0xFFFFFF00`; browsers clamp and play fine, but rodio's symphonia decoder trusts the field and every fragment concat of such a provider dies with "end of stream" (single-fragment reads never decode, which hides the bug). xAI clamps at the source; concat_speech clamps every fragment.

## Browser companion

- Gate every browser-protocol addition on a capability: the native host's `ClientMessage` is `deny_unknown_fields` and drops the relay on any unknown field or action, and older extensions disconnect on any app event type they cannot parse.
- Bind the browser follow to each clip's own `session_id` when it activates, never to a `browser-reading` event: a queued speak starts while an earlier clip still plays, and that clip's drain would complete the queued session. Gate trigger word events on `playback-fragment-audible` too, so a queued speak projects nothing until its own tile is audible.
- Match a page selection to copied text ignoring whitespace (`matchCopiedText`): the Windows clipboard has CRLF and Chrome's blank lines between paragraphs, while `captureSelection` keeps source whitespace and joins blocks with one `\n`.
- Map browser caption words onto the raw selection with `text_map::align`'s word alignment; never re-derive the sanitizer's rewrites in a second place, and keep the highlight verdict per word and per fragment.
- Release the old browser reading before asking the document to capture its replacement; a later disconnect erases the new paragraph/selection anchor.
- Start the browser session's state ticker at accept time, not after `speak_queued` returns; a post-synthesis ticker pins the companion panel at `buffering` for the whole generation while audio already plays.
- The browser pipe serves one client at a time; the native host retries `ERROR_PIPE_BUSY` with `WaitNamedPipeW` instead of failing the reading.
- Send a state frame's `word` only while `playing` or `paused` with `word_available`: the extension disconnects its native port (badge `!`) on any frame its parser rejects, and a trigger clip is announced audible while still `buffering`, at 0 ms over its first word.
- Treat the double-copy announce as arriving LATE: a clipboard copy fires no extension event, so ping the worker from the page's `copy` event and let `begin_trigger` wait bounded for the port + `hello` — gating on a live companion at copy time silently dropped every follow started from a sleeping worker, leaving only the stale `!` badge.
- Declare `file:///*` host access and include it in content-script registration and trigger tab queries only when the browser's file-access switch permits it; toolbar `activeTab` success does not prove automatic double-copy can reach local Markdown tabs.
- Rebuild and include tracked `browser-extension/dist/` after companion source edits; the loaded unpacked extension executes these bundles, so source-only fixes and passing source tests leave the browser on old code.

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

## Text pipeline

- Rename request flags in both `src-tauri/src/cli.rs` and `scripts/copyspeak.mjs`; Rust CLI tests do not cover the Node helper used by Hermes, so verify its emitted `/speak` payload separately.
- Fold Cyrillic homoglyphs only inside a word that also holds a Latin letter (`fold_homoglyphs`); a per-character fold turned every Russian word into mixed script ("привет" → "пpивeт").
- Keep U+200C (ZWNJ) out of both invisible-character regexes in `tts_normalize.rs`; Persian and Indic words are spelled with it.
- Add a script's sentence terminator to both `is_sentence_end` in `pagination.rs` and the phrase regex in `captions.ts`; text with no terminator force-splits at exact character counts, mid-word.
- The post-process output re-sanitize deletes single-word bracket tags (`remove_citations` matches `[sighs]` while `[clears throat]` survives by accident); route LLM output through `sanitize_text_tag_safe`, never the plain pipeline.

## Linux

- On Linux launch with `GDK_BACKEND=x11` and show() the HUD window before `set_position`; Wayland toplevels cannot self-position.
- Hide the HUD's native window when the frontend emits global `hud:stop` on Linux; webview playback never trips the Rust AudioPlayer monitor.

## Landing page

- Grep `src/` for the filename before removing or renaming anything in `static/`; the 0.2.6 screenshot swap left `screenshots.svelte` requesting the deleted `screen-v0.1.8.png` and the live landing page shipped a broken image.
- Keep version numbers out of locale strings; they rot on every release (all nine locales carried `v0.1.8` inside the screenshot caption).
- The landing flag `VITE_IS_VERCEL` is never read from the environment: `vite.config.js` `define` maps it from `process.env.VERCEL` (what Vercel's runners set). Local previews must run `bun run dev:landing` / `preview:landing` (they set `VERCEL=1`); exporting `VITE_IS_VERCEL=1` yourself silently serves the Play UI.

## Icons

- An asset-only icon swap does not re-embed the exe icon: tauri-build's generated `.rc` references `icons/icon.ico` by path, so cargo relinks without recompiling the resource. Touch `src-tauri/build.rs` (or `cargo clean -p copyspeak`) to force the resource rebuild, then verify by pixel-comparing `ExtractAssociatedIcon` output against the new ico's 32px frame. The tray's `include_bytes!` icons re-embed on the same rebuild; the installed `%LOCALAPPDATA%` app only changes on the next release install.
- Strip trailing whitespace when installing a designer-exported SVG verbatim: `git diff --check` fails on QuiverAI/Arrow-exported indentation lines, and whitespace-only stripping is render-safe (verify with a byte-compare of a re-render).

## Release

- `bun run bump` derives the new version only from `src/lib/version.ts`; verify all six ✅ lines after bumping. It never touches `browser-extension/` — bump the extension's `manifest.json` + `package.json` by hand on its own cadence.
- Pass `draft: true` to `softprops/action-gh-release` when attaching extra assets to tauri-action's draft release, or the step publishes the release unreviewed.
- Keep `src-tauri/Cargo.lock` tracked and update it deliberately: without it CI resolves fresh, and the 2026-10-16 mass-yank of `zip` <9 left every `tauri-plugin-updater` 2.x unresolvable mid-release (Cargo still honours a yanked version a lockfile pins).

## i18n

- Grep components for `>Text<` and `label="Text"` after an i18n pass; literal UI text bypasses `$_` and passes locale parity.
- Pass caught errors to `$_` values as `String(e)`; a catch binding is `unknown` and svelte-check rejects it.
- Run `vp fmt src/lib/i18n/types.ts` after `node scripts/generate-i18n-types.js`; raw generator output reflows `NestedKeyOf` and adds trailing whitespace, so the diff picks up noise.

## Frontend checks

- A committed bun.lock applies on fresh install even when a stale local node_modules hides the break: svelte-check 4.7.7 rejects `$props<T>()` type arguments, so type props with an `interface Props` and `let x: Props = $props()` instead.
- `vp check`'s anti-slop `no-runtime-typeof` rejects a `typeof` branch on a value union: coerce at the call (`String(value ?? "")`) or parse the value at its I/O boundary.

## Settings

- Force the last visible settings section active at the bottom of the scroll container: a short last section never reaches the scroll-spy reading band, and the layout `<main>` scrolls, not the window, so capture scroll events on the document.

## Experimental gating

- Experimental off FORCES features off at synthesis time (post-process, effects); it does not only hide the UI. Saved settings stay for re-enabling.
- Read the active effect through `effectiveEffect` (`src/lib/effects-gate.ts`), never `profile.effects` directly: the config-changed re-sync read the profile and bypassed the Experimental gate.
- Gate every history IPC that deletes stored audio (`run_history_cleanup` included): the synthesis saves and the automatic cleanup service do not cover the explicit command path.
