# CopySpeech Agent Voice

Speaks AI-agent activity through the running CopySpeech app.

- **Pi extension** (project-local): `.pi/extensions/copyspeech/`
- **Claude Code hook** (project-local): `scripts/claude-copyspeech-hook.mjs`, wired from `.claude/settings.json` or `.claude/settings.local.json`
- **Claude Code plugin** (separate package): `copyspeech-claude/`, with play buttons, captions, and a playback bar. Its auto-read and this hook both speak a reply when both are on. See [copyspeech-claude/README.md](../copyspeech-claude/README.md).
- **DSH plugin** (separate package): `copyspeech-dsh/`, installed into a DeepSeek Harness profile

## What it does

- Speaks final Pi or Claude Code assistant responses through the running CopySpeech app by default.
- Pi can optionally speak agent/tool activity and thinking blocks.
- Claude Code uses `Stop`/`SubagentStop` hooks to read the latest assistant message from the transcript.
- The DSH plugin reads each settled reply on demand or as it settles, with optional live captions.
- Triggers speech through CopySpeech's local control server, not the Windows clipboard.

## Setup

Pi (start from this repository):

```powershell
$env:CARTESIA_API_KEY="..."
$env:COPYSPEECH_PI_ENGINE="cartesia" # optional override; omit to use app settings
pi
```

Start CopySpeech yourself before using the extension. If `COPYSPEECH_PI_LAUNCH=1` is set, the extension looks for release/debug `copyspeech.exe` under `src-tauri/target/` and launches it when needed.

## Claude Code hook

The repo-local Claude hook is configured in `.claude/settings.json`; `.claude/settings.example.json` contains the same copyable config:

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [{ "type": "command", "command": "node scripts/claude-copyspeech-hook.mjs" }]
      }
    ],
    "SubagentStop": [
      {
        "matcher": "",
        "hooks": [{ "type": "command", "command": "node scripts/claude-copyspeech-hook.mjs" }]
      }
    ]
  }
}
```

`.claude/` is gitignored in this repo, so these settings are local-only unless force-added or moved to a tracked docs path.

## DSH plugin

The DSH plugin is a separate package in `copyspeech-dsh/` (not part of the app build). It adds a **CopySpeech** panel in the DeepSeek Harness sidebar, and a speaker action to each finished assistant message.

Install it into a DSH profile with the Plugin Manager's `install_bundle` action pointed at the `copyspeech-dsh` directory. The profile links that directory, so browser-half edits reach an open page through Client HMR; a host-half edit needs a DSH restart.

The panel is the only settings place. It carries four switches, stored in the page's `localStorage`:

- **Read new replies aloud** reads each reply in the open session when it settles. It starts at the next reply and never replays old ones.
- **Show captions while reading** highlights the word CopySpeech plays inside the reply text. It is off by default.
- **Animate caption highlight** glides the highlight between words.
- **Follow playback** scrolls the chat to keep the spoken word in view.

With automatic reading on, an experimental **Read only part of the reply** switch adds the top, bottom, or both-ends limit: 1–20 paragraphs per end, default 2 and 2. The limit applies only to automatic reads; the speaker action always reads the whole reply.

Speech goes to `POST /speak` with `wait: true`, so the speaker action returns to idle when playback ends. One reading runs at a time: a new reading stops the previous one by request id. The watcher and the speaker action strip Markdown with the same `toSpeech()` pass before posting; code fences are removed, inline code is kept.

Captions poll a read-only control-server route, `GET /status`, which answers the caption and word position the CopySpeech webview already computes for its own UI. The route sends `Access-Control-Allow-Origin: *`; the other control routes stay opaque to browsers. No reading, no route answer: the plugin stops polling and removes the highlight. A CopySpeech build without `/status`, or an engine without timings, reads aloud and shows no captions. Timings follow the provider: ElevenLabs sends one entry per character and other engines send words, so the plugin groups entries under the rendered words before aligning. Provider word windows can have silent gaps, so the last painted word holds until the next word paints; only an inactive snapshot, a same-fragment rewind, a reset, or the end of the reading clears it.

The playback bar controls the reading it started through three scoped routes: `POST /pause` and `POST /resume` with `{"request_id"}`, and `POST /seek` with `{"request_id","delta_ms"}`. `delta_ms` is milliseconds, negative rewinds, positive fast-forwards, and its absolute value is at most 60000. A control applies only while its named reading owns playback, so it never pauses, resumes or seeks a different reading. A reading that is still synthesizing gets up to ten seconds to become audible before the route answers `not_playing`. The answer is `{"ok":true,"request_id":…,"outcome":"paused"|"resumed"|"seeked"|"not_playing"}`. The plugin posts these payloads with `mode: "no-cors"` and a `text/plain` body, so the response is opaque and the answer is advisory; `GET /status` stays the only CORS-open route.

The plugin finds the settled message through the Chat row's `data-chat-node-key` and the Markdown body's `data-text-interaction-source`, aligns each caption fragment to the rendered text nodes, and paints the active word with the CSS Custom Highlight API. The highlight therefore sits inside the assistant message, not in a separate strip. The browser half reaches the API through `window.CSS.highlights` and `window.Highlight`: a local `CSS` constant would shadow the browser global and silently disable the highlight. The API cannot round its background, so a positioned layer inside the reply paints the rounded brand mark behind the word, and the highlight recolors the word's text over it. The layer lives in the reply's own stacking context, so it stays behind the text without wrapping host words in spans.

## Commands

```text
/copyspeech status
/copyspeech on
/copyspeech off
/copyspeech test hello from pi
/copyspeech engine cartesia|openai|elevenlabs|local
/copyspeech activity on|off
/copyspeech assistant on|off
/copyspeech thinking on|off
```

## Environment flags

- `COPYSPEECH_PI_ENABLED=0` / `COPYSPEECH_CLAUDE_ENABLED=0` disables voice on startup.
- `COPYSPEECH_PI_ENGINE=cartesia|openai|elevenlabs|local` / `COPYSPEECH_CLAUDE_ENGINE=...` overrides the running app engine.
- `COPYSPEECH_PI_EFFECT=walkie_talkie` / `COPYSPEECH_CLAUDE_EFFECT=...` overrides the running app effect.
- `COPYSPEECH_PI_ASSISTANT=0` disables final assistant-message speech in Pi.
- `COPYSPEECH_PI_ACTIVITY=1` enables thinking/tool announcements in Pi.
- `COPYSPEECH_PI_THINKING=0` disables spoken thinking blocks in Pi.
- `COPYSPEECH_PI_MAX_CHARS=700` / `COPYSPEECH_CLAUDE_MAX_CHARS=700` limits final response speech length.
- `COPYSPEECH_PI_LAUNCH=1` / `COPYSPEECH_CLAUDE_LAUNCH=1` enables auto-launching CopySpeech.
- `COPYSPEECH_CONTROL_URL=http://127.0.0.1:43117/speak` overrides the local control endpoint.

## History

Merged from `docs/COPYSPEECH_PI_VOICE.md` + `docs/COPYSPEECH_PI_VOICE_2.md` (2026-09-22). The V1-only clipboard-primer trigger described the pre-control-server behavior and no longer matches the extension code.
