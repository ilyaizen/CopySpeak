# CopySpeak for Hermes Desktop

Reads each new completed assistant reply aloud through the running CopySpeak app.

Verified live in the app on 2026-11-16: the highlight moves with speech, excerpt auto-read works, and replies do not replay.

## Settings

Settings → Plugins → CopySpeak.

- **Read new replies aloud** – off by default.
- **Read only part of the reply** – on by default. Reads top, bottom, or both.
- **Paragraph counts** – 1 to 20 for top and bottom.
- **Highlight spoken words** – on by default. Highlights the word CopySpeak speaks inside the reply.

Palette command `CopySpeak: Toggle auto-read` flips the first setting.

## Captions

- Needs CopySpeak running and the reply open in the focused chat.
- Reads CopySpeak's `GET /status` every 120 ms while a reading plays.
- Paints with the CSS Custom Highlight API (`::highlight(copyspeak-word)`). No Hermes source or bundle is patched.
- Finds the reply's text in the newest assistant message, skipping reasoning blocks.
- Stops 2 s after the last matched word, or after 60 s with no match. Clears on stop or on disable.

## Install

Create a junction so the app loads the repo file in place:

```
New-Item -ItemType Junction -Path "$env:LOCALAPPDATA\hermes\desktop-plugins\copyspeak-hermes" -Target "D:\Projects\CopySpeak\copyspeak-hermes"
```

Edits to `plugin.js` reload in the app. Load errors appear in `desktop.log` as `runtime load failed (copyspeak-hermes)`.

## Double playback rule

If Hermes's own "Read replies aloud" is on, both readers speak. The plugin skips every reply while that setting is on. Turn the built-in setting off to use this plugin.

## Limits (v1)

- No playback bar. Requests use `wait: false`.
- Captions match on text. A reading that CopySpeak runs for other text paints nothing.
- Two windows on one chat may race on a claim. The window is small.
- CopySpeak must run. If it does not, the plugin shows one error toast.
