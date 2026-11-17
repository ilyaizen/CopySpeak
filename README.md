<div align="center">

<img src="static/app-logo.svg" alt="CopySpeech logo" width="96" />

# CopySpeech

**Copy twice. Hear it.**

CopySpeech reads your clipboard aloud. Copy the same text twice within 1.5 seconds, and an AI voice speaks it.

[![Release](https://img.shields.io/github/v/release/ilyaizen/CopySpeech)](https://github.com/ilyaizen/CopySpeech/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[Download](https://github.com/ilyaizen/CopySpeech/releases) · [Changelog](CHANGELOG.md) · [Contributing](docs/CONTRIBUTING.md) · [Issues](https://github.com/ilyaizen/CopySpeech/issues)

</div>

![CopySpeech screenshot](static/screen-0.3.0.png)

## Features

- **Double-copy trigger.** No hotkey needed. The tray and the Play page also start playback.
- **14 built-in engines.** Run them locally or in the cloud. Add any CLI tool or HTTP endpoint. See [docs/engines.md](docs/engines.md).
  - Local: Kokoro (default), Piper, Kitten, Qwen3-TTS, Pocket, tiny-voca.
  - Cloud: OpenAI, ElevenLabs, Cartesia, Fish Audio, xAI, Mistral, Google Gemini, Microsoft Azure.
- **One-click local installs.** Kokoro installs from onboarding, with optional CUDA.
- **Live captions.** Words highlight as they play. Captions need an engine that reports timings.
- **Streaming playback.** Local engines start speaking after the first audio window.
- **Speed, pitch, and volume.** Voice profiles save each combination per engine.
- **Text cleanup.** Strips markdown, expands URLs, and normalizes Unicode.
- **Duck other apps (Windows).** Other apps get quieter while CopySpeech speaks.
- **Browser companion.** Reads selected text on a page. Chrome, Edge, and Firefox are supported. The page highlights each word as it plays.
- **Control server.** A local HTTP API for scripts and tools. See [Control server](#control-server).
- **Interface.** 9 languages, right-to-left layouts, dark and light themes, tray icon, and auto-updates.

## Install

Download a build from [Releases](https://github.com/ilyaizen/CopySpeech/releases). The latest version is 0.3.1.

| Platform          | Asset                                                                    |
| ----------------- | ------------------------------------------------------------------------ |
| Windows           | `CopySpeech_<version>_x64-setup.exe` (`.msi` also available)             |
| macOS (Intel)     | `CopySpeech_<version>_x64.dmg` (Apple Silicon runs it through Rosetta 2) |
| Linux             | `.AppImage`, `.deb`, or `.rpm`                                           |
| Browser companion | `CopySpeech-Companion-<version>.zip`                                     |

On first launch, onboarding asks you to pick an engine and a voice. Kokoro is the default.

**Linux:** Wayland blocks global hotkeys. Use double-copy, the tray, or a compositor keybind that calls the [control server](#control-server).

### Browser companion (Chrome)

1. Start the desktop app.
2. Extract the companion zip.
3. Open `chrome://extensions`, turn on **Developer mode**, and click **Load unpacked**. Select the extracted folder.
4. Grant per-site access in the extension settings.

Installed Windows builds register the Chrome host at launch. Edge, Firefox, Linux, and macOS need manual setup. See [BROWSER_EXTENSION.md](BROWSER_EXTENSION.md).

## Integrations

- **Hermes Desktop and DeepSeek Harness (DSH) plugins.** Speak new assistant replies in those apps. See [copyspeech-dsh/README.md](copyspeech-dsh/README.md) for DSH.
- **Pi extension and Claude Code hook.** Speak final assistant replies. See [docs/agent-voice.md](docs/agent-voice.md).

## Control server

A local HTTP API on `127.0.0.1:43117`.

| Route                         | Purpose                                     |
| ----------------------------- | ------------------------------------------- |
| `GET /health`                 | Check that the app runs                     |
| `GET /status`                 | Read the current caption                    |
| `POST /speak`                 | Speak text. Pass `wait` to block until done |
| `POST /pause`, `POST /resume` | Pause or resume a reading                   |
| `POST /seek`                  | Move a reading by a signed step             |
| `POST /stop`                  | Stop playback                               |

## Development

```bash
bun install
bun run tauri dev
```

| Command                      | Purpose                                 |
| ---------------------------- | --------------------------------------- |
| `bun run dev`                | Frontend only, on port 5173             |
| `bun run check`              | Svelte and TypeScript checks, then lint |
| `bun run test`               | Frontend tests (Vitest)                 |
| `bun run tauri build`        | Production build                        |
| `cd src-tauri && cargo test` | Rust tests                              |

**Stack:** Rust and Tauri 2 (backend), Svelte 5 and SvelteKit with TypeScript (frontend), and Bun.

Read [Understanding the Codebase](docs/CONTRIBUTING.md#understanding-the-codebase) before you change code.

## Roadmap

The next release targets lower time-to-first-audio and a smaller VRAM and RAM footprint. The goal is local engines that run well on low-VRAM GPUs and CPU-only machines.

## Contributing

See the [Contributing Guide](docs/CONTRIBUTING.md) for setup, style, and pull request steps.

## License

[MIT](LICENSE)
