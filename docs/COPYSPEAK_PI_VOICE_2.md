# CopySpeak TTS Agent Voice

Project-local Pi extension: `.pi/extensions/copyspeak/`.
Project-local Claude Code hook: `scripts/claude-copyspeak-hook.mjs` wired from `.claude/settings.json` or `.claude/settings.local.json`.

## What it does

- Speaks final Pi or Claude Code assistant responses through the running CopySpeak TTS app by default.
- Pi can optionally speak agent/tool activity and thinking blocks.
- Claude Code uses `Stop`/`SubagentStop` hooks to read the latest assistant message from the transcript.
- Triggers speech through CopySpeak TTS's local control server, not the Windows clipboard.

## Setup

Set any API keys and start Pi from this repository:

```powershell
$env:CARTESIA_API_KEY="..."
$env:COPYSPEAK_PI_ENGINE="cartesia" # optional override; omit to use app settings
pi
```

Start CopySpeak TTS yourself before using the extension. If `COPYSPEAK_PI_LAUNCH=1` is set, the extension looks for release/debug `copyspeak.exe` under `src-tauri/target/` and launches it when needed.

## Claude Code hook

The repo-local Claude hook is configured in `.claude/settings.json`; `.claude/settings.example.json` contains the same copyable config:

```json
{
  "hooks": {
    "Stop": [{ "matcher": "", "hooks": [{ "type": "command", "command": "node scripts/claude-copyspeak-hook.mjs" }] }],
    "SubagentStop": [{ "matcher": "", "hooks": [{ "type": "command", "command": "node scripts/claude-copyspeak-hook.mjs" }] }]
  }
}
```

`.claude/` is gitignored in this repo, so these settings are local-only unless force-added or moved to a tracked docs path.

## Commands

```text
/copyspeak status
/copyspeak on
/copyspeak off
/copyspeak test hello from pi
/copyspeak engine cartesia|openai|elevenlabs|local
/copyspeak activity on|off
/copyspeak assistant on|off
/copyspeak thinking on|off
```

## Environment flags

- `COPYSPEAK_PI_ENABLED=0` / `COPYSPEAK_CLAUDE_ENABLED=0` disables voice on startup.
- `COPYSPEAK_PI_ENGINE=cartesia|openai|elevenlabs|local` / `COPYSPEAK_CLAUDE_ENGINE=...` overrides the running app engine.
- `COPYSPEAK_PI_EFFECT=walkie_talkie` / `COPYSPEAK_CLAUDE_EFFECT=...` overrides the running app effect.
- `COPYSPEAK_PI_ASSISTANT=0` disables final assistant-message speech in Pi.
- `COPYSPEAK_PI_ACTIVITY=1` enables thinking/tool announcements in Pi.
- `COPYSPEAK_PI_THINKING=0` disables spoken thinking blocks in Pi.
- `COPYSPEAK_PI_MAX_CHARS=700` / `COPYSPEAK_CLAUDE_MAX_CHARS=700` limits final response speech length.
- `COPYSPEAK_PI_LAUNCH=1` / `COPYSPEAK_CLAUDE_LAUNCH=1` enables auto-launching CopySpeak TTS.
- `COPYSPEAK_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.
