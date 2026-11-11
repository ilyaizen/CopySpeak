# Local-engine bench (Stage 0)

Measures TTFA, RAM, and VRAM for CopySpeak's five local daemon engines
(Kitten, Qwen, Piper, Kokoro, Pocket) by speaking the daemon protocol v2
directly — no app code runs, no product code changes.

## Definitions

- **TTFA** — time from writing the request line to the first PCM payload byte
  read from the wrapper (concept-brief assumption 4).
- **load** — process spawn to the `READY 2` handshake. Includes the wrapper's
  pre-handshake warm-up (Qwen does not warm up; its first request pays that).
- **cold TTFA** — the first request after `READY 2`. Captures per-call first
  costs that the handshake already absorbed for the other engines.
- **warm TTFA** — median over N timed requests. Request 2 after `READY 2` is
  discarded as settle.
- **full** — request write to the `{"end":true}` frame.
- **RAM** — summed working set (RSS) of the daemon process tree (`uv` +
  `python`), sampled every 200 ms.
- **VRAM** — device-wide GPU memory in use (NVML), reported as a delta against
  the pre-spawn baseline.

## Usage (repo root)

```sh
uv run --project scripts/bench python scripts/bench/bench.py
uv run --project scripts/bench python scripts/bench/bench.py --engines piper --runs 3
uv run --project scripts/bench python scripts/bench/bench.py --engines kitten --devices cuda
```

Outputs land in `docs_internal/egm/baseline-<timestamp>/` (git-ignored):
`results.json`, `summary.md`, and one stderr log per run.

## Notes

- The daemon command replicates `CliTtsBackend::serve_args` exactly
  (`src-tauri/src/tts/cli.rs` + `first_class_local_cli` in
  `src-tauri/src/commands/tts/helpers.rs`), with the default-profile shape:
  Kitten sends no `--model`/`--speed` (wrapper defaults: nano-0.8, Rosie),
  Qwen sends `--model Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice` + `Aiden`,
  Piper `en_US-amy-medium`, Kokoro `af_heart`, Pocket `alba`.
- `--device cuda` mirrors the app's GPU toggle. The `providers:` line from the
  wrapper's stderr is captured per run; a CUDA request whose providers line
  lacks `CUDAExecutionProvider` is flagged `silent_cpu_fallback` in the
  summary (concept-brief finding 8).
- Engines whose install dir is missing are reported as `engine-not-installed`.
- Effects are an app-layer concept; wrappers have none, so every run is
  "effects off" by construction.
