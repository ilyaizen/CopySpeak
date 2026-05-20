# CopySpeak Pi voice extension

Reads only final Pi assistant responses aloud through the real CopySpeak app by default, without speaking Pi thinking blocks unless enabled.

## How it works

- Pi extension speaks once per completed agent run (`agent_end`) by default. Optional activity mode also listens to `agent_start` and `tool_execution_start` events. Thinking blocks are off by default.
- It speaks through CopySpeak's local control server (`http://127.0.0.1:43117/speak`) so Pi does not touch the Windows clipboard.
- When CopySpeak returns post-processed text, Pi shows that processed text in a notification and widget.
- By default it does not patch CopySpeak config or force engine/effect; it uses the settings from the running CopySpeak app.

## Setup

Put this directory at either:

- project-local: `.pi/extensions/copyspeak/index.ts` (already here)
- global: `%USERPROFILE%/.pi/agent/extensions/copyspeak/index.ts`

Set keys before starting Pi:

```powershell
$env:CARTESIA_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:ELEVENLABS_API_KEY="..."
# Optional override; omit this to use the running CopySpeak app settings.
$env:COPYSPEAK_PI_ENGINE="cartesia"
pi
```

Start CopySpeak yourself before using the extension. CopySpeak must be running a build that includes the local control server.

If you explicitly set `COPYSPEAK_PI_LAUNCH=1`, the extension can auto-launch CopySpeak. With `COPYSPEAK_EXE` omitted, it tries `src-tauri/target/release/copyspeak.exe` and `src-tauri/target/debug/copyspeak.exe` under the current working directory.

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
- `COPYSPEAK_PI_ENGINE=cartesia|openai|elevenlabs|local` overrides the running app engine; omit to use CopySpeak settings.
- `COPYSPEAK_PI_EFFECT=walkie_talkie` overrides the running app effect; omit to use CopySpeak settings.
- `COPYSPEAK_PI_ASSISTANT=0` disables speaking final assistant messages.
- `COPYSPEAK_PI_ACTIVITY=1` enables optional agent/tool activity announcements.
- `COPYSPEAK_PI_THINKING=1` enables speaking assistant thinking blocks when Pi includes them. Default is off.
- `COPYSPEAK_PI_MAX_CHARS=<number>` optionally caps spoken final response length. Default is unlimited (`0`).
- `COPYSPEAK_PI_LAUNCH=1` enables auto-launching CopySpeak. Default is manual/no launch.
- `COPYSPEAK_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.
