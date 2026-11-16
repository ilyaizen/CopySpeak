# CopySpeech Pi voice extension

Reads only final Pi assistant responses aloud through the real CopySpeech app by default, without speaking Pi thinking blocks unless enabled.

## How it works

- Pi extension speaks once per completed agent run (`agent_end`) by default. Optional activity mode also listens to `agent_start` and `tool_execution_start` events. Thinking blocks are off by default.
- It speaks through CopySpeech's local control server (`http://127.0.0.1:43117/speak`) so Pi does not touch the Windows clipboard.
- By default it does not patch CopySpeech config or force engine/effect; it uses the settings from the running CopySpeech app.
- Final assistant responses are sent to the running app, which applies saved sanitization/pre-processing, max length, LLM post-processing, effects, and TTS settings before playback.

## Setup

Put this directory at either:

- project-local: `.pi/extensions/copyspeech/index.ts` (already here)
- global: `%USERPROFILE%/.pi/agent/extensions/copyspeech/index.ts`

Set keys before starting Pi:

```powershell
$env:CARTESIA_API_KEY="..."
$env:OPENAI_API_KEY="..."
$env:ELEVENLABS_API_KEY="..."
# Optional override; omit this to use the running CopySpeech app settings.
$env:COPYSPEECH_PI_ENGINE="cartesia"
pi
```

Start CopySpeech yourself before using the extension. CopySpeech must be running a build that includes the local control server.

If you explicitly set `COPYSPEECH_PI_LAUNCH=1`, the extension can auto-launch CopySpeech. With `COPYSPEECH_EXE` omitted, it tries `src-tauri/target/release/copyspeech.exe` and `src-tauri/target/debug/copyspeech.exe` under the current working directory.

## Commands inside Pi

```text
/copyspeech status
/copyspeech on
/copyspeech off
/copyspeech test hello from pi
/copyspeech engine cartesia
/copyspeech engine openai
/copyspeech engine elevenlabs
/copyspeech engine local
/copyspeech activity on
/copyspeech activity off
/copyspeech assistant off
/copyspeech thinking on
/copyspeech thinking off
```

## Environment flags

- `COPYSPEECH_PI_ENABLED=0` disables on startup.
- `COPYSPEECH_PI_ENGINE=cartesia|openai|elevenlabs|local` overrides the running app engine; omit to use CopySpeech settings.
- `COPYSPEECH_PI_EFFECT=walkie_talkie` overrides the running app effect; omit to use CopySpeech settings.
- `COPYSPEECH_PI_ASSISTANT=0` disables speaking final assistant messages.
- `COPYSPEECH_PI_ACTIVITY=1` enables optional agent/tool activity announcements.
- `COPYSPEECH_PI_THINKING=1` enables speaking assistant thinking blocks when Pi includes them. Default is off.
- `COPYSPEECH_PI_MAX_CHARS=700` is kept for compatibility but final assistant messages use the running CopySpeech app's max length setting.
- `COPYSPEECH_PI_LAUNCH=1` enables auto-launching CopySpeech. Default is manual/no launch.
- `COPYSPEECH_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.
