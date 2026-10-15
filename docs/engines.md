# CopySpeak Engines

CopySpeak is a TTS **orchestrator**: it drives engines you choose, it does not
bundle one. Pick an engine, set it up once on the **Engine** page (API key or
local install), then create **Profiles** that bundle engine + voice + speed +
pitch + effect as one swappable unit.

> **Boundary:** the Engine page holds **credentials and setup only** (API keys,
> endpoint, install, setup test). **Voices, models, and per-engine knobs live
> in Profiles.** See [`profile-engine-settings.md`](./profile-engine-settings.md).

## Engine matrix

| Engine             | Type   | API key        | Offline | Live captions | Installer                       | Setup test               |
| ------------------ | ------ | -------------- | ------- | ------------- | ------------------------------- | ------------------------ |
| Cartesia (Sonic)   | cloud  | yes            | no      | yes           | —                               | `test_tts_engine_config` |
| ElevenLabs         | cloud  | yes            | no      | yes           | —                               | `test_tts_engine_config` |
| OpenAI             | cloud  | yes            | no      | no            | —                               | `test_tts_engine_config` |
| Google Gemini TTS  | cloud  | yes            | no      | no            | —                               | `test_tts_engine_config` |
| Microsoft / Azure  | cloud  | yes + endpoint | no      | no            | —                               | `test_tts_engine_config` |
| Kitten TTS         | local  | no             | yes     | yes           | `install-kittentts.ps1` / `.sh` | installer smoke test     |
| Piper (piper1-gpl) | local  | no             | yes     | yes           | `install-piper.ps1` / `.sh`     | installer smoke test     |
| Kokoro TTS         | local  | no             | yes     | yes           | `install-kokoro.ps1` / `.sh`    | installer smoke test     |
| Pocket TTS         | local  | no             | yes     | no            | `install-pocket.ps1` / `.sh`    | installer smoke test     |
| Qwen3-TTS          | local  | no             | yes     | yes           | `install-qwen.ps1` / `.sh`      | installer smoke test     |
| HTTP server        | either | varies         | varies  | no            | — (configure in profile)        | —                        |

## Cloud engines

Each cloud tab on the Engine page takes an API key (Microsoft also takes an
endpoint). Click **Test Setup** to synthesize a short clip with the engine's
default voice and confirm the credential works.

> Edge-TTS was removed from the app in 2026-10: Microsoft killed the free
> Read-Aloud protocol the `edge-tts` CLI speaks
> ([rany2/edge-tts#490](https://github.com/rany2/edge-tts/issues/490)). It comes
> back when upstream ships a fixed release.

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
**Install** button on each local engine tab opens a terminal window (PowerShell
on Windows, a shell script on Linux) that runs the matching installer
automatically — every installer ships as `.ps1` and `.sh`. If `uv` is missing,
the Engine page shows an **Install uv** button first.

Engines install into `%LOCALAPPDATA%\CopySpeak\engines\<engine>`. Each installer:

1. Requires `uv` (fails fast with a pointer to `install-uv.ps1` otherwise).
2. Creates a uv project (`uv tool install` for console-script engines, a uv
   project + wrapper for module-based engines).
3. Optionally runs a smoke test (`-SmokeTest`).
4. Prints a ready-to-paste profile snippet.

Common flags: `-Force` (reinstall), `-SmokeTest` (synthesize one clip),
`-Cuda` (GPU acceleration, see below).

| Engine     | Installer               | Size        | Notes                                                          |
| ---------- | ----------------------- | ----------- | -------------------------------------------------------------- |
| Kitten TTS | `install-kittentts.ps1` | 25-80MB     | 8 voices, CPU ONNX. Model downloads on first use.              |
| Piper      | `install-piper.ps1`     | ~60MB/voice | Drop `.onnx` + `.onnx.json` into `engines/piper/voices/`.      |
| Kokoro TTS | `install-kokoro.ps1`    | ~335MB      | Natural voices, broad accent coverage. Shared model.           |
| Pocket TTS | `install-pocket.ps1`    | 100M params | Kyutai Pocket; 14 voices in 6 languages. CPU-first.            |
| Qwen3-TTS  | `install-qwen.ps1`      | —           | Multilingual; 9 built-in voices; model downloads on first use. |

(Edge-TTS is uv-managed too but has no installer script — install its CLI
with `uv tool install edge-tts`.)

> Edge-TTS is currently removed from the app (upstream service dead, 2026-10);
> the note above applies only if/when it returns.

Manual run (if you prefer the terminal):

```powershell
./scripts/install-uv.ps1            # one-time uv bootstrap
uv tool install edge-tts            # edge-tts CLI (no app installer)
./scripts/test-engine.ps1 -Engine pocket       # verify any installed engine
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

## Languages

A profile's `language` tag decides when auto-switching picks it (see
[`adr/0001-multilingual-auto-switch.md`](./adr/0001-multilingual-auto-switch.md)).
The detected language is sent to an engine only where its API takes one.
Everything else gets the text alone.

| Engine            | Language sent to the engine                                  | Non-English voices                                            |
| ----------------- | ------------------------------------------------------------ | ------------------------------------------------------------- |
| Cartesia (Sonic)  | `language`, only the 44 Sonic 3.6 base codes; `nb` → `no`   | From the voice's language tag                                 |
| ElevenLabs        | `language_code`; `nb` → `no`; skipped on `multilingual_v2`   | Per voice, from the voice's `language` label                  |
| OpenAI            | not sent                                                     | Not verified                                                  |
| Google Gemini TTS | not sent                                                     | Not verified                                                  |
| Microsoft / Azure | not sent                                                     | Not verified                                                  |
| Kitten TTS        | not sent                                                     | English only                                                  |
| Piper             | not sent                                                     | Installer is `en_US` only; any-language installs are planned |
| Kokoro TTS        | not sent                                                     | English voices only; other voices are rejected                |
| Pocket TTS        | not sent                                                     | fr, es, it, de, pt load their own language model              |
| Qwen3-TTS         | not sent (the wrapper uses `language="Auto"`)                | Catalog includes zh, ja and ko voices                         |
| HTTP server       | not sent                                                     | Depends on the server                                         |

Cartesia probe (2026-10-15, `Cartesia-Version: 2024-06-10`): `language: "fr"` returns 200, an unsupported code such as `xx` returns 400 `unsupported language`. An omitted `language` also returns 200; whether it defaults to English was not checked.

Pocket's non-English voices need a reinstall: a wrapper copy alone does not
bring in the `pocket-tts` version floor.

## HTTP servers

Any TTS engine that exposes an HTTP API (OpenAI-compatible or custom) is
configured **per profile**: URL template, method, headers, body template,
voice, response format, timeout. See
[`profile-engine-settings.md`](./profile-engine-settings.md#http-and-cli-semantics)
for the placeholder tokens (`{text}`, `{raw_text}`, `{voice}`, `{speed}`).

## Adding a new engine

1. Implement `TtsBackend` (`src-tauri/src/tts/mod.rs`) and register the module there.
2. Add a `TtsEngine` variant (`src-tauri/src/config/tts.rs`) and a catalog entry
   (`src-tauri/src/tts/catalog.rs`).
3. Add an Engines page setup entry (`CLOUD_ENGINES` or `LOCAL_ENGINES` in
   `src/lib/components/engine/engine-meta.ts`).
4. Local engines: add `install-*.ps1` / `install-*.sh` under `scripts/`.

Known gotchas (test fixtures, installer bundling, wrapper backports) are in the
Engines section of [`agents/failure-log.md`](./agents/failure-log.md#engines).
