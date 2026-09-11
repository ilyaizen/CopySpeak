# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-09-12

### Added

- **Repeated text replays saved audio** — a double-copy, browser reading, hotkey or Play with text that history already holds for the active engine and voice plays the saved audio (with its caption sidecar) instead of calling the engine again. A paginated reading replays only when every fragment is saved, so it never mixes saved and fresh parts. A replay adds no history row, audio file or telemetry sample; history's **Regenerate** always synthesizes fresh audio (`regenerate_now`).
- **The Play page shows what is being read** — double-copy, hotkey and browser readings fill the text field with their text (`reading-started` event), and while audio plays the field becomes a live caption view of the audible part: spoken words dim, the current word is highlighted and kept in view.
- **Browser companion context menu** — right-click a selection and choose **Read with CopySpeak**.

### Changed

- **Browser companion shortcut is now `Alt+Shift+T`** (Chrome applies it on fresh installs only).

### Fixed

- **Play-page cache hits no longer duplicate history** — replaying saved audio used to add a new history row, copy the audio file again and record a ~0 ms synthesis sample that skewed time estimates.
- **Browser companion accepts selections that cross hidden or script text** — a selection spanning an inline `<script>`, SVG icon or hidden label was refused outright ("Select text in one permitted, ordinary HTML frame"); that text is now skipped.
- **Browser companion survives a quick second reading** — starting a new reading while the previous native host was still exiting found the single-instance pipe busy and failed with the "check native-host installation" badge; the host now waits for the pipe to free up.
- **Browser companion explains failures** — a refused selection or a failed synthesis now shows the `!` badge with the reason in its tooltip instead of ending silently.

## [0.2.1] - 2026-09-10

### Changed

- **Kokoro now has native live captions** — the local Kokoro engine reports `supports_captions: true`. The installer instead exports a duration-capable ONNX model (`kokoro-v1.0-duration.onnx`) using the pinned upstream exporter under an isolated Python 3.12 environment (Kokoro 0.8.4's NumPy 1.x dependency needs it), and installs `misaki[en]` for the English caption frontend. Existing audio-only installs show as needing reinstall; the old model is kept until the export succeeds.
- **Engine options are no longer suffixed with "(no live captions)"** — the profile manager now relies on the catalog's caption warning instead of decorating the label.

### Fixed

- **Browser companion word highlighting now survives sanitization** — the source map used to _replay_ a hand-picked subset of the sanitizer's rewrites and reject everything else, so a single `%` expanding to " percent" switched word highlighting off for a whole reading. `text_map` now aligns the raw selection and the spoken text word by word (order-preserving, with bounded resynchronization), so only a word the sanitizer invented loses its highlight — and it points at the punctuation it came from.
- **Live-caption verdicts are per fragment, not per reading** — the browser panel no longer reports "Word highlighting unavailable" for an entire reading while later fragments are still being synthesized, and a synthesis event belonging to a different reading is ignored instead of degrading the browser session.
- **Kokoro no longer aborts a reading on an unpronounceable bracket** — misaki passes punctuation it cannot pronounce straight through as a phoneme (an unbalanced `[` from a link-heavy web selection, a guillemet). Those symbols are absent from Kokoro's vocab and carry no sound, so the wrapper drops them instead of failing the fragment with `Kokoro cannot map pronunciation to model tokens: '[02:11'`. A missing _letter_ phoneme is still a hard error.
- Pass Kokoro installer/exporter options by their long names through `Invoke-Uv`; PowerShell binds short flags like `-o` before uv runs.
- `engine_status` for Kokoro checks for the duration-capable model, so the audio-only ONNX no longer counts as installed.

## [0.2.0] - 2026-09-09

### Added

- **Rename history readings** — a reading and the rest of its batch can be renamed without touching its text or audio files (`rename_history_reading`, up to 200 characters); the new name appears everywhere the reading is listed and can be cleared to go back to the automatic title.
- **Voice profile restore from history** — playing a history reading resolves the saved engine and voice ID through the profile catalog (`historyProfile`) and applies only the recorded settings (engine, voice, speed), keeping the current profile's pitch and effects; a missing voice profile aborts with a clear error instead of generating with a different voice.
- **Per-engine live-caption support flag** — the engine catalog reports `supports_captions` (native word/interval timing) for Piper, ElevenLabs, Cartesia, and Edge; the profile manager labels engines without it, warns when the active engine lacks captions, and a catalog test pins the flag to the native timing adapters.
- **Browser Companion developer preview** — a Chromium extension can read an ordinary webpage selection through CopySpeak with the toolbar action or `Alt+Shift+R`, then offers page-level Pause, Resume, and Stop controls. A native-messaging host and local named-pipe bridge start normal desktop playback; when sanitization, pagination, and engine caption timings map exactly to the original text, the page follows the current word, otherwise it safely keeps passage-only highlighting. The companion stops when the selected content changes, the page navigates, or its tab closes.

## [0.1.17] - 2026-09-08

### Changed

- **Speed and pitch are now independent knobs** — speed time-stretches the audio while preserving pitch (no more chipmunk voice when speeding up) and pitch shifts the voice without changing how long the reading takes. Previously both were a single resampling factor, so each knob moved the other. Both playback paths route through SoundTouch's WSOLA stretcher (`playback/time-stretch.ts`): the streaming PCM scheduler stretches each chunk before scheduling it and keeps `playbackRate` at 1, while the `<audio>` path bakes the pitch shift into the rendered blob at native duration and leaves speed to `preservesPitch` + `playbackRate`.
- **Speed and pitch ranges tightened** — speed caps at 0.5-2.0 (the IPC clamp was 0.25-4.0, inconsistent with every slider) and pitch narrows from 0.5-2.0 to 0.75-1.35, roughly ±5 semitones. A full octave was only tolerable while pitch also changed speed. Existing profiles keep their values, clamped into range on load; because pitch no longer secretly adds speed, a saved profile may read slower than it used to.

### Removed

- Dead WAV concatenation helpers (`concat_wav_files`, `find_wav_data_offset`) left over from the pre-streaming batch path.

## [0.1.16] - 2026-09-08

### Added

- **Resident local TTS daemons with GPU acceleration** — Piper, KittenTTS, Kokoro, and Pocket now keep their model loaded between utterances instead of reloading it per reading. Each runs as a per-engine daemon speaking protocol v2: a `READY 2` handshake, a format header, and length-prefixed 16-bit LE PCM chunks with an end marker — every byte count is declared, so text and binary share one pipe safely, and the single global temp-WAV path (where two concurrent syntheses collided) is gone. Wrappers select GPU execution providers where available (CUDA DLL directories registered with `os.add_dll_directory` inside the wrapper) and synthesize a throwaway phrase before the handshake, so `READY 2` means warm — first-request time-to-first-audio drops from a ~0.47s median to ~0.15s.
- **First-class Kokoro and Pocket wrappers** — `copyspeak-kokoro.py` drives kokoro-onnx directly (reusing the model files the installer downloads) and `copyspeak-pocket.py` uses pocket-tts's Python API with cached voice state, replacing per-invocation third-party CLIs that reloaded models and exposed no execution provider. Pocket becomes a `TtsEngine` variant with a migration promoting the old Local-preset profiles.
- **Native word timestamps** — Cartesia and ElevenLabs streams, Piper synthesis, and Edge batch output map provider timing metadata to per-fragment caption intervals (`tts/captions.rs`, `elevenlabs_timing.rs`); Edge gains a first-class wrapper script plus a caption-alignment test script.

### Changed

- **HUD captions highlight spoken words** — the marquee scroll is replaced with phrase captions that dim words as the active fragment plays. Position is driven by the audible fragment's PCM scheduler audio clock rather than synthesis events (which may describe a later fragment); native word intervals are available from ElevenLabs, Cartesia, Piper, and Edge.
- **Native caption intervals are preserved across playback** — stream framing, cache/history replay, and sample-count-based fragment concatenation keep intervals attached to their generated audio; word timings are never rescaled by text weights.
- **Speed/pitch changes reschedule unstarted PCM sources** — changing playback rate or pitch no longer leaves scheduled-but-unplayed chunks at the old settings.
- **Older engine installers stay on the one-shot path** — a wrapper still speaking protocol v1 is detected as unsupported; re-run the engine's installer to pick up the resident daemon.

## [0.1.15] - 2026-09-06

### Added

- **Recent history on the Play page** — a compact history rail below the reader shows the five most recent readings (when history is enabled), with a link to full History. Vertical mouse-wheel input scrolls the rail horizontally; the five-reading preview ends with a "View all in History" link.
- **Relative time labels in history** — readings show "Just now", "15 min ago", etc. via a locale-aware `historyTimeAgo` formatter.
- **Reading pagination count on history rows** — multi-part readings show a part count so a batched reading is distinguishable at a glance.
- **History-only Delete action** — delete a single reading from its History row (with confirmation), without touching playback controls.
- New tests for the recent history component and the playback store.

### Changed

- **Tighter Play page layout** — smaller side rail, visually hidden reader heading, denser spacing.

### Fixed

- **Audio queue recovery** — clear the shared audio queue on decode/play failure and invalidate pending decoding on Stop, so failed readings can be retried and stopped readings cannot restart.
- **History recording filenames** — more compact minute-key naming for saved audio files.

## [0.1.14] - 2026-09-06

### Added

- **Uninstall for every local TTS engine** — new `scripts/uninstall-engine.ps1 -Engine <kitten|piper|kokoro|pocket|edge>` removes the uv-managed engine directory and/or runs `uv tool uninstall`, streaming the same `[STEP]/[DONE]/[ERROR]` markers as the installers. Exposed as the `uninstall_engine` Tauri command and an Uninstall button (with confirmation) in the install dialog.
  - `-KeepModels` drops the venv/wrapper/manifest but keeps expensive model downloads.
  - uv itself is refused in both the script's `ValidateSet` and `uninstall_engine`: it is a shared prerequisite for every other local engine.
- **`engine_status` command** — probes each engine the way it is _used_ rather than trusting a manifest: `kokoro` requires its binary **and** both model files, the uv-tool engines require their binary on PATH, and `kitten`/`piper` require both `manifest.json` and `pyproject.toml`. Replaces `installed_voices`, whose result is now the `voices` field.
- **`scripts/test-installer-lib.ps1`** — runnable self-check for the installer helpers: asserts every interactive helper returns its default under `COPYSPEAK_NONINTERACTIVE=1` (run it with `< NUL`, so a hang _is_ the failure), parses every `scripts/**/*.ps1` and asserts it is ASCII-only, and runs `uninstall-engine.ps1` against a throwaway `LOCALAPPDATA` to prove the never-installed path is a clean exit-0 no-op.
- **`Add-UvToPath` / `augment_path_for_local_engines`** — uv and its tool shims install into `~/.local/bin`, which uv adds to the _user_ PATH. A running process keeps its launch-time PATH, so engines installed during a session used to look missing until an app restart. Both the shared installer lib and CopySpeak startup now prepend the known uv locations.
- **27 English Cartesia voices** in the static engine catalog (was 2), ids taken verbatim from the account's `GET /voices` dump. All are tagged `language: "en"` — the Cartesia API has no region field, so no `en-US`/`en-GB` locale is claimed; the accent, where the provider states one, stays in the description. The picker groups them by gender.
  - New test `cartesia_voice_ids_are_unique_uuids` guards the hand-copied id table (UUID shape + no duplicates).
  - `CartesiaTtsBackend::voice_display_name` now resolves labels from the catalog instead of a second hard-coded id/name map.

### Changed

- **Responsive desktop UI** — the main window now starts at 775×580, resizes down to 360×400, and keeps navigation, content, controls, and settings usable at narrow widths.
- **Flatter app layout** — Play, History, Voices, Engines, Settings, and Onboarding now share the same divider-led visual language without nested cards. The Play page uses a larger labeled editor and responsive quick settings.
- **Reading-level history** — paginated fragments now render as one reading with ordered full text, one playback/delete row, resolved engine and voice labels, duration, and a compact part count.
- **Cartesia is the fresh-install default** — new configs select the bundled Cartesia profile, and onboarding accepts and validates a Cartesia API key without spending synthesis credits.
- **ElevenLabs streaming pins `pcm_24000` instead of `pcm_44100`** — ElevenLabs gates `pcm_44100` output behind its Pro tier, so every streamed fragment failed at `phase=headers` with 403 `output_format_not_allowed` on non-Pro accounts. `STREAM_OUTPUT_FORMAT` and the `ChunkStream` meta now use 24 kHz / mono / 16-bit; the end-of-stream WAV wrap inherits the rate from the stream meta (no hardcoded rate), and the wiremock request-shape test asserts the new pin.
- **`edge` no longer has an installer** — `scripts/install-edge-tts.ps1` is gone and the Engines page no longer offers an Install dialog for it (no `installerId`). `engine_status` still probes `edge-tts` on PATH, and a missing binary surfaces `uv tool install edge-tts` in the synthesis error instead of a `pip install` hint.
- **Install dialog is driven by engine metadata** — the hard-coded `SHARED`/`ENGINE_SIZE` maps moved into `engine-meta.ts` as `voiceMode` (`per-voice` | `shared` | `none`) and `downloadSize`. `none` engines (uv, pocket) no longer render a meaningless voice checklist.
- **Voice picker is now primary over manual entry** — in the voices route, the Manual Voice input is disabled while the profile's voice matches a catalog id. The picker gains a "Custom / manual id…" row (`onmanual` prop) that unlocks and focuses the input; picking a real voice re-locks it. The unlock (`manualOverride`) is component-local state, reset on profile/engine change — no config schema change.
  - When no picker is rendered (engines without a catalog or refresh support), the input is always enabled and relabeled "Voice".
- **Cartesia voice refresh keeps language metadata** — `CartesiaVoice` gains a `language` field, mapped through to `VoiceCatalogEntry.language` in `list_tts_voices`, so a refresh no longer strips the language off every voice.
- **Renamed "Microsoft Azure" to "Microsoft Foundry"** — engine catalog label/description and the `en.json` engine + setup strings. Microsoft Learn docs URLs left unchanged (still `learn.microsoft.com/.../azure/ai-services/speech-service/`).
- **Piper daemon prewarm moved off the setup thread** — `prewarm_piper` ran inline in `setup()` while holding the config mutex, delaying the control server and clipboard listener. It now runs on its own thread with a cloned `TtsConfig`.

### Fixed

- **History playback controls** — each reading now uses one Play/Stop button, retains Play for replay, starts replay from the beginning, and lets another history row take over active playback. Loading decoration no longer changes the accessible action name.
- **History voice names** — saved readings resolve labels by engine and voice ID through profiles and the engine catalog; unknown IDs show “Saved voice” while preserving the raw ID in the tooltip.
- **ElevenLabs live PCM playback corrupted samples at network chunk boundaries** — `PcmStreamScheduler` now carries incomplete 16-bit interleaved frames into the next chunk instead of dropping their bytes before `pcm16LeToFloat32Channels`. Carry state is cleared at fragment end and stop; empty terminal events remain control-only. Development-only chunk size, arrival-gap, buffered-duration, and underrun diagnostics distinguish byte-alignment corruption from network starvation without increasing the 250 ms prebuffer.
- **Engine installers failed when launched from the app** — tauri's `resource_dir()` is built from a canonicalized exe path; on Windows `std::fs::canonicalize` returns `\\?\` verbatim paths, which leaked into `powershell -File` and `$PSScriptRoot`, breaking the `lib/copyspeak-engine-install.ps1` dot-source in every installer. `resolve_script` now strips the prefix once at the source, so installers work in dev and packaged builds; the per-script `$PSScriptRoot -replace` workarounds were removed.
- **`install-kokoro.ps1` did not parse under Windows PowerShell 5.1** — the file is BOM-less UTF-8, which 5.1 reads as ANSI, so an em dash inside a `Write-Host` string decoded to bytes containing a `"` and terminated the literal early. The script failed before running a single line on any machine without `pwsh`. All installer scripts are now ASCII-only, and the new self-check enforces it.
- **Installers hung forever when launched from the app** — `install_engine` spawns PowerShell with stdin closed, but the shared `Get-Confirmation` and `Select-VoiceFromMenu` helpers still called `Read-Host`. Reinstalling `edge` or `pocket` (whose force prompts did not check `-Voices`) blocked with no output. Both helpers now return their default when `COPYSPEAK_NONINTERACTIVE=1`, which the Rust spawner sets; the child also gets `-NonInteractive`.
- **Failed installs reported success** — `install-piper.ps1` caught per-voice download errors and still exited 0, and `install-kokoro.ps1` exited 0 with its model files missing, so the UI showed a green "Install complete" for an engine that could not synthesize. Both now exit 1, and partial `.onnx`/`.bin` downloads are deleted so the next run does not mistake them for a finished file.
- **Packaged builds could not install piper or kitten** — `tauri.conf.json` bundled the `.ps1` scripts but not the `scripts/piper/*.py` and `scripts/kitten/*.py` wrappers they copy, nor the new uninstaller. `resolve_script` also never looked in the Tauri resource directory; it now checks `resource_dir()` first, then the dev repo path. (The uninstaller is registered as `uninstall-*.ps1`: Tauri's resource copier fails with `Access is denied` when a non-glob source is paired with a directory destination.)
- **Engine installs could not be observed or retried** — `uv` and `edge` bypassed the streamed install dialog for a fire-and-forget launch that toasted "Installer launched" regardless of outcome. Every engine with an `installerId` now opens the dialog with its live log.
- **Redundant TTS health checks at startup** — `app-footer.svelte` fired `test_tts_engine` (a live API call) on mount and again on every global `config-changed` event, producing five Cartesia round-trips on launch. Checks are now throttled to one per 30s with an in-flight guard; explicit profile switches and initial mount bypass the throttle.
- **Health-check logs and toasts reported a blank model** (`Cartesia ()`) — the label was built from the global `TtsConfig` while the backend came from the active profile's engine options. Those global model/voice/format fields are `skip_serializing` (the profile owns them), so a config loaded from disk deserializes them empty. New `effective_backend_name` mirrors the override order in `create_backend_from_effective` — profile options, then global config, then `"unset"` — and covers OpenAI, ElevenLabs, Google, Microsoft and Edge, which read the same blanked fields. `test_tts_engine_config` has no profile to consult but reaches a user-facing toast, so it gets the same blank guard.
  - Regression test pins the serde subtlety: the blank appears only when the `cartesia` object is _present_ (carrying the persisted `api_key`), because the field-level `#[serde(default)]` then beats the container-level one.
- **HUD window flashed white on launch** — the HUD was created with `visible: true`, so WebView2 painted its default white surface at the OS-chosen position for a frame before Tauri applied the off-screen coordinates. It is now created hidden and shown only after being parked off-screen. The stale `x`/`y: 10000` in `tauri.conf.json` (which disagreed with `move_hud_offscreen`'s `-10000`) were removed, leaving one source of truth for the park position.
- **Kitten `model` option is wired end-to-end** — `KittenEngineOptions.model` now reaches the wrapper: `first_class_local_cli()` passes `--model {model}`, `CliTtsBackend::build_args` resolves the new `{model}` placeholder (dropping the flag when unset, so the nano default applies), and `create_backend_from_effective` threads the profile's model through the new `ProfileEngineOptions::kitten()` accessor. The installer's profile JSON and both frontend Kitten templates include `--model {model}`, keeping all three sources of truth in sync; the `ponytail:` dead-code marker on the field is removed.
- **Edge-TTS install hint named `pip`** — the not-found error text now matches the uv-based install used everywhere: `uv tool install edge-tts`.

### Removed

- **`scripts/install-edge-tts.ps1`** — deleted; `installer_script_for()` no longer maps `edge`, Edge's `engine-meta.ts` entry lost its `installerId`/`voiceMode`/`downloadSize`, and the onboarding page no longer offers an "Install edge-tts" button (a missing binary now explains `uv tool install edge-tts` in the synthesis error).

## [0.1.13] - 2026-08-02

## [0.1.12] - 2026-07-31

### Added

- **First-class local TTS engines** — Kitten, Piper, and Kokoro are now dedicated `TtsEngine` variants instead of `Local` presets, each with its own hard-coded CLI contract, per-engine options struct, and catalog entry (voices, labels, docs).
  - `TtsEngine` gains `Kitten`/`Piper`/`Kokoro` (serde `"kitten"`/`"piper"`/`"kokoro"`); new `KittenEngineOptions`/`PiperEngineOptions`/`KokoroEngineOptions` and matching `ProfileEngineOptions` arms (matches_engine/from_engine_map/Serialize/Deserialize/accessors).
  - Synthesis dispatch (`helpers.rs`) shells out each via its verified installer wrapper contract (Kitten/Piper via `uv run … copyspeak-*.py --text-file`; Kokoro via `kokoro-tts … --model/--voices`). Per-engine knobs (`model`/`length_scale`/`speed`) are exposed but not yet threaded to the CLI (`ponytail:`-marked).
  - Bundled first-class Kitten profile (`default_kitten_profile`) seeded into fresh installs and added to existing configs by the v3→v4 migration.
- **Built-in profile templates** — a "New profile from template" picker in the profile manager copies a fresh `VoiceProfile` seed (Kitten-Rosie, Piper-Amy, Kokoro-Heart) into profiles; a one-time copy, never a live reference.
- **Local CLI "Start from" template** — the custom-CLI escape hatch gains a Kitten/Piper/Kokoro-style prefill select for command/arguments/voice.
- **In-app engine installer dialog** — Kitten/Piper/Kokoro installs now run streamed in a dismissible dialog: per-voice selection (Piper per-voice downloads; Kitten/Kokoro shared model), live log, per-voice status (pending/installing/done/failed), and per-voice Retry. Reopens to add voices later, pre-checking already-installed voices.
  - New `install-dialog.svelte` + `install-store.svelte.ts` (persistent `install-progress` listener that survives the dialog being dismissed); `-Voices` param and `[STEP]`/`[DONE]`/`[ERROR]` markers on the three install scripts; shared `Write-EngineManifest` helper writes `%LOCALAPPDATA%\CopySpeak\engines\<engine>\manifest.json`.
  - New `installed_voices` Rust command reads the manifest for the pre-check.
- **Pocket TTS installer** — re-added as a local engine option: `install-pocket.ps1` installs `pocket-tts` as a uv tool, accepts `-Force`/`-SmokeTest`, and emits a CopySpeak profile snippet. Registered in Rust `installer_script_for()`/`local_engine_spec()`, frontend `LOCAL_ENGINES`, and locale strings.

### Changed

- **"Preset" vocabulary split** — `LOCAL_PRESETS` (the setup/installer registry) renamed `LOCAL_ENGINES`; the `preset` field on `LocalEngineOptions` and its catalog option are deleted. "Preset" now means a built-in profile template.
- **Config schema v4** — `TtsConfig::default().schema_version` → 4; new idempotent `migrate_add_kitten_profile_v4` staged migration adds the bundled Kitten profile (user profiles untouched, no remapping). Legacy top-level `preset`/`command`/`args_template`/`voice` fields retained for pre-v2 deserialization.
- The Local catalog entry is reduced to `command`/`args_template` only (voice is free text).
- **Installer no longer opens a detached console** — `install_engine` spawns the PowerShell script with piped stdout/stderr + `CREATE_NO_WINDOW`, streaming lines as `install-progress` Tauri events (terminal event carries `done`/`exit_code`). uv stays a fire-and-forget launch (no voice concept).

### Removed

- **Chatterbox** — removed entirely (voice-cloning shape doesn't fit the generic voice model): catalog voice, `LOCAL_ENGINES` entry, `install.rs` script mapping, `scripts/install-chatterbox.ps1`, `scripts/chatterbox/`, the `copyspeak-chatterbox.py` path rewrite in `WRAPPERS`, i18n keys, and doc references.

### Breaking Changes

- `TtsEngine` enum/union gains three variants; any exhaustive match (Rust) or `Record<TtsEngine, _>` (TS) must cover them.
- `LocalEngineOptions.preset` field removed; configs carrying it deserialize with the field ignored.

### Fixed

- Removed 1.2s Windows audio preroll hack that delayed playback start.
  - Replaced with a 10ms linear fade-in (`applyFadeIn` in `audio-utils.ts`) that prevents clipping without a perceptible delay.

## [0.1.11] - 2026-07-30

### Added

- **Select Dropdown for Local CLI Presets**: Replaced the free-text `Preset` field for local CLI engine profiles with a select dropdown listing supported presets (`kitten-tts`, `piper`, `kokoro`, `chatterbox`, `custom`).
- **Default Presets Auto-population**: Changing the preset automatically populates the default command executable, argument templates, and fallback/default voice for that preset.
- **Dynamic Voice Catalog Filtering**: The local CLI voice catalog now contains static entries for all supported local engine voices (KittenTTS, Piper, Kokoro, Chatterbox). Selecting a local preset dynamically filters the Voice Picker dropdown to only show the voices belonging to the selected preset.
- **Persistent Piper daemon** — the Piper voice model now stays loaded in RAM between utterances instead of being re-read from disk on every synthesis.
  - `scripts/piper/copyspeak-piper.py` gained a `--serve` mode: it loads the model once, prints `READY`, then answers one JSON request per stdin line (`{"text", "output"}`) with `{"ok"}` / `{"ok", "error"}`. Stdin EOF ends the process, so the daemon exits with CopySpeak.
  - New `src-tauri/src/tts/piper_server.rs` owns the daemon: `prewarm()` starts it on a background thread, `try_synthesize()` does the stdin round-trip, `shutdown()` kills it on quit.
  - `CliTtsBackend::synthesize` routes Piper through the daemon and falls back to the existing one-shot command whenever it isn't available — engine dirs with a pre-daemon wrapper keep working until `install-piper.ps1 -Force` is rerun.
  - The fallback is self-healing: a failed daemon request (dead pipe, crashed interpreter) serves that utterance one-shot and respawns the daemon in the background, so the next utterance is back on the fast path without a restart.
  - Known limitation: `abort_synthesis` does not interrupt synthesis on the daemon path (`ACTIVE_CLI_PID` is only set for one-shot subprocesses). Stop still cuts playback; it just can't cancel an in-flight daemon synthesis, which is ~0.3 s for normal utterances but scales with text length.
  - `CliTtsBackend::serve_args` derives the daemon argument list from the configured `args_template` by dropping the per-utterance `{input}`/`{output}` flags.

### Changed

- **Engine catalog options**: Updated `EngineOptionDescriptor` in both Rust backend and TypeScript frontend to support optional `choices`, allowing `select` option kinds.
- **Piper pre-warms at app startup** — `prewarm_piper()` runs from the Tauri `setup` hook when the active profile is a local Piper engine, so the first utterance no longer waits for the model load. A voice or profile switch re-warms after the next utterance.
- **`--output` is no longer required** by `copyspeak-piper.py` when `--serve` is given.

### Fixed

- **`bun check` type errors** — removed the unused `hotkeyEnabled`/`hotkeyShortcut` bindings in `play-page.svelte`, and guarded the possibly-null `localConfig` in the double-copy-window `onchange` handler in `settings-page.svelte`.

## [0.1.10] - 2026-07-07

### Changed

- **Default config version** bumped to `0.1.10`.

- **Default profiles shipped with new installs** now include four presets:
  - **Edge** (active, default) — `en-US-AvaMultilingualNeural`, speed 0.9, pitch 1.3.
  - **ElevenLabs** — George (`JBFqnCBsd6RMkjVDRZzb`), `eleven_turbo_v2_5`, speed 1.1, pitch 0.9.
  - **Cartesia** — Harper (`c5d00dfb-...`), `sonic-3.5`, speed 0.9, pitch 1.3.
  - **Gemini** — Orus, `gemini-2.5-flash-preview-tts`, speed 1.1, pitch 0.85.

- **Default profile effects** changed: effects now enabled with `walkie_talkie` as the default effect (was disabled/none).

- **Default HUD position** changed from `bottom-center` to `bottom-left`.

## [0.1.9] - 2026-07-06

### Added

- **`has_engine_credentials` command** — New IPC command checks whether an engine has credentials available (config.json or .env) without making HTTP requests. Used by the profile manager to show/hide the "Set up engine credentials" hint accurately.

- **Config-changed event listeners** — Play page and Voices page now listen for the `config-changed` Tauri event and reload config automatically when it changes externally.

- **Searchable voice picker** — the flat per-engine voice `<Select>` in the profile editor is replaced by `voice-picker.svelte`: a portaled popover with a live search box, automatic grouping (by gender for OpenAI/Google/Cartesia/ElevenLabs, by BCP-47 locale for Edge), inline metadata (`gender · language`) and a check mark on the active voice. Escape / outside-click closes it; the panel flips above the trigger when space below is tight. The manual Voice ID input remains below it as the escape hatch.

- **Cartesia voice refresh** — `supports_voice_refresh` is now `true` for Cartesia. New `CartesiaTtsBackend::list_voices()` calls `GET https://api.cartesia.ai/voices` (`X-API-Key` + `Cartesia-Version`) and maps results into `VoiceCatalogEntry`; on API failure `list_tts_voices` falls back to the static catalog list (mirroring the ElevenLabs pattern). The picker's Refresh button covers both ElevenLabs and Cartesia.

- **`.env` secret loading** — CopySpeak now reads a `.env` file placed next to `copyspeak.exe` (in dev: `src-tauri/target/debug/`). Keys defined there override the values typed in the Engines UI; UI-typed keys in `config.json` remain the fallback. Env values are never written back to disk. See `.env.example` for the full variable list (`OPENAI_API_KEY`, `ELEVENLABS_API_KEY`, `CARTESIA_API_KEY`, `GEMINI_API_KEY`/`GOOGLE_API_KEY`, `MICROSOFT_API_KEY`/`AZURE_API_KEY` + `MICROSOFT_ENDPOINT`, `POST_PROCESS_API_KEY`).
  - New `secrets.rs`: `load_dotenv()` (naive `KEY=VALUE` parser, called once after logging init in `main.rs`) and `resolve(config_val, env_names)` (env-wins resolution with alias support, e.g. Gemini accepts both `GEMINI_API_KEY` and `GOOGLE_API_KEY`).
  - All credential read-sites now route through `secrets::resolve`: the five TTS backends (`openai`, `elevenlabs`, `cartesia`, `google`, `microsoft`), the three cloud credential-check commands, and the live Groq post-processing path (`post_process::process`).

### Changed

- **Voice label priority** — `voice_display_name` now prioritizes the profile's `voice_label` from the catalog over the raw `voice_name` from config, giving cleaner filenames in history.

- **Voice label backfill** — Default profile and migration (`migrate_tts_config`) now populate `voice_label` from the voice catalog for profiles that have none.

- **HUD window deferred show** — HUD window starts with `visible: false` in `tauri.conf.json` and explicitly calls `window.show()` in `onMount` after transparent CSS is applied, preventing a white flash.

- **Credential hint uses backend check** — Profile-manager now calls `has_engine_credentials` (covers config + .env) instead of only checking the raw `config.json` api_key field.

- **`VoiceCatalogEntry` gains a `gender` field** (`Option<String>`, serialized as `gender`) surfaced in `src/lib/types.ts`. `catalog.rs::voice()` helper now takes a `gender` argument.

- **Enriched static voice metadata:**
  - OpenAI (11 voices) — labels capitalized, gender + concise style description per voice.
  - Google Gemini (29 voices) — gender + Google's documented style descriptor per voice; grouped Female/Male in the picker.
  - Cartesia (2 static voices) — gender + description; language set to `None` (multilingual).
  - Edge (30 voices) — `language` carries the BCP-47 locale parsed from the voice id (`en-US`, `en-GB`, …) and `gender` is now populated from the live `edge-tts --list-voices` metadata (Microsoft's published genders). The picker groups Edge by **locale** (region is the more useful cluster) with gender shown as per-row meta; cloud engines still group by gender. `en-US-DavisNeural` and `en-US-AmberNeural` are absent from the current live list (likely deprecated) and keep `gender: None`.
  - ElevenLabs — static catalog expanded from 1 (Rachel) to **21 real premade voices**. IDs/names/genders fetched from `GET /v1/voices` (2026-07-06), each with an `"{accent} · {gender} · {style}"` description; `language` set to `None` since gender is the picker group key (avoids a redundant per-row `en`). Rachel was rotated out of the premade set and is dropped — still usable via the manual Voice ID escape hatch.

### Fixed

- **Play-page reload→save→emit loop** — Added `externalLoad` guard in the config `$effect` to skip auto-save when config was loaded via the `config-changed` event, breaking the infinite reload→save→emit→reload cycle.

- **Footer voice label rendering** — Switched from `||` to `?? null` so legitimately empty-string `voice_label` values are preserved instead of falling through to `voice`.

- **ElevenLabs voice metadata** — `list_tts_voices` now propagates the `gender` label from the ElevenLabs API into the catalog entry (previously dropped).

## [0.1.8] - 2026-07-05

### Added

- **Engines page restored (`/engines`)** — dedicated surface for per-engine setup, decoupled from voice profiles. Hosts API key entry (+ endpoint for Microsoft), local-engine installers (kitten, piper, kokoro, pocket, chatterbox, uv), engine **Test** buttons (via `test_tts_engine_config`), and docs links. Sidebar groups Cloud and Local engines; uv-missing banner shown when the local prerequisite is absent.
  - New `routes/engines/+page.svelte`, `components/engine/engine-setup.svelte` (orchestrator), `components/engine/engine-panel.svelte` (presentational card), and `components/engine/engine-meta.ts` (single source of truth for setup metadata — credentials, installer ids, docs).
  - "Engines" nav item restored in `app-header.svelte`.

- **Test buttons for local engines** — every local engine panel (piper, kokoro, kitten, chatterbox, pocket) now shows a Test button that runs a real synthesis ("Hello.") through the engine's CLI wrapper and verifies the bytes are a valid audio file. New `test_local_engine` Tauri command (`commands::tts::health`) builds a `CliTtsBackend` from each installer's stable `{command, args_template}` and calls `synthesize()`. `engine-panel.svelte` Test block no longer gated on `kind === "cloud"`; `engine-setup.svelte::runTest` branches to `runLocalTest` for local entries.
  - Reuses the legacy `engine.localEngine.testEngine/engineWorking/engineFailed` i18n keys.

- **Interactive English voice selection in installers** — every local-engine installer now prompts (in its console window) to pick an English voice and bakes it into the smoke test and emitted profile snippet:
  - `install-piper.ps1`: numbered menu of en_US voices (amy, lessac, ryan, joe, libritts); downloads the chosen `.onnx` + `.onnx.json` pair from `huggingface.co/rhasspy/piper-voices` into `voices\` if missing. New `-SkipVoiceDownload` switch.
  - `install-kokoro.ps1`: menu of built-in English voices (af_heart, af_bella, af_nicole, af_sarah, am_adam, am_michael, bf_emma, bm_george); default `af_heart`.
  - `install-kittentts.ps1`: menu of the 8 built-in voices (Rosie, Bella, Luna, Kiki, Jasper, Bruno, Hugo, Leo); default `Rosie`.
  - New shared helpers `Select-VoiceFromMenu` and `Get-Confirmation` in `scripts/lib/copyspeak-engine-install.ps1`; the latter replaces the unused `Confirm-Install`.
  - All installers now prompt interactively for reinstall (force) when not passed `-Force`; default is No (no forced reinstall).

### Fixed

- **Piper installer `uv` self-dependency collision** — `New-EngineProject` in `scripts/lib/copyspeak-engine-install.ps1` ran `uv init --bare` with no `--name`, so the project was named after the directory basename. For piper this collided with the PyPI package name (`piper`), making `uv add piper` fail with _"Requirement name `piper` matches project name `piper`, but self-dependencies are not permitted"_ (exit code 2). Fixed by passing `--name copyspeak-<dir>` to `uv init`. Harmless for chatterbox/kitten whose dir names already differ from their PyPI package names.

- **Piper voice model not found** — `scripts/piper/copyspeak-piper.py` resolved `voices_dir` as `Path(__file__).parent / "voices"`, but the wrapper lives in `<engine_dir>/scripts/` while `install-piper.ps1` downloads `*.onnx` into `<engine_dir>/voices/`. So the wrapper looked in `<engine_dir>/scripts/voices/` (always empty) and reported _voice model not found … Available: []_ even though the installer had downloaded the model correctly. Fixed by going up one level: `Path(__file__).parent.parent / "voices"`. Updated the docstring hint to point at `<engine_dir>/voices/`.

- **Piper PyPI package-name collision (`piper` vs `piper-tts`)** — `install-piper.ps1` ran `uv add piper`, but PyPI's `piper` is an unrelated bioinformatics toolkit (_databio/pypiper_, module `pypiper`). The actual TTS engine ships as `piper-tts` (module `piper`). So `from piper import PiperVoice` failed with _No module named 'piper'_ even though `uv add` reported success. Fixed by depending on `piper-tts`. (The earlier `--name copyspeak-<dir>` fix unmasked this: previously the project self-named `piper`, colliding with the package `piper` and failing `uv add` outright.)

- **Piper wrapper using removed 0.x `synthesize(wf, text)` API** — `piper-tts` 1.x dropped the `PiperVoice.synthesize(wave_file, text)` signature and the WAV header is no longer auto-set by the caller; the wrapper hit `# channels not specified`. Switched to `synthesize_wav(text, wf)`, which owns WAV format setup via `set_wav_format=True`.

- **Chatterbox voice-prompt path** — `scripts/chatterbox/copyspeak-chatterbox.py` resolved voice prompts as `Path(__file__).parent / "voices"`, but the wrapper lives in `<engine_dir>/scripts/` while `install-chatterbox.ps1` creates `voices/` at the engine root. Same root cause as the piper path bug. Fixed by going up one level: `Path(__file__).parent.parent / "voices"`.

- **Kokoro installer missing model files** — `install-kokoro.ps1` ran `uv tool install kokoro-tts` and stopped, but the `kokoro-tts` binary requires `kokoro-v1.0.onnx` (~310 MB) and `voices-v1.0.bin` (~25 MB) that it neither bundles nor auto-downloads. Every synthesis failed with _"Required model files are missing"_. The installer now downloads both into `<engine_dir>/kokoro/models/`, and the args_template (installer snippet + `local_engine_spec` kokoro entry in `commands/tts/health.rs`) injects `--model`/`--voices` pointing at them. New `-SkipModelDownload` switch for offline/reuse.

- **Installer window auto-closing on failure** — the launcher wrapper in `commands/install.rs` ran the script inline, so a terminating error inside the installer (`throw`) escaped past the `ReadKey` pause and the console window closed before the user could read the error. Wrapped the script call in `try/catch`; the "Press any key to close" prompt now runs on both success and failure.

- **`install-chatterbox.ps1` interactive reinstall prompt** — added the same `Get-Confirmation` force prompt as the other installers for consistency.

- **Edge-TTS synthesis crash on `--rate`** — `edge-tts --rate -10%` failed with `argument --rate: expected one argument` (exit code 2): argparse parsed the leading `-` on `-10%` as a flag. Speed was being threaded backend→synthesis for Edge (`--rate`), OpenAI (`"speed"` in JSON body), and HTTP (`{speed}` placeholder), but the frontend **already** applies playback speed itself via `audioEl.playbackRate` (`playback-store.svelte.ts`), so speed was either applied twice (cloud) or crashed (Edge). Fixed by making speed a frontend-only concern mirroring pitch: removed the `speed` parameter from `TtsBackend::synthesize` and all 8 backends, dropped Edge's `speed_to_rate` helper + `--rate` arg, dropped OpenAI's `"speed"` body field, and dropped the HTTP `{speed}` placeholder. The persisted `profile.speed` field and `set_playback_speed` IPC are unchanged (frontend reads `activeProfile.speed` → `playbackRate`); saved audio files no longer bake in profile speed.

- **Local CLI engine wrapper paths** — pre-v0.1.8 local profiles (kitten, piper, chatterbox) stored the engine wrapper as a CWD-relative path (`scripts/

- copyspeak-<engine>.py`), which broke because the Tauri process CWD is `src-tauri/`(dev) or the install dir (packaged), not the engine install dir. Migration in`config/tts.rs::migrate_tts_config`now rewrites bare legacy wrapper paths to`{engine_dir}/<engine>/scripts/...`on every load (idempotent).`install-chatterbox.ps1`'s emitted `profileJson`was also missing`command`/`args_template`, so the printed snippet was non-functional — now baked in with the correct absolute path.

- **Screenshot script window title** — `screenshot-window.ps1` defaulted to `"CopySpeak TTS"` but the actual Tauri window title is `"CopySpeak"`. Capture would always fail unless the title was passed manually. Fixed default.

- **Screenshot capture script** — `scripts/capture-screenshot.mjs` reads version from `tauri.conf.json`, captures the Tauri window via `screenshot-window.ps1`, saves to `static/screen-v{version}.png`, and patches `screenshots.svelte` to reference the new file. One-command screenshot refresh: `node scripts/capture-screenshot.mjs`.

### Removed

- **`supports_speed` catalog flag** — `EngineCatalogEntry.supports_speed` (Rust + `EngineCatalogEntry` TS type) removed; it was unused in the UI and became dishonest once speed stopped being a synthesis parameter. HTTP `{speed}` template placeholder no longer substituted (always emitted `1` at synthesis; user templates should drop it).

- **Pocket engine** — dropped from the Engines page, About page, `local_engine_spec`, `installer_script_for`, `engine-meta.ts::LOCAL_PRESETS`, `i18n/types.ts`, and `scripts/install-pocket.ps1` deleted. The PyPI `pocket-tts` CLI is voice-cloning-first: `--voice` takes a path to a conditioning audio file (not a voice name), the installer-baked `--voice default` was always invalid, and the package pulls in PyTorch (~hundreds of MB). Runtime filename normalization (`pocket-tts → pocket` in `helpers.rs::engine_identifier`) and the `hud.rs` display-name branch are kept as defensive code for any legacy profile still referencing pocket.

- **`voice-credentials.svelte` deleted** — its contextual-credential UX is replaced by the Engines page. Setup metadata consolidated into `engine-meta.ts` (DRY).

- **Empty `/engine` and `/profiles` route directories removed** (left behind by the prior consolidation).

### Changed

- **Voices page (`/voices`) redesigned** — `profile-manager.svelte` restructured from a flat list into grouped cards (Identity, Engine & Voice, Sound, Advanced). Credentials no longer live here; the Engine row shows a passive "Set up engine credentials" hint linking to `/engines` when the active profile's engine key is missing.

- **Credential persistence fixed** — the per-engine config structs (`OpenAIConfig`, `ElevenLabsConfig`, `CartesiaConfig`, `GoogleTtsConfig`, `MicrosoftTtsConfig`) were `#[serde(skip_serializing)]` at the field level in `TtsConfig`, so API keys vanished on restart. Now only `api_key`/`endpoint` persist; profile-owned knobs (model, voice, format, etc.) remain skip-serialized per the profile/global boundary in `docs/profile-engine-settings.md`.

- **Landing screenshot updated** — `screenshots.svelte` now references `screen-v0.1.7.png` (was stale `screen-v0.1.4.png`). Fresh screenshot captured from the v0.1.7 Play page.

## [0.1.7] - 2026-07-05

### Added

- **Voice profiles system** — Create, edit, and switch between named voice profiles, each with its own engine, voice, speed, pitch, and effects settings.
  - New `VoiceProfile` and `ProfileEffects` types; `TtsConfig` now carries `active_profile_id` and `profiles`.
  - New Profiles page (`/profiles`) with inline profile manager.
  - New `speak_now_with_profile` Tauri command registered in `main.rs`.
  - Profiles nav item added to app header.

- **Expanded TTS engine types** — Added `EdgeTtsConfig`, `GoogleTtsConfig`, `MicrosoftTtsConfig`, and `HttpTtsConfig` to `TtsConfig`; `TtsEngine` union extended with `"edge"`, `"google"`, `"microsoft"`, and `"http"`.

- **Engine catalog types** — `EngineCatalogEntry`, `VoiceCatalogEntry`, `EngineOptionDescriptor` interfaces for server-driven engine metadata.

- **Centralized save bar** — Shared `save-bar.svelte.ts` store replaces per-page save bar markup in settings, profiles, and engine pages. Single save bar rendered in `+layout.svelte`.

- **Page motion transitions** — `MotionWrapper` component with fade+slide-up entrance animation on route changes; respects `prefers-reduced-motion` and a `motion-disabled` class.

- **Portal utility** — `portal()` action in `utils.ts` teleports a node to `<body>` so `position:fixed` escapes transformed ancestors.

- **Granular markdown sanitization toggles** — Each markdown strip feature (code blocks, inline code, headers, links, bold/italic, lists, blockquotes) can now be individually enabled/disabled in Settings → Sanitization. Inline code stripping defaults to off to preserve backtick-wrapped terms in technical text.

- **Post-processing providers expanded** — Added `xai`, `aws`, and `cerebras` to `PostProcessingProvider`; new `PostProcessingPromptPreset` type and `selected_prompt_label` / `prompt_presets` fields in `PostProcessingConfig`.

### Changed

- **Markdown stripping respects config** — `strip_markdown()` now accepts `MarkdownSanitizationConfig` and skips disabled features instead of always stripping everything.
- **Import/export settings refactored** — Internal cleanup of dialog state and validation flow.
- **Pi extension** — Removed unused `prepareText` wrapper; switched from custom `parseJson` to `JSON.parse`.

### Fixed

- **`speak_now_with_profile`** — Now exposed as a `#[tauri::command]` (was `pub(crate)`) so the frontend can invoke it.
- **Rust compiler warnings** — Added `#[allow(dead_code)]` on post-processing structs/functions not yet wired to the UI.
- **Devtools and browser flags** — Main window now enables devtools and sets `--force-prefers-no-reduced-motion`, `--enable-smooth-scrolling`, and file-access flags for local dev.

## [0.1.6] - 2026-07-04

### Changed

- **Product naming reverted to `CopySpeak`** — Dropped the `-tts` suffix and lowercasing introduced in the 0.1.5 rename. User-facing strings, package identity, and bundle metadata now use `CopySpeak` consistently.
  - `package.json` and `src-tauri/Cargo.toml` package name restored to the `CopySpeak`/`copyspeak` identity.
  - `src-tauri/tauri.conf.json`: `productName`, `publisher`, and main window `title` → `CopySpeak`.
  - Landing page (hero, footer, screenshots), in-app header, browser title, onboarding, settings tooltips, and HTML history-export titles now read `CopySpeak`.
  - Locale strings in `en.json` updated (`landing.hero.title`, `screenshots.*`, onboarding/welcome, about, app title, OpenAI engine detail).
  - `scripts/claude-copyspeak-hook.mjs` running-exe matcher updated for the new `CopySpeak.exe` bundle name.
- **Tagline now visible** — `Modern AI TTS Orchestrator` (`header.tagline`) is rendered under the in-app header title (was commented out) and added as the landing hero subtitle.
- **Engine page refactor** — Consolidated `engine-page.svelte` from per-engine subcomponents into a single data-driven panel driven by `ENGINE_TABS`. Removed the cloud-TTS API-key dialog, credential check/test helpers, and unused category metadata; engine settings are now edited inline with a single save flow.
  - Added `placeholderKey` per engine tab and `install_engine`/`check_command_exists` (uv) wiring for local engines.
  - Added English locale strings for engine API-key placeholders.

### Fixed

- **CI release build (Rust compile)** — Resolved 24 compile errors blocking `tauri build`:
  - Wired up `config::post_processing` module (declared + re-exported in `config/mod.rs`) so `LlmProviderConfig` and `PostProcessingProvider` resolve from `commands/config.rs`.
  - Removed 4 dead command registrations in `main.rs` (`get_data_dir`, `get_home_dir`, `get_installer_script_path`, `run_kittentts_installer`) — no definitions, no frontend callers.
  - Added missing `let eff = resolve_effective(&tts_config)` + `voice` bindings in `speak_now` and `speak_queued` (`commands/tts/synthesis.rs`) where `eff`/`voice` were referenced but never bound.

- **CI release build** — Removed stale `bundle.resources` entries in `tauri.conf.json` (`../install-kittentts.ps1`, `../kittentts-cli.py`) that referenced non-existent repo-root files and aborted the bundler. Engine installer scripts are resolved at runtime under `scripts/` via `install_engine`.

## [0.1.5] - 2026-05-20

### Added

- **LLM post-processing (Groq Cloud)** — Optional pass between sanitize and TTS synthesis that rewrites copied text into concise, listener-friendly speech tailored for software developers. Off by default. Configure under Settings → Advanced → LLM Post-Processing.
  - New `PostProcessConfig` (`enabled`, `api_key`, `model`, `prompt`) in `AppConfig`; config schema version bumped to `0.1.5`.
  - New Rust module `post_process` (`process`, `try_process`) wraps Groq's OpenAI-compatible `/chat/completions`.
  - New IPC command `check_groq_credentials` validates the key via `GET /models`.
  - Hooked into `speak_now` and `speak_queued` after the cfg snapshot, before pagination. LLM failures fall back to the original text and never block synthesis.
  - Hardcoded model dropdown: `openai/gpt-oss-20b`, `llama-3.3-70b-versatile`, `llama-3.1-8b-instant`.

### Changed

- **LLM post-processing default prompt** — Switched to a terse caveman-style rewrite prompt with a 3 bullet/point maximum.

### Fixed

- **CopySpeak TTS Pi extension** — Routes final Pi responses through the running app's sanitization, max-length, LLM post-processing, effects, and TTS pipeline instead of filtering/truncating in the extension.
- **Vercel landing page** — Updated the displayed version, screenshot asset, and removed the double-copy hero tagline.

## [0.1.4] - 2026-05-20

### Added

- **CopySpeak TTS Claude Code hook** — Added `scripts/claude-copyspeak-hook.mjs` to speak Claude Code `Stop`/`SubagentStop` assistant responses through the CopySpeak TTS control server.

### Changed

- **CopySpeak TTS Pi extension** — Disabled speaking Pi thinking blocks by default and expanded status text to show only non-default assistant/thinking/activity modes.

### Fixed

- **CopySpeak TTS Pi extension** — Removed the stale `.pi/extensions/copyspeak-voice` extension so only `/copyspeak` is registered.
- **Vercel deployments** — Added a repository `ignoreCommand` that runs production builds and skips preview builds.

## [0.1.3] - 2026-05-19

### Added

- **Update controls in settings** — Added the footer update status/check/install control below the automatic update-check setting.

### Fixed

- **CopySpeak TTS Pi extension** — Renamed the Pi command/extension path to `copyspeak` and shortened its Pi status text to `on`/`off`.
- **Vercel landing page** — Re-enabled non-English locale registration and footer language switching, and restored page scrolling despite the desktop app's global hidden body overflow.
- **Windows audio wake-up** — Add a low-level preroll to desktop playback on Windows so the audio device wakes before speech or radio effects begin.
- **About settings layout** — Removed the stale import/export separator and aligned About rows with the shared `SettingRow` spacing.

## [0.1.2] - 2026-05-18

### Added

- **Audio Effects system** — Frontend-only post-processing applied to TTS playback
  - New `EffectsConfig` (Rust + TS) persisted in `AppConfig` with `enabled` and `active_effect`
  - New Effects settings tab and conditional main-menu Effects tab (gated by `effects.enabled`)
  - New `/effects` route with live effect selector and preview button
  - **Walkie-talkie effect** — Narrow radio EQ, subtle saturation, light AM wobble, normalized PTT clicks, and low static under the voice
  - **8-bit Game Boy effect** — 4-bit sample quantization resampled to 11025 Hz for crunchy retro voice
  - `Effect` interface and registry in `src/lib/stores/playback/effects/` for extensibility
  - Effects render inside `OfflineAudioContext` and integrate with existing pitch-shift pipeline; results cached per `{pitch, effect}` pair

### Changed

- **Unified web and desktop SvelteKit app** — Consolidated the former `src-web` landing page into the main `src` app
  - Added Vercel environment detection via `import.meta.env.VITE_IS_VERCEL`
  - Route layout now renders the marketing landing page on Vercel and the Tauri app shell locally/in desktop builds
  - Removed the redundant `src-web` SvelteKit project

### Fixed

- **CopySpeak TTS Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak TTS control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak TTS Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak TTS Pi extension** — Now speaks only once after an agent run completes and no longer auto-launches CopySpeak TTS unless `COPYSPEAK_PI_LAUNCH=1` is set.
- **CopySpeak TTS Pi extension** — Added a two-minute duplicate speech guard to avoid charging TTS credits for repeated final messages.
- **CopySpeak TTS Pi extension** — Uses the running app's engine/effect settings by default and can include Pi thinking blocks in spoken assistant responses.
- **CopySpeak TTS Pi extension** — Speaks Pi thinking blocks as soon as each thinking block finishes streaming, while avoiding replaying those blocks in the final response.
- **CopySpeak TTS control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.
- **CopySpeak TTS control server** — `/speak` now waits for speech generation to complete before responding, allowing Pi extension requests to queue synthesis instead of overlapping.
- **Playback queue** — Single `audio-ready` events now use the existing fragment queue so Pi-generated thinking and final responses play sequentially instead of interrupting each other.
- **Global playback settings** — Sync playback volume, speed, pitch, and effects during app startup so Pi control-server speech uses the configured walkie-talkie effect outside the Play page.

## [0.1.1] - 2026-05-15

### Added

- **Audio Effects system** — Frontend-only post-processing applied to TTS playback
  - New `EffectsConfig` (Rust + TS) persisted in `AppConfig` with `enabled` and `active_effect`
  - New Effects settings tab and conditional main-menu Effects tab (gated by `effects.enabled`)
  - New `/effects` route with live effect selector and preview button
  - **Walkie-talkie effect** — Narrow radio EQ, subtle saturation, light AM wobble, normalized PTT clicks, and low static under the voice
  - **8-bit Game Boy effect** — 4-bit sample quantization resampled to 11025 Hz for crunchy retro voice
  - `Effect` interface and registry in `src/lib/stores/playback/effects/` for extensibility
  - Effects render inside `OfflineAudioContext` and integrate with existing pitch-shift pipeline; results cached per `{pitch, effect}` pair

- **Cartesia onboarding verification** — Onboarding now accepts a Cartesia API key and validates it via `check_cartesia_credentials` without synthesis.

- **Cartesia TTS backend** — Added Cartesia Sonic 3.5 as a cloud TTS engine
  - Added `CartesiaConfig`, `TtsEngine::Cartesia`, and `CartesiaTtsBackend`
  - Added Cartesia engine settings UI with model, voice ID, and output format controls

### Changed

- **Unified web and desktop SvelteKit app** — Consolidated the former `src-web` landing page into the main `src` app
  - Added Vercel environment detection via `import.meta.env.VITE_IS_VERCEL`
  - Route layout now renders the marketing landing page on Vercel and the Tauri app shell locally/in desktop builds
  - Removed the redundant `src-web` SvelteKit project
- **Default TTS engine** — New configs now default to Cartesia Sonic 3.5 with the Katie voice
- **Default pagination fragment size** — New configs now use `fragment_size: 500`
- **Engine picker order** — Cartesia now appears first in engine settings and footer selector
- **Cartesia voice selection** — Cartesia settings now show resolved voice names with a manual voice ID fallback
- **Onboarding flow** — First-run setup now focuses on Cartesia Cloud instead of local Kitten TTS installation

### Fixed

- **CopySpeak TTS Pi extension** — Switched Pi speech triggering from clipboard double-copy writes to the local CopySpeak TTS control server, avoiding primer speech and Windows clipboard failures.
- **CopySpeak TTS Pi extension** — Disabled activity/tool announcements by default so normal use only speaks final assistant responses unless `/copyspeak activity on` is enabled.
- **CopySpeak TTS control server** — Fixed `Content-Length` parsing so `/speak` accepts normal HTTP POST bodies from Pi, curl, and other clients.

## [0.1.0] - 2026-03-27

### Added

- **Global hotkey speak-from-clipboard** — Hotkey now triggers TTS directly from clipboard content
  - Added handler in global-shortcut plugin to call `speak_from_clipboard` on hotkey press
  - Logs hotkey trigger events for debugging

- **Dedicated History page** — New `/history` route for viewing all TTS generations
  - Moved history from play page to its own route
  - Conditionally shown in nav when history is enabled

- **SettingRow component** — Reusable settings row with label, tooltip, and consistent layout
  - Applied across all settings components for uniform UI

- **Live debug logs viewer** — Real-time log tail in About section when debug mode enabled
  - Shows last 20 lines, auto-refreshes every 2s

### Fixed

- **CopySpeak TTS Pi extension** — Reworked clipboard triggering to serialize double-copy events and avoid repeated trigger loops; startup now avoids focusing an already-running CopySpeak TTS instance.

- **Windows CLI backend PATH resolution** — Expanded PATH for finding Python/uv tools on Windows
  - Added `get_expanded_path()` to include common Python and uv installation paths
  - Fixes "executable not found" errors on clean Windows installations

### Changed

- **Settings page consolidation** — Major restructure from 8 sections to 3 tabs (General, Advanced, About)
  - Continuous scroll with scroll-spy navigation
  - Removed staggered loading (WebView2 crash workaround no longer needed)
  - HUD settings moved to General section as dropdown
  - Pagination/Sanitization moved to Advanced tab
- **Window size increased** — 675x540 → 775x640 for better content visibility
- **Hotkey capture redesign** — Cleaner UI with Kbd components and arrow key symbols (↑↓←→)
- **Quick-settings redesign** — Larger controls with clearer labels (Volume, Speed, Pitch)
- **App shell refactor** — Grid-based layout for better content distribution
- **Removed `show_notifications`** config field — Unused setting cleaned up
- **Default hotkey shortcut** — Changed from `Super+Shift+A` to `Win+Shift+A` for Windows clarity
- **Hotkey error messages** — Updated to use "Win" instead of "Win/Super" for consistency
- **Hotkey logging** — Added structured logging with `[Hotkey]` prefix for registration attempts and config changes
- **Border radius system** — Simplified radius variables for sharper brutalist aesthetic
  - `--radius-sm: 2px`, `--radius-md: var(--radius)`, `--radius-lg: 4px`, `--radius-xl: 6px`
  - Theme toggle and UI components updated to use `rounded-sm` instead of `rounded-none`
- **Logging noise reduction** — Suppressed verbose debug logs from tauri_plugin_updater and reqwest
- **Engine page layout refactor** — Moved badges to header section for cleaner UI
- **Progress bar animation** — Converted from JavaScript interval to CSS animation for smoother performance
- **Default Kokoro voice** — Changed from `af_heart` to `adam`
- **Internationalization** — Temporarily disabled language switcher, hardcoded to English during development

## [0.0.5] - 2026-03-24

### Added

- **Global hotkey configuration** — Configurable keyboard shortcut to trigger TTS
  - `hotkey` config field with modifier + key format (e.g., `"Ctrl+Space"`)
  - Hotkey capture component in settings UI
  - Backend IPC: `register_hotkey` with global-shortcut plugin
  - Hotkey re-registration on config change

- **Listening toggle** — Enable/disable clipboard monitoring via `listen_enabled` config
  - Toggle in quick-settings dropdown and app-footer
  - Backend IPC: `set_listening`, `get_listening` commands
  - Persisted to config, synced via `config-changed` event

### Fixed

- **HUD progress bar and marquee timing** — Accurate playback duration via cross-window event
  - HUD window and main window have separate JS contexts with separate `hudStore` instances
  - `playbackStore` in main window decodes audio via Web Audio API to get accurate duration
  - Emits `hud:audio-duration` event which HUD window receives and updates its `hudStore`
  - Progress now shows accurate percentage based on `AudioBuffer.duration`
  - Marquee animation timing now matches actual playback duration
  - ElevenLabs MP3 duration now accurately determined via Web Audio decode (not server estimate)

- **Audio playback on clean Windows 11** — AudioContext now resumes if suspended
  - Web Audio API requires user gesture to activate AudioContext on fresh profiles
  - Added `audioCtx.resume()` call when state is "suspended" in playback-store

## [0.0.3] - 2026-03-22

### Fixed

- **KittenTTS installer** now works on clean Windows 11 without Python pre-installed
  - Embeds installer scripts in binary and extracts to temp directory at runtime
  - Auto-detects any Python 3.x version, offers winget installation if not found
  - PowerShell window now visible with success/failure feedback before pause
  - Default config now uses `py -3.12` to ensure kittentts runs on same Python version used by installer
  - Health check detects `ModuleNotFoundError` with actionable error message
  - Fixed health check using invalid voice "test" instead of "Rosie"

## [0.0.2] - 2026-03-21

### Added

- **HUD playback enhancements**
  - Progress bar animation synced to audio duration
  - Marquee scrolling text for long speech content
  - `duration_ms` field in `HudSynthesizingPayload` for synthesis duration tracking

### Fixed

- Removed duplicate `$effect` in hud-playback-content component
- Removed debug `console.log` statement from production code

## [0.0.1] - 2026-03-20

### Added

- **Core TTS functionality** — Clipboard-triggered text-to-speech with multiple engine support
  - Double-copy trigger: copy twice within 1.5s to speak selected text
  - Hotkey trigger: configurable keyboard shortcut
  - Manual trigger: paste/play from UI

- **Multiple TTS engines**
  - **Kitten TTS** (default): Ultra-lightweight CPU-optimized ONNX inference, 8 built-in voices
  - **Piper TTS**: Local CLI engine with 20+ EN US voices
  - **Kokoro TTS**: Local CLI engine with multiple voices
  - **OpenAI TTS**: Cloud API with 9 voices (alloy, ash, coral, echo, fable, onyx, nova, shimmer, verse)
  - **ElevenLabs TTS**: Cloud API with voice library support

- **HUD overlay** — Floating heads-up display showing playback status, waveform visualization, and engine info
  - Real-time waveform visualization with 16-bar equalizer
  - Progress tracking for paginated synthesis
  - Click-through transparent overlay

- **History management** — Persistent history of TTS generations with playback
  - Audio files saved in native format (WAV/MP3/OGG/FLAC)
  - Fragmented copy grouping for paginated text
  - Batch playback and deletion

- **Settings system**
  - General: auto-start, debug mode, language (EN/ES with full i18n support)
  - Playback: speed (0.25x–4x), pitch (0.5x–2x), volume
  - Triggers: double-copy window, hotkey configuration
  - Sanitization: markdown stripping, text normalization

- **Auto-updater** — Check and install updates from GitHub Releases

- **Internationalization (i18n)** — Full localization with English and Spanish support, RTL layout ready

### Breaking Changes

- **HTTP TTS engine removed** — HTTP endpoint backend removed in favor of CLI and cloud engines
- **SSML support removed** — SSML markup passthrough feature removed
- **Streaming TTS mode removed** — Simplified to paginated synthesis only

[Unreleased]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.15...HEAD
[0.1.16]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.15...v0.1.16
[0.1.15]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.14...v0.1.15
[0.1.14]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ilyaizen/CopySpeak/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.5...v0.1.0
[0.0.5]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.3...v0.0.5
[0.0.3]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/ilyaizen/CopySpeak/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/ilyaizen/CopySpeak/releases/tag/v0.0.1
