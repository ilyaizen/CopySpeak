# CopySpeak Pi voice extension

Reads Pi agent activity aloud through the real CopySpeak app.

## How it works

- Pi extension speaks finalized assistant `message_end` events by default. Optional activity mode also listens to `agent_start` and `tool_execution_start` events.
- It patches `%APPDATA%/CopySpeak/config.json` to:
  - enable clipboard listening for normal CopySpeak use
  - enable the `walkie_talkie` audio effect
  - select the configured TTS engine
  - load API keys from environment variables
- It speaks through CopySpeak's local control server (`http://127.0.0.1:43117/speak`) so Pi does not touch the Windows clipboard.

## Setup

Put this directory at either:

- project-local: `.pi/extensions/copyspeak-voice/index.ts` (already here)
- global: `%USERPROFILE%/.pi/agent/extensions/copyspeak-voice/index.ts`

Set keys before starting Pi:

```powershell
$env:CARTESIA_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:ELEVENLABS_API_KEY="..."
$env:COPYSPEAK_PI_ENGINE="cartesia"
$env:COPYSPEAK_EXE="D:\GitHub\CopySpeak\src-tauri\target\release\copyspeak.exe"
pi
```

If `COPYSPEAK_EXE` is omitted, the extension tries `src-tauri/target/release/copyspeak.exe` and `src-tauri/target/debug/copyspeak.exe` under the current working directory.

The extension checks whether `copyspeak.exe` is already running before launching it, so starting Pi should not focus an existing CopySpeak window. CopySpeak must be running a build that includes the local control server.

## Commands inside Pi

```text
/copyspeak-voice status
/copyspeak-voice on
/copyspeak-voice off
/copyspeak-voice test hello from pi
/copyspeak-voice engine cartesia
/copyspeak-voice engine openai
/copyspeak-voice engine elevenlabs
/copyspeak-voice engine local
/copyspeak-voice activity on
/copyspeak-voice activity off
/copyspeak-voice assistant off
```

## Environment flags

- `COPYSPEAK_PI_ENABLED=0` disables on startup.
- `COPYSPEAK_PI_ENGINE=cartesia|openai|elevenlabs|local` selects engine.
- `COPYSPEAK_PI_ASSISTANT=0` disables speaking final assistant messages.
- `COPYSPEAK_PI_ACTIVITY=1` enables optional "thinking/tool" announcements.
- `COPYSPEAK_PI_MAX_CHARS=700` controls max spoken final response length.
- `COPYSPEAK_PI_LAUNCH=0` prevents auto-launching CopySpeak.
- `COPYSPEAK_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.
