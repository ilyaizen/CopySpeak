# CopySpeak read aloud

A DSH plugin that reads assistant replies aloud through the CopySpeak desktop app.

## What you get

- A **CopySpeak** panel in the sidebar, beside Plugins and Automation tasks:
  - **Read new replies aloud** reads each reply in the open session when it settles.
  - **Show captions while reading** highlights the word CopySpeak plays inside the reply text. It is off by default.
  - A status line tells you whether the CopySpeak app is running.
- A speaker action in the action row of each finished assistant message.
- Clicking the speaker action speaks that message. Clicking it again stops the reading.
- The action returns to the speaker state when playback ends.

## Requirements

- CopySpeak runs on this machine with its control server on `127.0.0.1:43117` (the default).
- The reading uses the app's active voice profile.
- Captions need a CopySpeak build with `GET /status` and an engine that reports caption timings. Timings may be word-level or character-level (ElevenLabs sends one per character); the plugin groups them under the rendered words. Older builds, or an engine without timings, read aloud as usual and show no highlight.

## How it works

Every surface shares one transport to the CopySpeak control server:

```
POST http://127.0.0.1:43117/speak
{"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
```

The request uses `mode: 'no-cors'` and a `text/plain` content type. The browser then sends no CORS preflight, and CopySpeak parses the JSON body without inspecting the content type. The response is opaque, so the plugin can tell that the request reached CopySpeak but cannot read a CopySpeak error message. `wait: true` makes the response arrive when playback ends, which restores the idle action state.

Stop uses `POST /stop {"request_id": "<id>"}`, so the plugin cancels exactly its own reading and leaves other integrations alone.

Captions poll one read-only route while a reading runs:

```
GET http://127.0.0.1:43117/status
{"active": true, "captions": {"text": "...", "words": [{"text_start": 0, "text_end": 4, "start_ms": 0, "end_ms": 320}]}, "position_ms": 120, "paused": false}
```

CopySpeak reports the caption and word position it already computes for its own UI, at most once per 100 ms. The route is CORS-open and read-only. The plugin polls it only while its own reading plays, stops when the route never answers, and removes the highlight when the snapshot goes inactive. Provider timings are not always words: ElevenLabs reports one entry per character, so the plugin groups entries under the whitespace-delimited words of `captions.text` before aligning them to the rendered message.

The highlight paints inside the message, not in a separate strip:

- The Chat view marks each settled message body with `data-text-interaction-source` and each row with `data-chat-node-key`, so the plugin finds the exact reply the action or the watcher asked for.
- Caption word offsets index the sanitized text CopySpeak spoke, not the rendered Markdown. For each fragment the plugin aligns the fragment's words onto the rendered text nodes, then paints the active word with the CSS Custom Highlight API (`window.CSS.highlights`, `::highlight(copyspeak-read-word)`).
- A word that leaves the middle band of the chat scrollport scrolls the chat smoothly to it, at most once per 250 ms.

Slots:

- `sidebar.panellist` – the CopySpeak icon in the sidebar, id `copyspeak`.
- `main` – the CopySpeak panel, key `copyspeak`.
- `conversation.chat.assistant-actions` – the per-message speaker action.
- `conversation.composer.dock` – a null-rendering per-session watcher that speaks each newly settled reply and runs the caption highlight.

The two switches live in the page's `localStorage` under `dsh.copyspeak-voice.auto-read` and `dsh.copyspeak-voice.captions`. Both are off when the key is absent.

## Icon

The plugin mark is a silhouette of the CopySpeak app logo: the speaker and its two sound waves with the gradient card and layer colors removed. The in-UI glyph uses `currentColor`, so it follows light and dark themes. The plugin-list icon uses one neutral gray fill because the Plugin Manager renders it as an `<img>`.

## Limits

- The control server answers no CORS preflight, so the plugin never learns why CopySpeak rejected a request. A silent reading usually means a CopySpeak configuration problem (voice profile, engine credential).
- CopySpeak truncates the text to its configured `max_text_length`.
- Code blocks are removed from the spoken text.
- The highlight paints only while the reply is on screen and only while `/status` answers. It also stays hidden for a fragment whose words cannot be found in the rendered text, such as a number the sanitizer spells out or a reply in a spaceless script where the spoken words and the rendered words tokenize differently.
- Auto-read starts at the next settled reply. Enabling it never replays old replies.
- The switches and playback state are per page tab.

## Files

- `package.json` – bundle manifest, `dsh.bundle` patch, `dsh.client` declaration.
- `cordis.patch.yml` – inserts the `copyspeak-voice` Loader row.
- `index.js` – no-op host half, keeps the bundle row loadable.
- `client.js` – the browser half: glyph, preferences, CopySpeak transport, caption alignment and highlight, and four slot registrations.
- `locale/en.json` – Plugin Manager title and description.
- `icon.svg` – the gray silhouette of the CopySpeak mark.

## Install

```
plugin_manager install_bundle D:\Projects\CopySpeak\copyspeak-dsh
```

The profile links this directory, so a saved `client.js` reaches an open page through Client HMR. A host-half change needs a DSH restart.
