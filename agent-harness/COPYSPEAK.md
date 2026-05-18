# CopySpeak CLI Harness SOP

## Analysis

CopySpeak is a Tauri v2 Windows desktop application. The GUI is Svelte 5; the backend is Rust under `src-tauri/src`. GUI actions invoke Tauri commands in `src-tauri/src/commands/*` for config, queue, playback, history, TTS credentials/voices/health/synthesis, and updates.

Native persistent data is JSON-like app config/history managed by the Rust backend. The harness uses a JSON project file (`cli-anything-copyspeak/v1`) as agent-editable state and invokes real CopySpeak/TTS backends for audio synthesis.

## Backend rule

The harness does not synthesize audio in Python. Export calls a real backend subprocess: local `kittentts-cli.py` for Kitten TTS or installed `piper`/`kokoro`-style CLIs. Launch/check commands locate the real CopySpeak executable via `COPYSPEAK_EXE`, PATH, or `src-tauri/target/{release,debug}/copyspeak.exe`.

## CLI design

- `project new/info/set-config`: create and inspect project JSON.
- `queue add/list/remove/clear`: manage text to synthesize.
- `export text/queue`: call real TTS backend and verify audio magic bytes/frames.
- `backend check/launch`: inspect or start real CopySpeak.
- default command enters REPL with unified `ReplSkin`.

All commands support top-level `--json`. Mutating one-shot commands auto-save unless `--dry-run` is set.
