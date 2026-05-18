# CopySpeak Pi Voice Extension

Project-local Pi extension: `.pi/extensions/copyspeak-voice/`.

## What it does

- Speaks Pi agent activity through CopySpeak.
- Configures `%APPDATA%/CopySpeak/config.json` for the selected TTS engine and `walkie_talkie` effect.
- Triggers speech through CopySpeak's existing double-copy listener.

## Setup

Set any API keys and start Pi from this repository:

```powershell
$env:CARTESIA_API_KEY="..."
$env:COPYSPEAK_PI_ENGINE="cartesia"
$env:COPYSPEAK_EXE="D:\GitHub\CopySpeak\src-tauri\target\release\copyspeak.exe"
pi
```

If `COPYSPEAK_EXE` is omitted, the extension looks for release/debug `copyspeak.exe` under `src-tauri/target/`.

## Commands

```text
/copyspeak-voice status
/copyspeak-voice on
/copyspeak-voice off
/copyspeak-voice test hello from pi
/copyspeak-voice engine cartesia|openai|elevenlabs|local
/copyspeak-voice activity on|off
/copyspeak-voice assistant on|off
```

## Environment flags

- `COPYSPEAK_PI_ENABLED=0` disables voice on startup.
- `COPYSPEAK_PI_ENGINE=cartesia|openai|elevenlabs|local` selects engine.
- `COPYSPEAK_PI_ASSISTANT=0` disables final assistant-message speech.
- `COPYSPEAK_PI_ACTIVITY=0` disables thinking/tool announcements.
- `COPYSPEAK_PI_MAX_CHARS=700` limits final response speech length.
- `COPYSPEAK_PI_LAUNCH=0` prevents launching CopySpeak.

## Loop/focus behavior

The extension serializes clipboard triggers and writes only one double-copy sequence per spoken message: a unique primer, then two identical clipboard writes. This avoids stale clipboard state retriggering additional cycles.

When Pi starts, the extension checks whether `copyspeak.exe` is already running before launching it. This avoids triggering CopySpeak's single-instance focus behavior while using Pi.
