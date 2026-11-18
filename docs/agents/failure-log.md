# Failure log by area

Area gotchas from shipped work, one imperative line each. `AGENTS.md` keeps the lines every session needs and points here by area; read an area's section before editing it.

## Playback

- Double-copy and browser readings run `speak_queued`; Play page, hotkey and control server run `speak_now` — change both paths together.
- Wire playback controls through `playbackStore` (webview audio); Rust `toggle_pause`/`skip_forward`/`skip_backward` commands drive only the Rodio safety-net player, which is never what you hear.
- Treat `speak_now`'s return as playback-START, not completion: only the webview store's `finishPlayback`/`handleStop` → `playback-finished` → `playback_signal` chain means completion, and every terminal path must emit it or blocking `/speak --wait` callers hang until the 600 s cap.
- Route playback speed and pitch through `TimeStretcher` (SoundTouch `pitch` setter, then `stretch.tempo = speed / pitch`); keep `playbackRate` at 1 on PCM sources.
- On a rate change, re-stretch only the native chunks of unstarted PCM sources; rendered audio keeps the rate it was rendered at.
- Give `TimeStretcher`'s first output after every reset a ~5 ms fade-in and hold the last ~5 ms of input out of the WSOLA flush; cold-start or hard speech-to-silence steps click at fragment seams.
- Anchor PCM skip targets to `_stallPosition` in `pcm-stream.ts`: `_mediaNow()` collapses to 0 whenever no scheduled source covers the playhead (synthesis stalls, final drain, the START_DELAY window after a rebuild), so `mediaNow - n` alone makes ±5s jump to the timeline start.
- Re-sync `playbackStore` from its own `config-changed` listener; pages that only `set_config` never update it.
- Gate streaming playback off when the active profile has an effect: warm daemons stream (checked by `CliTtsBackend::runtime_ready`), and the PCM scheduler has no effect chain — only the `audio-fragment-ready` path applies effects.
- Emit the streaming `is_final` marker on the last fragment only; an intermediate one arms the player's completion timer mid-passage.
- Signal a failed streamed reading with `playback-stream-failed` and its `session_id`, never `playback-stop` (which cuts every reading): a dead stream never sends the final marker that `playback-finished` waits for.
- A fragment queue that waits across synthesis gaps needs a terminal release on every early exit of its reading (`reading-failed`, `playback-stream-failed`, `browser-reading {active:false}`); otherwise `playback-finished` never fires and blocking `/speak --wait` hangs to the cap.
- Reset a completed reading's retained replay fragments when the next reading starts; a reading that never queues fragments (streamed) otherwise inherits them, and Replay plays the wrong reading.
- Give each reading its own `AbortScope::begin_reading()` or `AbortScope::begin_reading_for_request(request)` and check `scope.aborted()`. `begin()` and `begin_for_request()` are helper scopes for work that is not a reading, and they carry reading id 0. Never clear the global abort state at a reading's start, since that un-aborts a stream still draining from the previous reading.
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
- Claim speaker ownership when a reading becomes audible, not while it synthesizes: the webview sends `playback-started` with its `reading_id`, and Rust claims `PlaybackOwner` for that reading; `playback-finished` releases it. A scoped stop then cuts only the reading that owns the speakers. A reading that has merely started synthesizing owns nothing, so a guard-time claim makes cancelling it cut the previous reading. Heal the store's own `_streamStopped` gate from `_stoppedReadingId`; do not reintroduce `StreamRearm` or a per-chunk streamed claim.
- Never let two readings play at once: streamed audio runs through `PcmStreamScheduler` and batch fragments through the `<audio>` element, and starting one must stop or defer the other. A fresh streamed reading started over a still-playing cached batch reading (double audio on 2026-11-11, speed 1.0). Both start paths check the other engine first, and `_drainHeldAudio` hands the speakers over when the active reading's `finishPlayback` fires. `playback-started` covers a playback RUN, not a reading: `_startedEmitted` clears only on `playback-finished` (`finishPlayback`, `handleStop`, `handleStreamFailed`), so a second queued reading whose fragments join the active queue before it drains chains into the same run and emits no second `playback-started` (two queued double-copies 0.65 s apart at speed 1.5, 2026-11-11) — with audio overlap or without; ducking stays armed because the run never ends in between.
- Attribute every playback event to one reading: allocate the id in `AbortScope::begin_reading*` (0 for helper scopes), carry it on `audio-stream-chunk`, `pagination:complete` and `playback-stream-failed`, and let only the reading that owns a scheduler feed it. The webview holds another reading's chunks per reading id in `_heldStreamRuns` and `_drainHeldAudio` replays them in order after the owner drains; without it a second streamed reading joins the first timeline (one `playback-started`, captions switched mid-track 7360→7520 ms, mixed audio on multi-fragment streams), reproduced 2026-11-11 with two streamed double-copies 0.7 s apart at speed 1.0.
- Only the reading that owns a scheduler can feed it; match `pagination:complete`, `playback-stream-failed`, and `reading-failed` by `reading_id` first, then fall back to `session_id` for id-less events. A scoped stop of a waiting reading emits `playback-reading-dropped`, so the store drops only that reading's held run.
- Bound the held stream audio at 4 runs and 32 MiB of base64: drop the oldest waiting reading, refuse a reading that cannot fit, and ignore every late chunk of a dropped, refused or failed reading.
- Stop a failed audible reading alone: never call `handleStop` from the owner-failure branch, keep the waiting runs and the batch queue, then drain.
- Drain held stream runs by lowest reading id, streamed before queued batch fragments: streamed PCM cannot be regenerated, while a batch fragment can replay.
- Signal each batch reading at its final audible fragment, even when another reading continues the same playback run.
- Keep completion signals per reading until its wait consumes them; a later finish must not overwrite an earlier one.

## Captions

- Captions dying on every surface at once means the active engine's provider stopped emitting timing metadata: check the logs for "Disabling ... captions", verify other engines still write `.captions.json`, and run the `#[ignore]`d `live_caption_probe` before releases.
- Preserve native caption intervals with their generated audio through stream framing, cache/history replay, and sample-count-based fragment concatenation; never scale word timings by text weights.
- Emit daemon caption frames as cumulative request snapshots (text and ms covering every sentence so far): live, batch, and history consumers each keep only the newest frame, so sentence-local frames validate but kill highlights after the first chunk.
- Reject unusable caption metadata without rejecting valid PCM; propagate caption clears to live/cache/history consumers, keep audio/protocol errors fatal, and retain source text for untimed fragments when joining readings.
- Round estimated caption milliseconds once, after accumulating sentence offsets; per-sentence rounding re-rounds sentence-boundary pairs apart by 0.1 ms, and a `start_ms < audio_end` verdict clears the whole reading's captions.
- ElevenLabs with-timestamps streams carry alignment on the leading record(s) and stream later records as audio alone; hard-degrade only when no aligned record ever arrives.
- Accept an ElevenLabs record with audio and an empty `characters` array as audio-only: `eleven_v4_turbo` leads with them, and rejecting one as blank alignment clears captions for the whole fragment, so the live highlight never starts.
- Accumulate caption timings that start inside a word's span, not only contiguous ones: per-token streams (xAI, estimation) split "don't" around the apostrophe, and requiring contiguous timing text dropped the whole word's timing, so the pill skipped it (2026-11-17).
- External clients (the DSH plugin) read live captions from the control server's `GET /status`; the webview reports each `publishCaption` snapshot through `report_playback_caption` (throttled to ~100 ms) and Rust answers it while younger than 1 s, so no cross-process HUD event is needed and HUD-off machines still serve captions. Keep `/status` the only CORS-open route.
- A control-server speak holds the queued gate like `speak_queued` does: a `speak_now` that skips `queued_gate` splices its first fragment between a queued reading's fragments, because `speak_queued` holds no synthesis lock for it to wait on.
- Attribute every `playback-finished` to its reading (`audio-fragment-ready` carries `reading_id`; the store falls back to it for batch runs): an unattributed finish releases any `/speak --wait` whose audio never played, and the previous reply's late stop-finish poisons the next request's wait.
- Wire interrupt mode (`on_retrigger`) at the audible paths — `do_abort_synthesis` in `handle_speak` and `speak_queued` — not only the Rodio `AudioPlayer`: the webview never reads that setting, so a player-only mode change cuts nothing the user hears.
- Highlight DSH captions inside the assistant message, not in a strip: the Chat row carries `data-chat-node-key` and the settled Markdown body carries `data-text-interaction-source`, but `/status` word offsets index the sanitized spoken fragment, not the rendered Markdown, so align the fragment words to the rendered text nodes before painting with the CSS Custom Highlight API.
- DSH caption timings follow provider granularity: ElevenLabs `/status` entries are per character, so group entries under the whitespace-run word they overlap before aligning them to rendered words; treating each entry as a word anchors on a single letter, finds no match, and paints nothing.
- Never name a DSH plugin variable after a browser global (`CSS`, `Highlight`, `window`): a local `const CSS` stylesheet shadowed the browser `CSS` in the client half, which made `CSS.highlights` undefined and silently skipped every highlight; use `window.CSS.highlights` and `window.Highlight` explicitly.
- When resolving the spoken reply from a Chat row, skip `[data-text-interaction-source]` bodies inside `[data-variant="think"]`: expanded reasoning shares the row key and would otherwise put reasoning words before reply words in the alignment list.
- Parse `oklch()` as well as `rgb()` when blending caption colors from theme tokens: Chromium serializes computed oklch tokens space-preserving as `oklch(...)` (probed on WebView2/Edge 157 — the runtime shares Edge's exact version), so an `rgba?()`-only probe regex returns null and every glide silently degrades to the instant swap. Verify serialization claims on the app's own WebView2 version, not on assumed behavior.
- Treat `activeIndex -1` caption ticks as hold, not clear: the 25 ms clock lands between word timings on every inter-word gap, and tearing the glide state down there makes every following word settle instantly (looked like "parse bug fixed but still no glide"). Hold the last paint plus its knockout color, and clear only on a reading boundary via a reset key. The companion has the same failure shape through its own wordless frames — see the Browser companion section.

## Engines

- Point `scripts/bench/bench.py` at `%LOCALAPPDATA%\CopySpeak\engines`, the root the installers and app use; the bench had `CopySpeech\engines`, which never holds engines.
- Catch only dizione's `ValueError` with "nothing to pronounce" as a skip; dizione raises it instead of returning empty ids, and other ValueErrors are real failures.
- Split engine text on CJK sentence marks (。！？) too; ASCII-only splitting merges a Japanese sentence into the next English one and gives it caption timings.
- Keep Cartesia's stream request on `pcm_s16le`: the player decodes only signed 16-bit PCM, so the Encoding select applies to saved (batch) audio only.
- Read Google's `output_format` in `google.rs` before trusting its select; the backend never reads it.
- Stop or idle a running `tauri dev` before `cargo check`: its cargo process holds the build-directory lock, so a second `cargo check` waits silently.
- Keep the engine-test lock separate from displayed results; invalidate results after credential replacement or installer changes.
- Stop cloud engine tests when the environment-key warning defers saving; a resolved save callback does not prove persistence.

- The app loads secrets from `<exe-dir>/.env` (`src-tauri/target/debug/.env` in dev), not the repo-root `.env`.
- A "called from X" comment is not the call site — `local_daemon.rs` claimed prewarm ran "from the set_config path" while `set_config` never called it; grep the named call site before trusting the comment.
- Local engine wrappers speak daemon protocol v2 (`READY 2`) and emit 16-bit signed LE PCM; `pcm-stream.ts` drops any other `bits_per_sample`.
- Gate every streaming branch on `supports_streaming()` AND `runtime_ready()`; the static claim alone sends a cold CLI daemon's one-shot command down the stream branch.
- Adding a `TtsEngine` variant: update the catalog test's engine list, entry count, and the caption-support list in `tts/catalog.rs` tests (`caption_support_matches_timing_adapters`, in catalog order), and the default-config profile count in `config/tests.rs` (`test_default_tts_config_uses_the_os_default_profile`, which follows the OS default).
- Seed a bundled profile for any engine that becomes the OS default: `complete_onboarding` finds an engine's profile by engine id and fails with "No profile configured" when none exists (tiny-voca default, 2026-11-17).
- Pin each `TtsEngine` serde id to the frontend union and presets with `#[serde(rename)]`: plain lowercase turned `TinyVoca` into `tinyvoca`, which rejected the `tiny-voca` preset (found 2026-11-17, fixed before 0.3.1 shipped).
- Add every new first-class local engine to `DAEMON_ENGINES` in `tts/local_daemon.rs`: `engine_kind()` matches the wrapper filename against that list, and prewarm, daemon streaming, `runtime_ready`, and `supports_streaming` all hang off it. Without the entry the app runs the engine one-shot forever (hit with tiny-voca, 2026-11-17); the catalog parity test `daemon_engines_match_the_first_class_local_catalog_entries` fails until both sides match.
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
- Make any Tauri command that calls a provider `async` and run the blocking backend call in `tauri::async_runtime::spawn_blocking`, with the config lock released first: a sync `#[tauri::command]` runs on the main thread, so `list_tts_voices` froze the window for an untimed fetch while holding the config mutex. Bound provider voice-list requests with `tts::http::voice_list_client()`.
- Emote-tag support is per (engine, model) pair in `tts/catalog.rs::supports_emote_tags`, not per engine — `eleven_v3_conversational` and Fish `drama-3` stay excluded until someone listens to a tagged read on them.
- Clamp provider WAV headers with `audio/wav.rs::clamp_chunk_sizes_to_file` before ANY in-app decode, not just duration math: xAI ships `data` = `i32::MAX` and a sign-bit-set RIFF size, Fish ships `0xFFFFFF00`; browsers clamp and play fine, but rodio's symphonia decoder trusts the field and every fragment concat of such a provider dies with "end of stream" (single-fragment reads never decode, which hides the bug). xAI clamps at the source; concat_speech clamps every fragment.
- Bump `VOICE_CACHE_SCHEMA` in `commands/tts/voices.rs` whenever `VoiceCatalogEntry` gains a field: a serde default alone keeps old cached lists showing no tags until Refresh, while a version mismatch makes the next Profiles visit fetch again.
- Provide generated voice previews when catalog entries lack preview URLs; static catalog voices do not include audio samples.
- Isolate one-shot preview files and process tracking from normal readings; concurrent synthesis must not overwrite or cancel another request.
- Return `Option<Guard>` explicitly when the guard implements `Drop`; `Option<Guard>` does not implement `Drop`.
- Serialize local model loads, streams, previews, and one-shot tests; retire idle daemons before starting another process.

## Browser companion

- Gate every browser-protocol addition on a capability: the native host's `ClientMessage` is `deny_unknown_fields` and drops the relay on any unknown field or action, and older extensions disconnect on any app event type they cannot parse.
- Bind the browser follow to each clip's own `session_id` when it activates, never to a `browser-reading` event: a queued speak starts while an earlier clip still plays, and that clip's drain would complete the queued session. Gate trigger word events on `playback-fragment-audible` too, so a queued speak projects nothing until its own tile is audible.
- Match a page selection to copied text ignoring whitespace (`matchCopiedText`): the Windows clipboard has CRLF and Chrome's blank lines between paragraphs, while `captureSelection` keeps source whitespace and joins blocks with one `\n`.
- Map browser caption words onto the raw selection with `text_map::align`'s word alignment; never re-derive the sanitizer's rewrites in a second place, and keep the highlight verdict per word and per fragment.
- Release the old browser reading before asking the document to capture its replacement; a later disconnect erases the new paragraph/selection anchor.
- Start the browser session's state ticker at accept time, not after `speak_queued` returns; a post-synthesis ticker pins the companion panel at `buffering` for the whole generation while audio already plays.
- The browser pipe serves one client at a time; the native host retries `ERROR_PIPE_BUSY` with `WaitNamedPipeW` instead of failing the reading.
- Register the Chrome native host with PLAIN paths (no `\\?\` extended-length prefix) in both the manifest `path` and the registry value: Tauri's `resource_dir()` returns canonicalized `\\?\` paths, and Chrome fails the handshake with `Error when communicating with the native messaging host` without the host ever reaching the pipe (0.3.0). `browser_registration.rs::plain_path` strips the prefix before writing; when a connect fails while every name check passes, `reg query` the key and read the manifest for the prefix.
- Treat a repo registration fix as NOT INSTALLED until you rebuild and reinstall: an installed exe that predates the fix re-writes the broken values on every launch and clobbers each hand repair, so the breakage recurs with no code change (d71983a landed 8 hours after the 0.3.1 install; the stale exe re-prefixed the registration on every startup). Compare the installed `copyspeech.exe` mtime with the fix commit date, install a build that contains the fix, and only then declare the companion fixed.
- Send a state frame's `word` only while `playing` or `paused` with `word_available`: the extension disconnects its native port (badge `!`) on any frame its parser rejects, and a trigger clip is announced audible while still `buffering`, at 0 ms over its first word.
- Treat the double-copy announce as arriving LATE: a clipboard copy fires no extension event, so ping the worker from the page's `copy` event and let `begin_trigger` wait bounded for the port + `hello` — gating on a live companion at copy time silently dropped every follow started from a sleeping worker, leaving only the stale `!` badge.
- Declare `file:///*` host access and include it in content-script registration and trigger tab queries only when the browser's file-access switch permits it; toolbar `activeTab` success does not prove automatic double-copy can reach local Markdown tabs.
- Rebuild and include tracked `browser-extension/dist/` after companion source edits; the loaded unpacked extension executes these bundles, so source-only fixes and passing source tests leave the browser on old code.
- A companion follow does not survive a page refresh: the worker stops a page-started reading on `tabs.onUpdated`, `hello` only stores the automatic flag (no re-announce), and trigger claims bind to the pre-refresh documentId — the pill returns only after the user starts a new read, so reproduce mid-reading glide bugs inside that new reading.
- Set the Firefox build's `strict_min_version` to 153.0 or later: `sendMessage` `documentId`, `executeScript` `documentIds`, and `sender.documentId` ship only in Firefox 153. Derive `dist-firefox/manifest.json` in `build.mjs` (`bun run build:firefox`), never by hand.
- Firefox native host manifests use `allowed_extensions` with the gecko ID, and live at `HKCU\Software\Mozilla\NativeMessagingHosts\com.copyspeech.browser` on Windows and `~/.mozilla/native-messaging-hosts/` on Linux, not the Chrome `NativeMessagingHosts` directories.
- The companion's icons come from `src-tauri/icons` at build time. A stale `dist/icons` keeps the old colour until the build reruns.
- Smoke-test Firefox with `npx web-ext@latest run ... --profile-create-if-missing --arg=-headless --no-input`; web-ext has no `--headless` flag, and `--firefox-profile` needs a `user.js` with `devtools.debugger.remote-enabled=true` or the devtools port never binds (web-ext then loops on ECONNREFUSED and exits).
- Run the extension's Playwright specs from the repo root: the specs resolve `browser-extension/dist` against the process CWD, so running them inside `browser-extension/` doubles the path and every fixture asset 404s.
- Give the content spec's chrome stub every runtime API the bundle touches at load: `hover.ts` reads `chrome.runtime.getURL` at module top, and a stub missing it kills the script before `onMessage.addListener`, which reads as `window.receive is not a function` and fails the whole spec.
- Anchor a caption tween's clock at its first rendered frame and cap each frame's step (`MARK_FRAME_CAP_MS` in `glide.ts`): state frames run inside busy-page tasks, and a paint-time wall clock snapped the pill across late frames (14 rendered steps on an idle page, 1-2 on a real one).
- Hold the painted word on every wordless companion state frame: `find_word_at_position` returns None on inter-word caption gaps and trailing fragment audio while the status stays `playing`, so the ticker emits `word: null` frames mid-reading, and clearing there makes the NEXT word settle instantly — the old abrupt word-swap is back for the rest of the reading. Keep the paint until a reading boundary tears down through `release`.
- Parse `oklch(...)` in the companion's `resolveColor` as well as `rgb()`: pages style text with oklch (Tailwind v4 default palette), and Chrome and Edge alike serialize computed oklch space-preserving as `oklch(...)` (probed headless on this box), so an `rgba?()`-only regex settles every tween instantly on those pages. Port the app's `parseColor` (`rgb`/`color(srgb ...)`/`oklch`) unchanged.
- Run the root `bun run check` after browser-extension edits too: the glide commit shipped four unformatted `browser-extension/` files, and the root formatter gate stays red until a `vp fmt` pass on them.
- Append the companion's caption pill to `document.documentElement` and never isolate the anchor root for it: `isolation` on a non-ancestor cannot change the mark's backdrop, and the whole page IS the backdrop that makes `mix-blend-mode: difference` invert the text behind the word.
- Fill the caption pill white under `mix-blend-mode: difference` and paint it above the word (`z-index` in the extension, positioned-over-inline in the app): a border-only mark never inverts the word, so the bubble reads as a plain outline. An isolated but transparent host breaks the fill into a solid white block, because the blend then sees no opaque backdrop — the Play page host is safe (it carries `bg-card`), the HUD caption must stay un-isolated.
- Follow the spoken word only when its line leaves a 15–82 % band of its scroll port, with a pause after a user wheel/touch/scroll-key gesture: scrolling on every word fights the reader, and one full-viewport jump loses their place. Call `glide.reposition()` from the scroll port's `scroll` event, because an inner box scroll moves the word without moving the document.
- Run `browser-extension/tests/glide.test.ts` and the content Playwright spec after changing the pill: they pin the mark's geometry, blend mode and settle/glide split, and the KISS rewrite left both asserting removed `copyspeech-word*` highlights (12 unit failures) while the app's vitest suite stayed green.

## History & cache

- Trace history export through its UI, backend command, templates, tests, and documentation before removing it; a batch export example is not the whole feature.
- Branch every synthesis save site on `history.enabled`: on logs and saves to the history folder, off calls `save_speech_cache` and logs nothing; `find_cached_speech` must read the same store.
- Replay an early stop's paid fragments from the content-addressed cache with history off: `find_cached_reading` replays a whole reading only when every fragment is saved, so keep the per-fragment `find_cached_fragment` lookup in the pagination loop; a streamed fragment never replays, since it has no complete WAV.
- A cached whole-reading replay is one `audio-ready` on the batch webview path even with streaming on, so it pays per audio second before first audio: about 4.2 s for a 72 s reading at pitch 0.9, about 1.1 s at pitch 1.0. Measure it from the `[Playback] webview playback started` line minus the cached-replay emit line.
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

- Capture the on-disk TTS schema version before `migrate_tts_config`: it resets the version to 2 on every load, so a later schema-gated migration otherwise overwrites per-profile preferences at every restart.
- Rename request flags in `scripts/copyspeech.mjs`; the Node helper is the live CLI used by Hermes, so verify its emitted `/speak` payload separately. `src-tauri/src/cli.rs` was never compiled (no `mod cli;`) and was deleted.
- Fold Cyrillic homoglyphs only inside a word that also holds a Latin letter (`fold_homoglyphs`); a per-character fold turned every Russian word into mixed script ("привет" → "пpивeт").
- Keep U+200C (ZWNJ) out of both invisible-character regexes in `tts_normalize.rs`; Persian and Indic words are spelled with it.
- Add a script's sentence terminator to both `is_sentence_end` in `pagination.rs` and the phrase regex in `captions.ts`; text with no terminator falls to whitespace-snapped force-splits — no mid-word cuts since #88, but sentence-level breaks degrade to clause/word boundaries.
- The post-process output re-sanitize deletes single-word bracket tags (`remove_citations` matches `[sighs]` while `[clears throat]` survives by accident); route LLM output through `sanitize_text_tag_safe`, never the plain pipeline.
- JIT-gate every paginated path: `synthesize_paginated` (`speak_now`) and `speak_queued_internal` (double-copy, browser companion) submit fragment 0 at once, then each later fragment inside the previous fragment's lead window; a fully cached queued reading skips the gate because it costs no provider request.
- Serialize queued readings with a flush + barrier (`commands/tts/queued_gate.rs`): the webview's `FragmentQueue` plays in ENQUEUE order and carries no reading id, so a second reading's fragment 0 would interleave with the active reading's gated fragments; a waiting reading asks the holder to flush its rest at once, then takes the turn, which also frees a paused reading's turn.
- Check the abort scope on every JIT gate poll and wake its 120 ms sleep on a scoped stop (`until_cancelled`): the previous `/speak` releases its wait lock only after this future returns, so a slow poll delays the next click, and a stopped reading must never submit the next fragment.
- Flipping a `MarkdownSanitizationConfig` default fails every test that builds the config via `..Default::default()` and asserts the old behavior; grep those tests in the same change and pin the flipped field explicitly where the test means to exercise the toggle.

## Linux

- On Linux launch with `GDK_BACKEND=x11` and show() the HUD window before `set_position`; Wayland toplevels cannot self-position.
- Hide the HUD's native window when the frontend emits global `hud:stop` on Linux; webview playback never trips the Rust AudioPlayer monitor.

## Landing page

- Grep `src/` for the filename before removing or renaming anything in `static/`; the 0.2.6 screenshot swap left `screenshots.svelte` requesting the deleted `screen-v0.1.8.png` and the live landing page shipped a broken image.
- Keep version numbers out of locale strings; they rot on every release (all nine locales carried `v0.1.8` inside the screenshot caption).
- The landing flag `VITE_IS_WEB` is never read from the environment: `vite.config.js` `define` maps it from `process.env.COPYSPEECH_WEB`. Local previews must run `bun run dev:landing` / `preview:landing` (they set `COPYSPEECH_WEB=1`); exporting `VITE_IS_WEB=1` yourself silently serves the Play UI.
- Format imported HTML/CSS/JS under `static/` before running `bun run check`; the global formatter checks those files too.
- Scope landing CSS under `.landing` in `landing.css`: `+layout.svelte` renders the Play UI's root too, so a bare `h2`, `footer` or `:root` rule restyles the app.
- Target `.dark .landing` for dark-only rules: mode-watcher adds `.dark` to `<html>` and never `.light`, so light is the base selector.
- Copy preview assets the landing uses from the untracked `docs_internal/` into `static/landing/`: the gitignored folder never deploys.
- Give the hero `.promise` heading `position: relative` when it uses `captionGlide`: the glide pill positions against its host, so a static host puts the pill at the wrong offset.
- End the landing Listen demo on `audio.ended`, not on the silent loop's `LAST_END + 300` clock: a clip shorter than that tail never reaches it, so the button stays on Stop and the silent loop stays off.
- Never mark the providers marquee clone `inert`: each copy is narrower than the carousel viewport, so the clone is on screen for most of the loop and `inert` makes visible logos swallow clicks (events retarget to `.providers-track` with no href). Keep the clone `aria-hidden` with `tabindex="-1"` on its links — mouse-clickable, still unfocusable and silent to assistive tech.
- Translate a source mark's ink-box origin to (0, 0) before scaling it into a tight lockup viewBox; scaling alone shifts and clips artwork whose source coordinates start elsewhere.
- Center the OpenAI wordmark's cap-height ink band against the Blossom and preserve its p descender in the viewBox; aligning the full wordmark box's top to the Blossom and cropping to the Blossom height clips the descender.
- svgo 4.x rejects the providers' svgo.config.cjs `removeViewBox` override inside `preset-default` ("not part of preset-default") and its output can be slightly LARGER than the input — the viewBox still survives because svgo 4 dropped the plugin from the preset. Never "fix" the config per the warning's suggestion (a standalone `'removeViewBox'` plugin): that ENABLES the plugin and strips the viewBox the mask sizing needs.

## Icons

- Do not fill Lucide icons with `fill="currentColor"`: their stroke-path geometry smears when filled (`rotate-ccw` is an open arc). Use the vendored Material filled glyphs in `src/lib/components/icons/` (copied from google/material-design-icons); add new filled glyphs there in the same pattern.
- Drive tray playback status from the webview store, never `AudioPlayer`: Rodio is only a safety net, and refreshing the tray from its callbacks must not re-lock the player. Preserve the branded icon’s color and artwork; macOS template mode strips color.
- Resize Windows tray artwork to the system small-icon dimensions with alpha-aware filtering before passing it to Tauri: Windows' native downscale from 512px produces jagged edges.
- Keep `icons/icon.ico` in the Cargo build script's `rerun-if-changed` inputs: tauri-build's `.rc` references it by path, so without that input a changed asset leaves stale exe resources. Verify the embedded 32px frame against the source after rebuilding; the installed app changes only on the next release install.
- Strip trailing whitespace when installing a designer-exported SVG verbatim: `git diff --check` fails on QuiverAI/Arrow-exported indentation lines, and whitespace-only stripping is render-safe (verify with a byte-compare of a re-render).
- Regenerate every icon derivative in the same pass as the base set: `tray/speaking.png`, `tray/paused.png` (`scripts/render-tray-icons.py`), `app-icon-inverted.png`, `browser-extension/dist/icons/`, and the SECOND logo copy `static/landing/app-logo.svg` (used by the landing header — it is a separate file and sat on the old palette unnoticed) are separate artifacts and have drifted from `icon.png` more than once.

## Release

- `bun run bump` derives the new version only from `src/lib/version.ts`; verify the five ✅ lines and expect the README ⚠️ (no `**Current Version:**` line exists — hand-bump its asset-name version strings). It never touches `browser-extension/` — bump the extension's `manifest.json` + `package.json` by hand on its own cadence.
- Pass `draft: true` to `softprops/action-gh-release` when attaching extra assets to tauri-action's draft release, or the step publishes the release unreviewed.
- Keep `src-tauri/Cargo.lock` tracked and update it deliberately: without it CI resolves fresh, and the 2026-10-16 mass-yank of `zip` <9 left every `tauri-plugin-updater` 2.x unresolvable mid-release (Cargo still honours a yanked version a lockfile pins).
- In a fresh clone or worktree, run `bun run build` once before any `cargo` command in `src-tauri/`; tauri-build fails with `frontendDist "../dist" doesn't exist` until `dist/` exists.
- Every `latest.json` platform entry must keep both `url` and `signature`; a signature-only entry (the 0.3.0 Linux step) breaks the updater manifest. The Linux workflow checks this with `jq -e` before upload. Patch every entry that points at the AppImage, including `linux-x86_64-appimage`, or AppImage installs get a stale signature.
- Read `CHANGELOG.md` with `-Encoding utf8` in every PowerShell release step; Windows PowerShell decodes UTF-8 dashes as cp1252 mojibake without it.
- Build `browser-native-host` in release mode before any Windows `tauri dev` or `tauri build`; `tauri.windows.conf.json` bundles its `.exe`, and tauri-build fails loudly when the file is missing.
- Keep the browser extension's `key` field and `com.copyspeech.browser` host name in sync with `browser_registration.rs`; a mismatch makes Chrome show `!` with no visible error.

## i18n

- Bind the language selector to the saved preference, not the resolved locale: an empty value means System and must remain selectable even when it currently resolves to English.
- Read the saved locale through the scalar `get_saved_locale` IPC before the first render; onboarding can hang on the full `get_config` payload even when a config file exists.
- Keep tray labels in sync with the webview's effective locale, not just the saved value: an unset locale currently makes the Rust tray fall back to English while the webview follows the system language.
- Grep components for `>Text<` and `label="Text"` after an i18n pass; literal UI text bypasses `$_` and passes locale parity.
- Pass caught errors to `$_` values as `String(e)`; a catch binding is `unknown` and svelte-check rejects it.
- Run `vp fmt src/lib/i18n/types.ts` after `node scripts/generate-i18n-types.js`; raw generator output reflows `NestedKeyOf` and adds trailing whitespace, so the diff picks up noise.
- Write locale JSON with LF bytes: Python `Path.write_text` turns LF into CRLF on Windows, and git then warns on every locale file; use `open(path, "w", encoding="utf-8", newline="\n")`.
- List only directories that hold locale JSON in `.vscode/settings.json` `i18n-ally.localesPaths`; i18n-ally warns "No locale files loaded" when any listed directory is empty, even if another one loads.

## Frontend checks

- Import test APIs from `vite-plus/test`, not `vitest`; `vp check`'s `prefer-vite-plus-imports` rejects the bare import.
- Hold a mocked async API on a promise the test opens when proving a race; `vi.waitFor` polls, so an ungated mock finishes the whole setup before the test's next step and the race never happens.
- Type test catalog maps with `satisfies Record<string, Catalog>`, not a `: Record<…>` annotation; `vp check`'s anti-slop `no-known-value-widening` rejects the annotation.
- Let one dynamic `import()` of a `vi.mock`ed module settle before code under test starts another: concurrent ones can resolve the real module, so the mock never sees the race.
- Unlisten a Tauri `listen` that resolves after its component's `onDestroy`: HMR can destroy a component mid-`onMount`, and the orphaned listener double-fires (one double-copy spoke twice).
- A committed bun.lock applies on fresh install even when a stale local node_modules hides the break: svelte-check 4.7.7 rejects `$props<T>()` type arguments, so type props with an `interface Props` and `let x: Props = $props()` instead.
- `vp check`'s anti-slop `no-runtime-typeof` rejects a `typeof` branch on a value union: coerce at the call (`String(value ?? "")`) or parse the value at its I/O boundary.
- Stub `Element.prototype.scrollIntoView`, `HTMLMediaElement.prototype.play` and `pause` in component tests: jsdom does not implement scrolling or media playback.
- After `git checkout` of a watched source file, `touch` it and re-fetch the module from the dev server: the Vite watcher can miss the checkout write and keep serving the pre-revert transform, so the app paints deleted code.
- After moving a test into `__tests__/`, rewrite its `./` and `../` specifiers, `vi.mock` targets and dynamic `import()` paths from the old folder; `import.meta.dirname` paths that point at the source folder need one more `..`.
- Format only the moved file with `vp fmt <file>` after editing a test; a long line that fails `vp check` is not a reason to loosen the check.
- Bump `vite`, `vitest` and the `overrides` pins together with `vite-plus`: `vite-plus` 1.7.0 needs `vite-plus-core@8.3.8000` and `vitest@5.1.0`, and mismatched pins fail `svelte-check` on `vitest.config.ts` with "Excessive stack depth" (types only, tests still run).
- The extension tests resolve `jsdom` from the root `node_modules`: jsdom 31 serializes `translate(-40px,0px)` as `translate(-40px)`. Same transform; expect that string in `browser-extension/tests/glide.test.ts`.

## Rust tests

- Split a Rust test module as `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`; a child file beside a non-`mod.rs` parent avoids a new directory.
- Run `rustfmt` on the split-out child file only; rustfmt on the parent reflows unrelated non-test code (seen in `control_server.rs`).
- Wait for the dev session's `cargo run` to release the build-directory lock; never kill another session's cargo to run `cargo test`.

## Settings

- Keep the Lenis content element stable across route changes; MotionWrapper replaces its inner node on navigation.
- Sync Lenis's cached limit through `syncLenisToLayout` after collapses, not only on resize: the fixed-height content box never resizes when a section grows, so smooth scrolls clamp short. Call `lenis.dimensions.resize()` (not `lenis.resize()`, which cancels the in-flight scroll) before a programmatic `scrollTo`.
- Keep the footer handle's @ at the same tracking as its letters.
- Update `play-page.svelte`'s browser mock config when adding required `AppConfig` fields, matching backend defaults.
- Pass the current locale direction to Bits UI tooltip content: its floating wrapper defaults to `dir="ltr"` even when the document is RTL, scrambling mixed Hebrew or Arabic and Latin text.
- Gate hidden startup and close-to-tray on an actually available tray; create the main window hidden, show first-run setup explicitly, and use one unminimize/show/focus helper for tray and second-instance reopening. Propagate OS autostart registration failures before persisting the new preference.
- Force the last visible settings section active at the bottom of the scroll container: a short last section never reaches the scroll-spy reading band, and the layout `<main>` scrolls, not the window, so capture scroll events on the document.
- Spread ground parallax across the full scroll range while keeping the texture within its overscan; clamping raw scroll distance makes it stop moving early on long Settings pages.
- The ui `collapsible` index exports `Collapsible` as the Root component only, not a namespace: `Collapsible.Trigger`/`.Content` type-error in svelte-check — import `CollapsibleRoot`/`CollapsibleTrigger`/`CollapsibleContent` by name.
- Draw the shadow below an empty settings divider with a positioned pseudo-element; an inset shadow on a zero-height divider is invisible.
- Give light settings cards a tinted surface and a bright divider line above its darker shadow; two dark lines read as one thick rule.

## Experimental gating

- Experimental off FORCES features off at synthesis time (post-process, effects); it does not only hide the UI. Saved settings stay for re-enabling.
- Read the active effect through `effectiveEffect` (`src/lib/effects-gate.ts`), never `profile.effects` directly: the config-changed re-sync read the profile and bypassed the Experimental gate.
- Gate every history IPC that deletes stored audio (`run_history_cleanup` included): the synthesis saves and the automatic cleanup service do not cover the explicit command path.

## DSH plugin (copyspeech-dsh)

- Register one interaction id per gesture, not one per button: `uiConversation.interactions.dispatch()` tries handlers in registration order and returns the first id that calls `request.open()`, so a second id competes with the host Quote interaction instead of appearing beside it. Shadow the host cell for its `quote` key and render both actions there.
- Shadow a keyed slot cell by registering the same key at a DIFFERENT priority and let the lowest live priority render; a same-key registration at the same priority throws. Keep the host interaction registered for admission and replace only its cell.
- Capture a selection Range when the action opens: the text-interaction host passes only `readText()`/`dismiss()`, and a click collapses the native selection unless the pill's `pointerdown` calls `preventDefault()`.
- Clear the native selection when a selection reading starts; the cloned range survives, so the caption highlight still paints the spoken words inside the former selection.
- Commit DSH plugin client.js with an explicit pathspec (`git commit -- client.js`); a plain `git commit` swept another session's staged settings-file renames into the commit.
- Drive the pill glide from ONE rAF loop: a CSS transition beside it drifts mid-glide, because the transform curve and the transition curve differ.
- Gate auto-read on the Session snapshot, never on the first reply id alone: the Chat history loads after the composer dock mounts, so the last loaded reply's id arrives as a change and reads aloud on every DSH restart. The watcher waits for `openState === 'open'` and speaks a reply only after it has seen `running` true. Run `bun scripts/test-dsh-plugin-auto-read.mjs` after touching the watcher; `AUTO_READ_CLIENT=<path to another client.js>` proves the checks fail on the old code.
- A partial auto-read excerpt drops the reply's middle, so one caption fragment can jump from the top block to the bottom block. Align across that gap only on a proven run of the next spoken words (`resyncForward`); the normal 24-token window otherwise leaves the fragment below its 50% match floor, and no highlight paints at all.
- Grep every call of a helper a caption port deletes: a bare `wordRects` or `releaseMarkHost` throws inside the status-poll promise, so the pill silently never paints while every other test stays green. Run `bun scripts/test-dsh-plugin-captions.mjs` after touching the pill; `CAPTION_CLIENT=<path>` proves the checks fail on the old code.
- Repaint the fixed pill on every scroll: a `position: fixed` pill does not travel with the chat scrollport, so it detaches from the word until the next paint. Re-read the active word's rect on the scroller and the window, and once more at glide end, because the follow scroll can move the target mid-glide.

## Hermes plugin (copyspeech-hermes)

- Gate auto-read on a `message.start` seen for the same session_id, because the gate needs the turn's start.
- Drop events with `replayed === true`, because replayed events repeat old replies.
- Require `payload.status === 'complete'` and the focused session_id before speaking, because child sessions finish on their own id.
- Dedupe across windows with a claim keyed `claim.<session_id>.<seq>` in `ctx.storage`, because the SDK gives no window id.
- Skip the plugin read when `localStorage['hermes.desktop.autoSpeakReplies'] === 'true'`, because Hermes's built-in reader speaks the same reply.
- Match `GET /status` captions to the reply text, not to a request id, because `/status` carries no request_id.
- Require half of the caption words to map to the reply, because a lower share means a foreign reading.
- Resync across the excerpt gap only on a run of 3 words, because one common word matches far ahead.
- Hook replies with `[data-slot="aui_assistant-message-root"][data-role="assistant"]` and read text from `.aui-md`, skipping reasoning slots, because the DOM marks replies that way.
- Look up the reply in the focused pane, `[data-session-anchor]` = `session-tile:<focusedStoredSessionId>` else `workspace`, never the last assistant root in the document, because a reply in another pane can be the last root.
- Keep a caption alignment miss unkeyed so the next poll retries, because the reply can render after the first poll.
- Install the plugin through a junction made with PowerShell `New-Item -ItemType Junction`, because the watcher hot-reloads edits made in the repo.
- Check the CopySpeech app log for the missed fragment's `Clamped` line before tuning plugin alignment, because a missing line means the engine never delivered that fragment's captions; the missed reply aligned offline (approximate sanitizer: 170 of 173 words).
- Run `bunx vp fmt` before writing a regex that strips the SDK import: the formatter splits long imports across lines, and a single-line regex then leaves a dangling import in the test copy.
- Write boolean and string guards without `typeof`: `vp check` rejects runtime `typeof` (no-runtime-typeof); use `Array.isArray`, `String(x) === x`, or `??` with a typed default.
- Keep helpers in `plugin.js`, because the loader rejects relative imports; `plugin.test.mjs` strips the SDK import lines into a temp copy.
- Re-check focus and settings when a `/status` response lands, not only before the request: a late reply from the old chat otherwise paints in the new one.
- Use `scrollTo` behavior `instant` for Reduce Motion, not `auto`: `auto` defers to the page's CSS `scroll-behavior`, so a smooth-scroll page still animates the follow move.
- Inject manual playback UI into the chat DOM: the desktop plugin SDK has no per-message chat slot, so the play button rides the host hover action row `.aui-message-actions-desktop` inside `[data-slot="aui_msg-actions"]`. Verify the selector against the RUNNING build's renderer chunks, not the checkout source.
- Start every plugin reading through the control server's request-scoped routes (`/speak` with `request_id`, `/pause`, `/resume`, `/seek`, `/stop`) and mirror the request in one playback record, so the bar can pause, seek, and stop any reading the plugin started, auto-read included.
- Gate the plugin's injected UI on `window.MutationObserver !== undefined && window.document !== undefined`: the node test world's window fake offers neither, and `typeof` fails `vp check`.
- Initialize injected buttons before attachment; prune their sync entries after detachment, not during creation.
- Accept element-node selection boundaries inside the reply body; a Range does not always start in a text node.
- Serialize manual starts, not the `wait: true` response; otherwise a new click cannot stop a blocked earlier reading.
