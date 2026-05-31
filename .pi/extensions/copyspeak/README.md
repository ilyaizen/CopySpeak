# CopySpeak TTS Pi voice extension

Reads only final Pi assistant responses aloud through the real CopySpeak TTS app by default, without speaking Pi thinking blocks unless enabled.

## How it works

- Pi extension speaks once per completed agent run (`agent_end`) by default. Optional activity mode also listens to `agent_start` and `tool_execution_start` events. Thinking blocks are off by default.
- It speaks through CopySpeak TTS's local control server (`http://127.0.0.1:43117/speak`) so Pi does not touch the Windows clipboard.
- By default it does not patch CopySpeak TTS config or force engine/effect; it uses the settings from the running CopySpeak TTS app.
- Final assistant responses are sent to the running app, which applies saved sanitization/pre-processing, max length, LLM post-processing, effects, and TTS settings before playback.

## Setup

Put this directory at either:

- project-local: `.pi/extensions/copyspeak/index.ts` (already here)
- global: `%USERPROFILE%/.pi/agent/extensions/copyspeak/index.ts`

Set keys before starting Pi:

```powershell
$env:CARTESIA_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:ELEVENLABS_API_KEY="..."
# Optional override; omit this to use the running CopySpeak TTS app settings.
$env:COPYSPEAK_PI_ENGINE="cartesia"
pi
```

Start CopySpeak TTS yourself before using the extension. CopySpeak TTS must be running a build that includes the local control server.

If you explicitly set `COPYSPEAK_PI_LAUNCH=1`, the extension can auto-launch CopySpeak TTS. With `COPYSPEAK_EXE` omitted, it tries `src-tauri/target/release/copyspeak.exe` and `src-tauri/target/debug/copyspeak.exe` under the current working directory.

## Commands inside Pi

```text
/copyspeak status
/copyspeak on
/copyspeak off
/copyspeak test hello from pi
/copyspeak engine cartesia
/copyspeak engine openai
/copyspeak engine elevenlabs
/copyspeak engine local
/copyspeak activity on
/copyspeak activity off
/copyspeak assistant off
/copyspeak thinking on
/copyspeak thinking off
```

## Environment flags

- `COPYSPEAK_PI_ENABLED=0` disables on startup.
- `COPYSPEAK_PI_ENGINE=cartesia|openai|elevenlabs|local` overrides the running app engine; omit to use CopySpeak TTS settings.
- `COPYSPEAK_PI_EFFECT=walkie_talkie` overrides the running app effect; omit to use CopySpeak TTS settings.
- `COPYSPEAK_PI_ASSISTANT=0` disables speaking final assistant messages.
- `COPYSPEAK_PI_ACTIVITY=1` enables optional agent/tool activity announcements.
- `COPYSPEAK_PI_THINKING=1` enables speaking assistant thinking blocks when Pi includes them. Default is off.
- `COPYSPEAK_PI_MAX_CHARS=700` is kept for compatibility but final assistant messages use the running CopySpeak TTS app's max length setting.
- `COPYSPEAK_PI_LAUNCH=1` enables auto-launching CopySpeak TTS. Default is manual/no launch.
- `COPYSPEAK_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.
