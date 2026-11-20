<div align="center">

<img src="static/app-logo.svg" alt="CopySpeech logo" width="96" />

# CopySpeech

**Double-copy any text. CopySpeech reads it aloud.**

CopySpeech reads your clipboard aloud. Copy the same text twice within 1.5 seconds, and an AI voice speaks it.

[![Release](https://img.shields.io/github/v/release/ilyaizen/CopySpeech)](https://github.com/ilyaizen/CopySpeech/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[Download](https://github.com/ilyaizen/CopySpeech/releases) · [Changelog](CHANGELOG.md) · [Contributing](docs/CONTRIBUTING.md) · [Issues](https://github.com/ilyaizen/CopySpeech/issues)

</div>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="static/screen-0.3.1-dark.png">
  <img alt="CopySpeech screenshot" src="static/screen-0.3.1-light.png">
</picture>

## Features

- **Double-copy trigger.** No hotkey needed. The tray and the Play page also start playback.
- **15 built-in engines.** Run them locally or in the cloud. Add any CLI tool or HTTP endpoint. See [docs/engines.md](docs/engines.md).
  - Local: tiny-voca (default, English text only), Kokoro, Piper, Kitten, Qwen3-TTS, Pocket.
  - Cloud: OpenAI, ElevenLabs, Cartesia, Fish Audio, xAI, Mistral, Google Gemini, Microsoft Azure.
- **One-click local installs.** Kokoro installs from onboarding, with optional CUDA.
- **Live captions.** Words highlight as they play. Captions need an engine that reports timings.
- **Streaming playback.** Local engines start speaking after the first audio window.
- **Speed, pitch, and volume.** Voice profiles save each combination per engine.
- **Text cleanup.** Strips markdown, expands URLs, and normalizes Unicode.
- **Duck other apps (Windows).** Other apps get quieter while CopySpeech speaks.
- **Browser companion.** Reads selected text on a page. Chrome, Edge, and Firefox are supported. The page highlights each word as it plays.
- **Interface.** 9 languages, right-to-left layouts, dark and light themes, tray icon, and auto-updates.

## Install

Download a build from [Releases](https://github.com/ilyaizen/CopySpeech/releases). The latest version is 0.3.2.

| Platform          | Asset                                                                    |
| ----------------- | ------------------------------------------------------------------------ |
| Windows           | `CopySpeech_<version>_x64-setup.exe` (`.msi` also available)             |
| macOS (Intel)     | `CopySpeech_<version>_x64.dmg` (Apple Silicon runs it through Rosetta 2) |
| Linux             | `.AppImage`, `.deb`, or `.rpm`                                           |
| Browser companion | `CopySpeech-Companion-<version>.zip`                                     |

On first launch, onboarding asks you to pick an engine and a voice. tiny-voca is the default.

**Linux:** Wayland blocks global hotkeys. Use double-copy or the tray.

### Browser companion (Chrome)

1. Start the desktop app.
2. Extract the companion zip.
3. Open `chrome://extensions`, turn on **Developer mode**, and click **Load unpacked**. Select the extracted folder.
4. Grant per-site access in the extension settings.

Installed Windows builds register the Chrome host at launch. Edge, Firefox, Linux, and macOS need manual setup. See [BROWSER_EXTENSION.md](BROWSER_EXTENSION.md).

## Integrations

- **Hermes Desktop and DeepSeek Harness (DSH) plugins.** Speak new assistant replies in those apps. See [copyspeech-dsh/README.md](copyspeech-dsh/README.md) for DSH.
- **Pi extension and Claude Code hook.** Speak final assistant replies. See [docs/agent-voice.md](docs/agent-voice.md).
- **Claude Code plugin.** Play buttons and live captions in Claude Code. See [copyspeech-claude/README.md](copyspeech-claude/README.md).

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
