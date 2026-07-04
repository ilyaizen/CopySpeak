# CopySpeak Engines

CopySpeak is a TTS **orchestrator**: it drives engines you choose, it does not
bundle one. Pick an engine, set it up once on the **Engine** page (API key or
local install), then create **Profiles** that bundle engine + voice + speed +
pitch + effect as one swappable unit.

> **Boundary:** the Engine page holds **credentials and setup only** (API keys,
> endpoint, install, setup test). **Voices, models, and per-engine knobs live
> in Profiles.** See [`profile-engine-settings.md`](./profile-engine-settings.md).

## Engine matrix

| Engine              | Type   | API key | Offline | Installer                       | Setup test                  |
| ------------------- | ------ | ------- | ------- | ------------------------------- | --------------------------- |
| Edge-TTS            | cloud  | no      | no      | `install-edge-tts.ps1`          | `test_tts_engine_config`    |
| Cartesia (Sonic)    | cloud  | yes     | no      | —                               | `test_tts_engine_config`    |
| ElevenLabs          | cloud  | yes     | no      | —                               | `test_tts_engine_config`    |
| OpenAI              | cloud  | yes     | no      | —                               | `test_tts_engine_config`    |
| Google Gemini TTS   | cloud  | yes     | no      | —                               | `test_tts_engine_config`    |
| Microsoft / Azure   | cloud  | yes + endpoint | no | —                              | `test_tts_engine_config`    |
| Kitten TTS          | local  | no      | yes     | `install-kittentts.ps1`         | installer smoke test        |
| Piper (piper1-gpl)  | local  | no      | yes     | `install-piper.ps1`             | installer smoke test        |
| Kokoro TTS          | local  | no      | yes     | `install-kokoro.ps1`            | installer smoke test        |
| Pocket TTS          | local  | no      | yes     | `install-pocket.ps1`            | installer smoke test        |
| Chatterbox          | local  | no      | yes     | `install-chatterbox.ps1`        | installer smoke test        |
| HTTP server         | either | varies  | varies  | — (configure in profile)        | —                           |

## Cloud engines

Each cloud tab on the Engine page takes an API key (Microsoft also takes an
endpoint). Click **Test Setup** to synthesize a short clip with the engine's
default voice and confirm the credential works.

| Engine            | Where to get credentials                                                                 |
| ----------------- | --------------------------------------------------------------------------------------- |
| OpenAI            | <https://platform.openai.com/api-keys>                                                  |
| ElevenLabs        | <https://elevenlabs.io/app/settings/api-keys>                                           |
| Cartesia          | <https://cartesia.ai/console>                                                           |
| Google Gemini TTS | <https://aistudio.google.com/app/apikey>                                                |
| Microsoft / Azure | Azure AI Foundry deployment key + endpoint                                              |

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

Common flags: `-Force` (reinstall), `-SmokeTest` (synthesize one clip).

| Engine        | Installer                | Size     | Notes                                                       |
| ------------- | ------------------------ | -------- | ----------------------------------------------------------- |
| Edge-TTS      | `install-edge-tts.ps1`   | tiny     | Free Microsoft Read Aloud; no model download.               |
| Kitten TTS    | `install-kittentts.ps1`  | 25-80MB  | 8 voices, CPU ONNX. Model downloads on first use.           |
| Piper         | `install-piper.ps1`      | ~60MB/voice | Drop `.onnx` + `.onnx.json` into `engines/piper/voices/`. |
| Kokoro TTS    | `install-kokoro.ps1`     | ~500MB   | Natural voices, broad accent coverage.                      |
| Pocket TTS    | `install-pocket.ps1`     | compact  | Straightforward CLI voice selection.                        |
| Chatterbox    | `install-chatterbox.ps1` | ~2GB     | Zero-shot + emotion control; optional voice clone wavs.     |

Manual run (if you prefer the terminal):

```powershell
./scripts/install-uv.ps1            # one-time uv bootstrap
./scripts/install-edge-tts.ps1 -SmokeTest
./scripts/test-engine.ps1 -Engine chatterbox   # verify any installed engine
```

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
