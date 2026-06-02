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

[Piper](https://github.com/OHF-Voice/piper1-gpl) (piper1-gpl) is a fast, local offline neural TTS engine. CopySpeak optimizes Piper performance by running a persistent background server, keeping the model loaded in RAM to eliminate reload latency.

#### 1. Setup & Automation

To automate installing Piper and setting up dependencies on Windows, run one of the helper scripts in the project root:

##### For CPU-Only Caching Server:
```powershell
# Run in PowerShell
./setup-piper-cpu.ps1
```
This script will install `piper-tts[http]` and standard `onnxruntime` CPU-only dependencies.

##### For GPU/CUDA Caching Server:
```powershell
# Run in PowerShell
./setup-piper-cuda.ps1
```
This script will install `piper-tts[http]`, `onnxruntime-gpu`, and official NVIDIA PyPI library dependencies (`nvidia-cuda-runtime-cu12`, `nvidia-cudnn-cu12`, etc.) to run on the GPU out-of-the-box.

#### 2. Manual Installation

##### For CPU-Only Inference:
```bash
pip install "piper-tts[http]"
```

##### For CUDA/GPU Acceleration:
1. Ensure you have modern NVIDIA drivers and a CUDA-compatible GPU.
2. Uninstall CPU onnxruntime and install GPU/NVIDIA packages:
```bash
pip uninstall onnxruntime
pip install onnxruntime-gpu nvidia-cuda-runtime-cu12 nvidia-cudnn-cu12 nvidia-cublas-cu12 nvidia-cufft-cu12 nvidia-curand-cu12 nvidia-cusolver-cu12 nvidia-cusparse-cu12 nvidia-nvjitlink-cu12
```

#### 3. Voice Model Downloads

All `.onnx` and `.onnx.json` files must be placed in a folder named `piper-voices` in your user home directory:
`C:\Users\<User>\piper-voices`

You can download voices via python:
```bash
python3 -m piper.download_voices en_US-joe-medium --data-dir C:\Users\<User>\piper-voices
```
CopySpeak automatically scans this directory on startup and populates the dropdown voice menu dynamically with all quality variations (low, medium, high).

#### 4. Preset Configuration (applied automatically when "Piper" is selected):

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
pip install edge-tts
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

All backends implement the `TtsBackend` trait:

```rust
#[async_trait]
pub trait TtsBackend: Send + Sync {
    /// Synthesize text to WAV audio bytes
    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        _speed: f32,
    ) -> Result<Vec<u8>, TtsError>;

    /// Check if the backend is available and properly configured
    async fn health_check(&self) -> Result<bool, TtsError>;
}
```

### Error Types

```rust
pub enum TtsError {
    CommandNotFound(String),
    CommandFailed { code: i32, stderr: String },
    OutputNotFound(PathBuf),
    InvalidWav(String),
    IoError(std::io::Error),
}
```

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
