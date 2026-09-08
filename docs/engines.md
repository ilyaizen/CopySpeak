# CopySpeak Engines

CopySpeak is a TTS **orchestrator**: it drives engines you choose, it does not
bundle one. Pick an engine, set it up once on the **Engine** page (API key or
local install), then create **Profiles** that bundle engine + voice + speed +
pitch + effect as one swappable unit.

> **Boundary:** the Engine page holds **credentials and setup only** (API keys,
> endpoint, install, setup test). **Voices, models, and per-engine knobs live
> in Profiles.** See [`profile-engine-settings.md`](./profile-engine-settings.md).

## Engine matrix

| Engine             | Type   | API key        | Offline | Installer                | Setup test               |
| ------------------ | ------ | -------------- | ------- | ------------------------ | ------------------------ |
| Edge-TTS           | cloud  | no             | no      | — (`uv tool install edge-tts`) | `test_tts_engine_config` |
| Cartesia (Sonic)   | cloud  | yes            | no      | —                        | `test_tts_engine_config` |
| ElevenLabs         | cloud  | yes            | no      | —                        | `test_tts_engine_config` |
| OpenAI             | cloud  | yes            | no      | —                        | `test_tts_engine_config` |
| Google Gemini TTS  | cloud  | yes            | no      | —                        | `test_tts_engine_config` |
| Microsoft / Azure  | cloud  | yes + endpoint | no      | —                        | `test_tts_engine_config` |
| Kitten TTS         | local  | no             | yes     | `install-kittentts.ps1`  | installer smoke test     |
| Piper (piper1-gpl) | local  | no             | yes     | `install-piper.ps1`      | installer smoke test     |
| Kokoro TTS         | local  | no             | yes     | `install-kokoro.ps1`     | installer smoke test     |
| Pocket TTS         | local  | no             | yes     | `install-pocket.ps1`     | installer smoke test     |
| Chatterbox         | local  | no             | yes     | `install-chatterbox.ps1` | installer smoke test     |
| HTTP server        | either | varies         | varies  | — (configure in profile) | —                        |

## Cloud engines

Each cloud tab on the Engine page takes an API key (Microsoft also takes an
endpoint). Click **Test Setup** to synthesize a short clip with the engine's
default voice and confirm the credential works. Edge-TTS is the exception:
it needs no key, but its CLI must be on PATH first
(`uv tool install edge-tts`).

| Engine            | Where to get credentials                      |
| ----------------- | --------------------------------------------- |
| OpenAI            | <https://platform.openai.com/api-keys>        |
| ElevenLabs        | <https://elevenlabs.io/app/settings/api-keys> |
| Cartesia          | <https://cartesia.ai/console>                 |
| Google Gemini TTS | <https://aistudio.google.com/app/apikey>      |
| Microsoft / Azure | Azure AI Foundry deployment key + endpoint    |

Voices, models, formats, and provider knobs (stability, similarity, etc.) are
chosen per profile — they are not set on the Engine page.

## Local engines (uv-based installers)

All local engines are managed by [`uv`](https://docs.astral.sh/uv/). The
**Install** button on each local engine tab opens a PowerShell window that runs
the matching installer automatically. If `uv` is missing, the Engine page shows
an **Install uv** button first.

Engines install into `%LOCALAPPDATA%\CopySpeak\engines\<engine>`. Each installer:

1. Requires `uv` (fails fast with a pointer to `install-uv.ps1` otherwise).
2. Creates a uv project (`uv tool install` for console-script engines, a uv
   project + wrapper for module-based engines).
3. Optionally runs a smoke test (`-SmokeTest`).
4. Prints a ready-to-paste profile snippet.

Common flags: `-Force` (reinstall), `-SmokeTest` (synthesize one clip),
`-Cuda` (GPU acceleration, see below).

| Engine     | Installer                | Size        | Notes                                                     |
| ---------- | ------------------------ | ----------- | --------------------------------------------------------- |
| Kitten TTS | `install-kittentts.ps1`  | 25-80MB     | 8 voices, CPU ONNX. Model downloads on first use.         |
| Piper      | `install-piper.ps1`      | ~60MB/voice | Drop `.onnx` + `.onnx.json` into `engines/piper/voices/`. |
| Kokoro TTS | `install-kokoro.ps1`     | ~335MB      | Natural voices, broad accent coverage. Shared model.      |
| Pocket TTS | `install-pocket.ps1`     | 100M params | Kyutai Pocket; 14 voices in 6 languages. CPU-first.       |
| Chatterbox | `install-chatterbox.ps1` | ~2GB        | Zero-shot + emotion control; optional voice clone wavs.   |

(Edge-TTS is uv-managed too but has no installer script — install its CLI
with `uv tool install edge-tts`.)

Manual run (if you prefer the terminal):

```powershell
./scripts/install-uv.ps1            # one-time uv bootstrap
uv tool install edge-tts            # edge-tts CLI (no app installer)
./scripts/test-engine.ps1 -Engine chatterbox   # verify any installed engine
```

### Resident models

Every local engine keeps its model in RAM between utterances. CopySpeak starts
one wrapper process per engine in `--serve` mode at launch and talks to it over
a pipe. Each wrapper also synthesizes a throwaway phrase before reporting ready,
so the one-off costs of a first call (phonemizer init, ONNX graph warmup) land
during background startup instead of on your first utterance. Piper additionally
streams sentence by sentence, so audio starts before the passage is finished.

Switching voice, engine, or the GPU toggle restarts that engine's process: the
utterance that triggered the switch runs one-shot, and the next one is warm.
Nothing is left running after CopySpeak quits.

If an engine was installed before this existed, its wrapper is out of date and
CopySpeak silently stays on the slower one-shot path. Re-run its installer with
`-Force` to pick up the new wrapper.

### GPU acceleration

Off by default. Two steps, both required:

1. Re-run the installer with `-Cuda`. It adds the GPU runtime to that engine's
   own project (`onnxruntime-gpu` plus the NVIDIA CUDA/cuDNN wheels; a CUDA
   build of torch for Pocket) and then proves it by synthesizing a clip on the
   GPU. Nothing is installed system-wide.
2. Turn on **GPU acceleration** in that engine's profile settings.

Needs an NVIDIA GPU and a current driver. If the GPU is unavailable at
synthesis time the engine says so in the log rather than quietly using the CPU.

Pocket is the exception worth knowing about: it is designed for CPU, and on a
fast laptop chip the GPU is no quicker. It pays off on thread-limited machines.

## HTTP servers

Any TTS engine that exposes an HTTP API (OpenAI-compatible or custom) is
configured **per profile**: URL template, method, headers, body template,
voice, response format, timeout. See
[`profile-engine-settings.md`](./profile-engine-settings.md#http-and-cli-semantics)
for the placeholder tokens (`{text}`, `{raw_text}`, `{voice}`, `{speed}`).

## Adding a new engine

Backend steps live in [`docs_internal/tts_backends.md`](../docs_internal/tts_backends.md):
implement `TtsBackend`, register in `tts/mod.rs`, add a catalog entry, and (for
local engines) add an installer under `scripts/` plus a tab in the Engine page
registry (`ENGINE_TABS` in `engine-page.svelte`).
