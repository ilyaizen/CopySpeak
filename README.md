# CopySpeak

A lightweight Windows Wrapper GUI for AI Text-to-Speech engines. It reads clipboard text aloud on a double-copy trigger with high-quality AI TTS voices.

## Quick Start

```bash
bun install
bun run tauri dev
```

## Features

- **Double-copy trigger**: Copy same text twice within 800ms to trigger TTS
- **Multiple TTS backends**: Local (kokoro-tts, piper) and Cloud (OpenAI, ElevenLabs)
- **System tray**: Quick access to controls
- **Audio save mode**: Save TTS output to files
- **Dark/Light mode**: Brutalist design with theme support

## Tech Stack

| Component       | Technology                       |
| --------------- | -------------------------------- |
| Backend         | Rust (Tauri v2)                  |
| Frontend        | Svelte 5, TypeScript, Vite       |
| Package Manager | Bun v1.3                         |
| Audio           | rodio                            |
| UI              | shadcn-svelte, Tailwind CSS v4.2 |

## Project Structure

```
src/                  # Svelte 5 frontend
src-tauri/src/        # Rust backend
├── main.rs           # Entry point
├── config.rs         # Persistence
├── commands.rs       # IPC handlers
├── clipboard.rs      # Double-copy detection
├── audio.rs         # Playback
└── tts/             # TTS backends
```

## Commands

```bash
bun run tauri dev    # Development
bun run tauri build  # Production build
bun run check        # Type check
bun run test         # Run tests
```
