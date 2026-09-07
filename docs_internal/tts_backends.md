# TTS Backend Integration Guide

> **Last Updated:** 2026-02-26
> **Purpose:** Reference for supported TTS engines and adding new backends

---

## Table of Contents

- [TTS Backend Integration Guide](#tts-backend-integration-guide)
  - [Table of Contents](#table-of-contents)
  - [Overview](#overview)
  - [Supported Backend Types](#supported-backend-types)
    - [1. CLI Backend (Default)](#1-cli-backend-default)
    - [2. HTTP Server Backend](#2-http-server-backend)
  - [Preset Configurations](#preset-configurations)
    - [Piper (Default Preset)](#piper-default-preset)
    - [kokoro-tts](#kokoro-tts)
    - [Chatterbox](#chatterbox)
    - [Coqui TTS / XTTS-v2](#coqui-tts--xtts-v2)
    - [eSpeak-ng](#espeak-ng)
    - [Edge TTS (Microsoft)](#edge-tts-microsoft)
    - [ElevenLabs (Cloud)](#elevenlabs-cloud)
  - [Backend Trait Interface](#backend-trait-interface)
    - [Error Types](#error-types)
    - [Streaming](#streaming)
    - [Local daemon protocol (v2)](#local-daemon-protocol-v2)
    - [GPU acceleration](#gpu-acceleration)
  - [Adding a New Backend](#adding-a-new-backend)
    - [Step 1: Create Backend Module](#step-1-create-backend-module)
    - [Step 2: Register in mod.rs](#step-2-register-in-modrs)
    - [Step 3: Add Preset Configuration](#step-3-add-preset-configuration)
    - [Step 4: Document in This File](#step-4-document-in-this-file)
  - [Troubleshooting](#troubleshooting)
    - [Common Issues](#common-issues)
    - [Testing a Backend](#testing-a-backend)
    - [Health Check](#health-check)
  - [Performance Comparison](#performance-comparison)
  - [Local HTTP Server Backend](#local-http-server-backend)
    - [Kokoro Local Server](#kokoro-local-server)
    - [Fish Speech 1.5](#fish-speech-15)
    - [Coqui TTS Server](#coqui-tts-server)
    - [Chatterbox Server](#chatterbox-server)
    - [Generic OpenAI-Compatible TTS](#generic-openai-compatible-tts)

---

## Overview

CopySpeak is designed as a TTS **orchestrator** - it doesn't bundle its own TTS engine. Instead, users install their preferred TTS engine, and CopySpeak communicates with it via a configurable interface.

---

## Supported Backend Types

### 1. CLI Backend (Default)

The CLI backend spawns an external process to synthesize speech.

### 2. HTTP Server Backend

The HTTP backend sends requests to a local (or remote) HTTP TTS server.

**How it works:**

1. CopySpeak builds a request from configurable URL and body templates
2. Sends an HTTP request to the TTS server
3. Reads the audio bytes from the response
4. Plays the audio

**Configuration:**

```json
{
  "tts": { "active_backend": "http" },
  "http_tts": {
    "url_template": "http://localhost:8880/v1/audio/speech",
    "headers": [["Content-Type", "application/json"]],
    "body_template": "{\"model\":\"kokoro\",\"input\":\"{text}\",\"voice\":\"{voice}\",\"response_format\":\"wav\"}",
    "response_format": "wav",
    "timeout_secs": 30
  }
}
```

**Placeholder Tokens:**

| Token     | Description               |
| --------- | ------------------------- |
| `{text}`  | The text to synthesize    |
| `{voice}` | Selected voice identifier |
| `{speed}` | Playback speed multiplier |

**How it works:**

1. CopySpeak writes text to a temp file or passes as argument
2. Calls the TTS command with templated arguments
3. Reads the output WAV file
4. Plays audio and cleans up temp files

**Configuration:**

```json
{
  "tts": {
    "preset": "custom",
    "command": "my-tts-engine",
    "args_template": "--text \"{text}\" --output \"{output}\" --voice \"{voice}\"",
    "voice": "en-us"
  }
}
```

**Placeholder Tokens:**

| Token        | Description                         |
| ------------ | ----------------------------------- |
| `{text}`     | The text to synthesize              |
| `{output}`   | Path to output WAV file             |
| `{voice}`    | Selected voice identifier           |
| `{data_dir}` | CopySpeak config directory          |
| `{raw_text}` | Actual text content (not file path) |

---

## Preset Configurations

### Kitten TTS (Default Preset)

[Kitten TTS](https://github.com/KittenML/KittenTTS) is an ultra-lightweight TTS engine (25-80MB) that runs on CPU without requiring a GPU.

**Features:**

- **Ultra-lightweight** — Model sizes from 25 MB (int8) to 80 MB
- **CPU-optimized** — ONNX-based inference runs efficiently without a GPU
- **8 built-in voices** — Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo
- **24 kHz output** — High-quality audio at a standard sample rate
- **Apache 2.0 license** — Fully open source

**Installation:**

Run the PowerShell installer from the project root:

```powershell
./install-kittentts.ps1
```

Or manually:

```bash
pip install https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl
pip install soundfile
```

**Preset Configuration** (auto-applied when "Kitten TTS" preset is selected):

```json
{
  "tts": {
    "preset": "kitten-tts",
    "command": "python3",
    "args_template": [
      "{home_dir}/kittentts/kittentts-cli.py",
      "--text",
      "{raw_text}",
      "--voice",
      "{voice}",
      "--output",
      "{output}"
    ],
    "voice": "Jasper"
  }
}
```

**Available voices:**

- `Jasper` (default) — Natural male voice
- `Bella` — Warm female voice
- `Luna` — Soft female voice
- `Bruno` — Deep male voice
- `Rosie` — Cheerful female voice
- `Hugo` — Clear male voice
- `Kiki` — Playful female voice
- `Leo` — Neutral male voice

**Model variants:**

| Model                  | Parameters | Size  | Quality    |
| ---------------------- | ---------- | ----- | ---------- |
| `kitten-tts-nano-0.8`  | 15M        | 25 MB | Fast, good |
| `kitten-tts-micro-0.8` | 40M        | 41 MB | Balanced   |
| `kitten-tts-mini-0.8`  | 80M        | 80 MB | Highest    |

Default model is `nano` (fastest, smallest). Change via `--model` flag in CLI.

**Notes:**

- Models are downloaded automatically on first use from Hugging Face Hub
- First synthesis will be slower as the model downloads (~25-80MB depending on variant)
- Subsequent syntheses are fast as the model is cached locally
- Playback speed is controlled via browser frontend playback rate (not at TTS generation level)

---

### Piper

[Piper](https://github.com/OHF-Voice/piper1-gpl) (piper1-gpl) is a fast, offline neural TTS engine with many EN US voices.

**Installation:**

```bash
pip install piper
```

**Model download** — place `.onnx` + `.onnx.json` files in `%APPDATA%\CopySpeak\`:

```bash
# Example: download joe-medium
python3 -m piper.download_voices en_US-joe-medium
# Then move the files into %APPDATA%\CopySpeak\
```

**Preset Configuration** (auto-applied when "Piper TTS" preset is selected):

```json
{
  "tts": {
    "preset": "piper",
    "command": "python3",
    "args_template": [
      "-m",
      "piper",
      "--data-dir",
      "{data_dir}",
      "-m",
      "{voice}",
      "-f",
      "{output}",
      "--input-file",
      "{input}"
    ],
    "voice": "en_US-joe-medium",
    "speed": 1.0
  }
}
```

**Placeholder tokens:**

- `{data_dir}` — resolves automatically to `%APPDATA%\CopySpeak` (where models are stored)
- `{voice}` — model name, e.g. `en_US-joe-medium`
- `{output}` — temp WAV output path
- `{input}` — temp text input file path

**Available EN US voices (medium quality):**
`amy`, `arctic`, `bryce`, `danny`, `hfc_female`, `hfc_male`, `joe` (default),
`john`, `kathleen`, `kristin`, `kusal`, `l2arctic`, `lessac`, `libritts`,
`libritts_r`, `ljspeech`, `norman`, `reza_ibrahim`, `ryan`, `sam`

**Notes:**

- On Windows you may need `python` instead of `python3` depending on your Python installation
- Each voice requires its own `.onnx` + `.onnx.json` pair in `%APPDATA%\CopySpeak\`
- Playback speed and pitch are controlled via browser frontend playback rate (not at TTS generation level)

---

### kokoro-tts

[Kokoro TTS](https://github.com/hexgrad/kokoro) is a fast, high-quality local TTS engine.

**Installation:**

```bash
# Install via pip
pip install kokoro-tts

# Or download standalone binary
```

**Preset Configuration:**

```json
{
  "tts": {
    "preset": "kokoro",
    "command": "kokoro-tts",
    "args_template": "--text \"{text}\" --output \"{output}\" --voice \"{voice}\"",
    "voice": "af_nicole"
  }
}
```

**Available Voices:**

- `af_nicole` - American Female
- `af_sky` - American Female (younger)
- `am_adam` - American Male
- `am_michael` - American Male (older)
- `bf_emma` - British Female
- `bm_george` - British Male

---

---

### Chatterbox

[Chatterbox](https://github.com/resemble-ai/chatterbox) is an open-source, zero-shot TTS with emotion control.

**Installation:**

```bash
pip install chatterbox-tts
```

**Preset Configuration:**

```json
{
  "tts": {
    "preset": "chatterbox",
    "command": "chatterbox-tts",
    "args_template": ["--text", "{input}", "--output", "{output}", "--voice", "{voice}"],
    "voice": "default"
  }
}
```

---

### Coqui TTS / XTTS-v2

[Coqui TTS](https://github.com/coqui-ai/TTS) provides state-of-the-art neural TTS including the XTTS-v2 multilingual model.

**Installation:**

```bash
pip install TTS
```

**Preset Configuration:**

```json
{
  "tts": {
    "preset": "coqui-tts",
    "command": "tts",
    "args_template": ["--text", "{input}", "--out_path", "{output}", "--model_name", "{voice}"],
    "voice": "tts_models/en/ljspeech/tacotron2-DDC"
  }
}
```

**Note:** Replace `{voice}` with the Coqui model name. For XTTS-v2, use `tts_models/multilingual/multi-dataset/xtts_v2`.

---

### eSpeak-ng

[eSpeak-ng](https://github.com/espeak-ng/espeak-ng) is a compact, open-source speech synthesizer.

**Installation:**

```bash
# Windows: Download from releases
# Add to PATH
```

**Configuration:**

```json
{
  "tts": {
    "preset": "espeak",
    "command": "espeak-ng",
    "args_template": "-w \"{output}\" -v {voice} \"{text}\"",
    "voice": "en-us"
  }
}
```

---

### Edge TTS (Microsoft)

[Edge TTS](https://github.com/rany2/edge-tts) uses Microsoft's online TTS service.

**Installation:**

```bash
uv tool install edge-tts
```

**Configuration:**

```json
{
  "tts": {
    "preset": "edge-tts",
    "command": "edge-tts",
    "args_template": "--text \"{text}\" --write-media \"{output}\" --voice {voice}",
    "voice": "en-US-AriaNeural"
  }
}
```

**Note:** Requires internet connection. Edge TTS outputs MP3 by default - use `--write-media` for WAV.

---

### ElevenLabs (Cloud)

[ElevenLabs](https://elevenlabs.io) provides state-of-the-art AI speech synthesis with natural-sounding voices.

**Features:**

- High-quality neural TTS with emotional range
- 1000+ voices including cloned voices
- Multilingual support (29 languages)
- Voice customization (stability, similarity, style)
- Multiple output formats (MP3, PCM, FLAC, OGG)

**Configuration:**

```json
{
  "tts": {
    "active_backend": "elevenlabs",
    "elevenlabs": {
      "api_key": "your_api_key_here",
      "voice_id": "21m00Tcm4TlvDq8ikWAM",
      "model_id": "eleven_turbo_v2_5",
      "output_format": "mp3_44100_128",
      "voice_stability": 0.5,
      "voice_similarity_boost": 0.75,
      "voice_style": null,
      "use_speaker_boost": null
    }
  }
}
```

**Available Models:**

- `eleven_multilingual_v2` - Latest multilingual model (recommended)
- `eleven_multilingual_v1` - Original multilingual model
- `eleven_monolingual_v1` - English-only model
- `eleven_turbo_v2` - Fast generation, lower quality
- `eleven_turbo_v2_5` - Fastest generation

**Output Formats:**

| Format             | Quality    | File Size  | Notes                          |
| ------------------ | ---------- | ---------- | ------------------------------ |
| `mp3_44100_128`    | Good       | Medium     | **Recommended** - best balance |
| `mp3_44100_192`    | Excellent  | Large      | High quality MP3               |
| `mp3_44100_32`     | Acceptable | Small      | Compact size                   |
| `pcm_44100`        | Lossless   | Very Large | Uncompressed WAV-compatible    |
| `flac_44100`       | Lossless   | Large      | Compressed lossless            |
| `ogg_vorbis_44100` | Good       | Medium     | Open format                    |

**Voice Settings:**

| Setting                  | Range     | Default | Description                                     |
| ------------------------ | --------- | ------- | ----------------------------------------------- |
| `voice_stability`        | 0.0 - 1.0 | 0.5     | Higher = more consistent, Lower = more variable |
| `voice_similarity_boost` | 0.0 - 1.0 | 0.75    | Higher = closer to original speaker             |
| `voice_style`            | 0.0 - 1.0 | null    | Higher = more expressive (optional)             |
| `use_speaker_boost`      | bool      | null    | Improves clarity (optional)                     |

**Getting Started:**

1. Create an account at https://elevenlabs.io
2. Generate an API key at https://elevenlabs.io/app/settings/api-keys
3. In CopySpeak settings, select "ElevenLabs" as the backend
4. Enter your API key
5. Select a voice from the dropdown (voices are fetched from your account)

**Popular Voice IDs:**

- `21m00Tcm4TlvDq8ikWAM` - Rachel (calm, neutral)
- `EXAVITQu4vr4xnSDxMaL` - Bella (warm, conversational)
- `ErXwobaYiN019PkySvjV` - Antoni (friendly, warm)
- `MF3mGyEYCl7XYWbV9V6O` - Elli (expressive, versatile)
- `TxGEqnHWrfWFTfGW9XjX` - Josh (deep, professional)

**API Notes:**

- MP3 formats are playable by rodio immediately
- PCM formats are ideal for maximum quality but larger file sizes
- Playback speed and pitch are controlled via browser frontend playback rate (not at generation level)

---

## Backend Trait Interface

All backends implement the `TtsBackend` trait (`src-tauri/src/tts/mod.rs`).
It is synchronous — the command layer calls it inside `spawn_blocking` — and
speed is deliberately absent, because playback rate is applied in the frontend.

```rust
pub trait TtsBackend: Send + Sync {
    fn name(&self) -> &str;
    /// Blocking synthesis returning a complete audio file.
    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError>;
    fn health_check(&self) -> Result<(), TtsError>;

    /// Load the model ahead of the first utterance. Local engines with a
    /// resident daemon override this; everyone else is a no-op.
    fn prewarm(&self, _voice: &str) {}

    fn supports_streaming(&self) -> bool { false }
    /// PCM chunks as they are produced. The default wraps `synthesize` as a
    /// single chunk, so consumers never special-case an engine.
    fn synthesize_streaming(&self, text: &str, voice: &str) -> Result<ChunkStream, TtsError>;

    fn file_extension(&self) -> &str { "wav" }
    fn voice_display_name(&self, voice_id: &str) -> String { voice_id.to_lowercase() }
}
```

### Error Types

`TtsError` (`src-tauri/src/tts/mod.rs`) covers `Unavailable`, `Http`,
`CommandFailed`, `OutputNotFound`, and `Io`.

### Streaming

A streaming backend produces `ChunkItem::Pcm` values on a `ChunkStream`
(`src-tauri/src/tts/stream.rs`); the command layer forwards each one as an
`audio-stream-chunk` event and the frontend's `PcmStreamScheduler`
(`src/lib/stores/playback/pcm-stream.ts`) schedules them gap-free. **Chunks must
be 16-bit signed LE PCM** — the scheduler drops anything else — so a backend
producing float samples converts before sending.

| Engine | Streams | How |
| --- | --- | --- |
| ElevenLabs | yes | `POST /v1/text-to-speech/{id}/stream`, `output_format=pcm_24000` |
| Cartesia | yes | `POST /tts/bytes` with `container: raw`, `pcm_s16le` @ 44.1 kHz |
| OpenAI | yes | `POST /v1/audio/speech` with `response_format: "pcm"` (24 kHz mono, chunk-transfer). The batch path keeps the user's configured container, which is what file output and the cache want. |
| Google Gemini | yes | `streamGenerateContent?alt=sse`; base64 PCM per `inlineData` part |
| Piper / Kitten / Kokoro / Pocket | yes, once warm | Through the resident daemon (below). Piper yields one chunk per sentence; the others send one chunk per request. |
| Microsoft, HTTP | no | Both point at a user-supplied endpoint. We cannot assume it streams, and guessing wrong means a broken engine rather than a slow one. |
| Edge | no | Synthesis goes through the `edge-tts` Python CLI, which writes an MP3 to disk. Streaming would mean writing a Rust client for Microsoft's Read Aloud WebSocket — a large change for one engine. |

Pagination is evaluated **before** streaming (`commands/tts/synthesis.rs`), so a
long passage is split into fragments and each fragment streams. That keeps every
request under the provider's input limit — OpenAI rejects input past 4096
characters — without giving up low latency. Only the last fragment sends the
`is_final` marker; an intermediate one would arm the player's completion timer
mid-passage.

### Local daemon protocol (v2)

`src-tauri/src/tts/local_daemon.rs` keeps one wrapper process per local engine
alive in `--serve` mode so the model stays in RAM. Everything degrades to the
one-shot command: a wrapper that does not speak v2 is recorded as unsupported
and never retried for that configuration.

Handshake is the line `READY 2`. A bare `READY` is a v1 (temp-file) wrapper —
the user needs to re-run that engine's installer with `-Force`.

A wrapper synthesizes a throwaway phrase *before* sending the handshake, so
`READY 2` means "warm", not merely "loaded". Measured on Piper, this moves the
first request's time-to-first-audio from a 0.47 s median (0.44-0.71) to 0.15 s
(0.14-0.16), for ~0.3 s added to a startup that already runs in the background.

Request, one JSON line on stdin:

```json
{"text": "..."}
```

Reply, JSON header lines interleaved with raw binary on stdout. Every byte count
is declared, so text and binary share one pipe safely:

```text
{"ok":true,"sample_rate":22050,"channels":1,"bits_per_sample":16}
{"chunk":8820}   followed by 8820 bytes of PCM
{"chunk":9600}   followed by 9600 bytes
{"end":true}
```

A failure at any point is one `{"ok":false,"error":"..."}` line; the Rust side
treats it the same before and after the header.

A daemon is keyed on its full command + args, so changing voice, engine, or the
GPU toggle invalidates it: that utterance runs one-shot and the next is warm.
`try_stream` checks the daemon *out* of the registry for the duration of a
stream rather than holding a mutex, which keeps two requests from interleaving
on one pipe. Dropping the receiver (stop/abort) kills the daemon, because the
pipe still holds unread frames.

Wrappers live in `scripts/<engine>/copyspeak-<engine>.py` and are copied into
`%LOCALAPPDATA%\CopySpeak\engines\<engine>\scripts\` by the installer.

### GPU acceleration

Per-engine, off by default, exposed as a `cuda` boolean in the profile's engine
options. When on, CopySpeak passes `--device cuda` and each wrapper picks the
device its library actually exposes:

| Engine | Call |
| --- | --- |
| Piper | `PiperVoice.load(model, use_cuda=True)` |
| Kitten | `KittenTTS(model, backend="cuda")` |
| Kokoro | `ONNX_PROVIDER=CUDAExecutionProvider` (kokoro-onnx reads it) |
| Pocket | `TTSModel.load_model().to("cuda")` — PyTorch, not ONNX |

On Windows, onnxruntime-gpu and torch do not find cuDNN/cuBLAS on their own, and
since Python 3.8 the process `PATH` is ignored for extension-module
dependencies. Each wrapper's `enable_cuda_dlls()` therefore calls
`os.add_dll_directory` on every `nvidia/*/bin` and `nvidia/*/bin/*` directory
(CUDA 12 and CUDA 13 layouts respectively). Doing this inside the process that
loads the DLLs is what makes it work; setting `PATH` from Rust would not.

The GPU runtime is installed **into the engine's own uv project**
(`install-*.ps1 -Cuda`), never into user site-packages, which would shadow the
resolved build for every engine at once. The installer verifies with a real
`--device cuda` synthesis: `ort.get_available_providers()` lists
`CUDAExecutionProvider` even when the DLLs are missing, so it can only produce a
false green.

---

## Adding a New Backend

### Step 1: Create Backend Module

Create `src-tauri/src/tts/my_backend.rs`:

```rust
use super::{TtsBackend, TtsError};
use async_trait::async_trait;

pub struct MyBackend {
    // Configuration fields
}

impl MyBackend {
    pub fn new(/* config */) -> Self {
        Self { /* ... */ }
    }
}

#[async_trait]
impl TtsBackend for MyBackend {
    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        speed: f32,
    ) -> Result<Vec<u8>, TtsError> {
        // Implementation
    }

    async fn health_check(&self) -> Result<bool, TtsError> {
        // Check if backend is available
    }
}
```

### Step 2: Register in mod.rs

```rust
// src-tauri/src/tts/mod.rs
mod cli;
mod my_backend;

pub use cli::CliBackend;
pub use my_backend::MyBackend;
```

### Step 3: Add Preset Configuration

Update `config.rs` to recognize the new preset:

```rust
pub fn backend_for_preset(preset: &str) -> Box<dyn TtsBackend> {
    match preset {
        "kokoro" => Box::new(CliBackend::kokoro_preset()),
        "my-backend" => Box::new(MyBackend::new()),
        _ => Box::new(CliBackend::from_config(config)),
    }
}
```

### Step 4: Document in This File

Add documentation above for users.

---

## Troubleshooting

### Common Issues

| Issue               | Solution                                                  |
| ------------------- | --------------------------------------------------------- |
| "Command not found" | Ensure TTS engine is installed and in PATH                |
| "Invalid WAV"       | Check that TTS engine outputs valid WAV format            |
| "Command failed"    | Check stderr output in logs for engine-specific errors    |
| Slow synthesis      | Consider a faster engine (kokoro, piper) or local install |

### Testing a Backend

```bash
# Test from command line first
kokoro-tts --text "Hello world" --output test.wav --voice af_nicole

# Check WAV file is valid
ffprobe test.wav
```

### Health Check

CopySpeak runs a health check on startup. If it fails:

1. Verify the command exists
2. Check permissions
3. Try running the command manually
4. Check logs for detailed error messages

---

## Performance Comparison

| Engine               | Speed        | Quality      | Offline | Size        | Backend   |
| -------------------- | ------------ | ------------ | ------- | ----------- | --------- |
| Kitten TTS (default) | ⚡ Very Fast | ⭐⭐⭐⭐     | ✅      | 25-80MB     | Local CLI |
| Piper (piper1-gpl)   | ⚡ Very Fast | ⭐⭐⭐⭐     | ✅      | ~60MB/voice | Local CLI |
| kokoro-tts (CLI)     | ⚡ Fast      | ⭐⭐⭐⭐     | ✅      | ~500MB      | Local CLI |
| Chatterbox           | 🐢 Medium    | ⭐⭐⭐⭐⭐   | ✅      | ~2GB        | Local CLI |
| Coqui TTS / XTTS-v2  | 🐢 Medium    | ⭐⭐⭐⭐⭐   | ✅      | ~1-2GB      | Local CLI |
| eSpeak-ng            | ⚡ Very Fast | ⭐⭐         | ✅      | ~5MB        | Local CLI |
| Kokoro server        | ⚡ Fast      | ⭐⭐⭐⭐     | ✅      | ~500MB      | HTTP      |
| Fish Speech 1.5      | ⚡ Fast      | ⭐⭐⭐⭐⭐   | ✅      | ~1GB        | HTTP      |
| Coqui TTS server     | 🐢 Medium    | ⭐⭐⭐⭐⭐   | ✅      | ~1-2GB      | HTTP      |
| Chatterbox server    | 🐢 Medium    | ⭐⭐⭐⭐⭐   | ✅      | ~2GB        | HTTP      |
| Edge TTS             | 🌐 Network   | ⭐⭐⭐⭐⭐   | ❌      | 0 (cloud)   | Local CLI |
| OpenAI               | 🌐 Network   | ⭐⭐⭐⭐⭐   | ❌      | 0 (cloud)   | Cloud API |
| ElevenLabs           | 🌐 Network   | ⭐⭐⭐⭐⭐⭐ | ❌      | 0 (cloud)   | Cloud API |

---

## Local HTTP Server Backend

CopySpeak supports any TTS engine that exposes an HTTP API. Select **"Local HTTP Server"** as the active backend and choose a preset or configure manually.

---

### Kokoro Local Server

[Kokoro FastAPI](https://github.com/remsky/Kokoro-FastAPI) exposes an OpenAI-compatible TTS endpoint.

**Installation:**

```bash
docker run -p 8880:8880 ghcr.io/remsky/kokoro-fastapi-cpu
# or GPU variant
docker run --gpus all -p 8880:8880 ghcr.io/remsky/kokoro-fastapi-gpu
```

**Configuration:**

```json
{
  "tts": { "active_backend": "http", "voice": "af_heart" },
  "http_tts": {
    "url_template": "http://localhost:8880/v1/audio/speech",
    "headers": [["Content-Type", "application/json"]],
    "body_template": "{\"model\":\"kokoro\",\"input\":\"{text}\",\"voice\":\"{voice}\",\"response_format\":\"wav\"}",
    "response_format": "wav",
    "timeout_secs": 30
  }
}
```

---

### Fish Speech 1.5

[Fish Speech](https://github.com/fishaudio/fish-speech) is a fast, multilingual, zero-shot TTS model.

**Installation:**

```bash
pip install fish-speech
fish_speech start
```

**Configuration:**

```json
{
  "tts": { "active_backend": "http", "voice": "default" },
  "http_tts": {
    "url_template": "http://localhost:8880/v1/tts",
    "headers": [["Content-Type", "application/json"]],
    "body_template": "{\"text\":\"{text}\",\"reference_id\":\"{voice}\",\"format\":\"wav\"}",
    "response_format": "wav",
    "timeout_secs": 30
  }
}
```

---

### Coqui TTS Server

[Coqui TTS](https://github.com/coqui-ai/TTS) can be run as a server.

**Installation:**

```bash
pip install TTS
tts-server --model_name tts_models/en/ljspeech/tacotron2-DDC
```

**Configuration:**

```json
{
  "tts": { "active_backend": "http", "voice": "p225" },
  "http_tts": {
    "url_template": "http://localhost:5002/api/tts?text={text}&speaker_id={voice}",
    "headers": [],
    "body_template": null,
    "response_format": "wav",
    "timeout_secs": 60
  }
}
```

**Note:** The `{voice}` maps to a speaker ID. Use `GET /api/tts` for single-speaker models (omit `speaker_id`).

---

### Chatterbox Server

Run Chatterbox as an HTTP server.

**Configuration:**

```json
{
  "tts": { "active_backend": "http", "voice": "default" },
  "http_tts": {
    "url_template": "http://localhost:8000/generate",
    "headers": [["Content-Type", "application/json"]],
    "body_template": "{\"text\":\"{text}\",\"voice\":\"{voice}\"}",
    "response_format": "wav",
    "timeout_secs": 60
  }
}
```

---

### Generic OpenAI-Compatible TTS

Any server implementing the OpenAI `/v1/audio/speech` API (e.g. LocalAI, LiteLLM proxy).

**Configuration:**

```json
{
  "tts": { "active_backend": "http", "voice": "alloy" },
  "http_tts": {
    "url_template": "http://localhost:8880/v1/audio/speech",
    "headers": [
      ["Content-Type", "application/json"],
      ["Authorization", "Bearer sk-not-needed"]
    ],
    "body_template": "{\"model\":\"tts-1\",\"input\":\"{text}\",\"voice\":\"{voice}\",\"response_format\":\"wav\"}",
    "response_format": "wav",
    "timeout_secs": 30
  }
}
```
