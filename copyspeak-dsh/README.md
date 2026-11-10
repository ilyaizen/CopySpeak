# CopySpeak read aloud

A DSH plugin that reads assistant replies aloud through the CopySpeak desktop app.

## What you get

- A **CopySpeak** panel in the sidebar, beside Plugins and Automation tasks:
  - **Read new replies aloud** reads the final reply of each turn in the open session. Intermediate steps of a tool-calling turn are read once, at the end.
  - **Show captions while reading** highlights the word CopySpeak plays inside the reply text. It is off by default.
  - **Follow playback** scrolls the chat to keep the spoken word in view. It is on by default.
  - A status line tells you whether the CopySpeak app is running.
- A CopySpeak action in the completed turn's action row.
- The action shows an outlined speaker when idle. A tooltip reads **Read aloud with CopySpeak**.
- Clicking the action speaks that message.
- While the message plays, the action shows the filled speaker in the theme's brand color. Its tooltip reads **Stop reading**.
- Clicking the action while it plays stops the reading.
- When playback ends, the action returns to the outlined speaker.
- A floating playback bar in the upper right corner, below the session header's bottom border, while a reply plays. It starts with the colored CopySpeak app logo. Every control is an icon with a tooltip and an accessible label:
  - **Pause** stops the audio at the current word. The control then shows **Resume**.
  - **-5 s** rewinds 5 seconds. **+5 s** skips forward 5 seconds. Both controls work while paused.
  - **Stop** cancels the reading from any page.
  - **Show in chat** scrolls the chat back to the spoken word and resumes follow. Follow lives in the side panel only, so with captions off this control is the only way back to the spoken line.

## Requirements

- CopySpeak runs on this machine with its control server on `127.0.0.1:43117` (the default).
- The reading uses the app's active voice profile.
- Captions need a CopySpeak build with `GET /status` and an engine that reports caption timings. Timings may be word-level or character-level (ElevenLabs sends one per character); the plugin groups them under the rendered words. Older builds, or an engine without timings, read aloud as usual and show no highlight.
- The pause and seek controls need a CopySpeak build with `POST /pause`, `POST /resume`, and `POST /seek`. An older build ignores these routes.

## How it works

Every surface shares one transport to the CopySpeak control server:

```
POST http://127.0.0.1:43117/speak
{"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
```

The request uses `mode: 'no-cors'` and a `text/plain` content type. The browser then sends no CORS preflight, and CopySpeak parses the JSON body without inspecting the content type. The response is opaque, so the plugin can tell that the request reached CopySpeak but cannot read a CopySpeak error message. `wait: true` makes the response arrive when playback ends, which restores the idle action state.

Stop uses `POST /stop {"request_id": "<id>"}`, so the plugin cancels exactly its own reading and leaves other integrations alone.

Pause, resume, and seek use the same no-cors `text/plain` path and the same `request_id`, so they also change only this plugin's reading:

```
POST http://127.0.0.1:43117/pause  {"request_id": "<id>"}
POST http://127.0.0.1:43117/resume {"request_id": "<id>"}
POST http://127.0.0.1:43117/seek   {"request_id": "<id>", "delta_ms": -5000}
```

The bar sends `-5000` for **-5 s** and `5000` for **+5 s**. A paused reading keeps its `wait: true` request open. The request resolves when the reading resumes and ends, or when the reading stops.

Auto-read speaks one reply per turn. The host writes one `turn-tail` node when a turn ends, and its `closing` step is the turn's final assistant reply. The watcher speaks that step, so it skips intermediate steps and never reads a reply halfway through a turn.

Captions and the bar poll one read-only route while a reading runs:

```
GET http://127.0.0.1:43117/status
{"active": true, "captions": {"text": "...", "words": [{"text_start": 0, "text_end": 4, "start_ms": 0, "end_ms": 320}]}, "position_ms": 120, "duration_ms": 3200, "paused": false}
```

CopySpeak reports the caption, the word position, and the pause state it already computes for its own UI, at most once per 100 ms. The route is CORS-open and read-only. The plugin polls it for the whole reading, also with captions off: one snapshot drives the inline highlight, follow, Show in chat, and the bar's pause state. While paused the app keeps `active: true` and a frozen position. The poll stops when the route never answers, and an inactive snapshot removes the highlight. Provider timings are not always words: ElevenLabs reports one entry per character, so the plugin groups entries under the whitespace-delimited words of `captions.text` before aligning them to the rendered message.

The bar shows the app's pause state. A click flips the glyph at once, and the next snapshot confirms it. The app stays the source of truth, because it can also toggle pause from its own window.

The highlight paints inside the message, not in a separate strip:

- The Chat view marks each settled message body with `data-text-interaction-source` and each row with `data-chat-node-key`, so the plugin finds the exact reply the action or the watcher asked for.
- Caption word offsets index the sanitized text CopySpeak spoke, not the rendered Markdown. For each fragment the plugin aligns the fragment's words onto the rendered text nodes, then paints the active word with the CSS Custom Highlight API (`window.CSS.highlights`, `::highlight(copyspeak-read-word)`). The API cannot round a background, so a positioned layer inside the message paints a rounded brand mark behind the word and recolors the word's text through the highlight.
- Provider word windows can have gaps between words, and ElevenLabs reports one timing per character. The plugin holds the last painted word across a gap until the next word paints. Only an inactive snapshot, a same-fragment rewind, a reset, or the end of the reading clears the mark.
- Follow playback uses a wide band. When the spoken word leaves it, its line moves to the upper third of the scrollport, at most once per 400 ms.
- A manual wheel, touch, or key scroll pauses following for 5 s, so the view does not fight the reading.
- The bar's **Show in chat** control scrolls to the spoken word at once and resumes following.

The caption position counts inside one spoken fragment. A normal fragment change starts the count again, so the plugin aligns forward from its last matched word. A position that drops by more than 300 ms on the same fragment is a rewind: the plugin aligns again from the start of the message, so the highlight and follow find the spoken word again.

Slots:

- `sidebar.panellist` – the CopySpeak icon in the sidebar, id `copyspeak`.
- `main` – the CopySpeak panel, key `copyspeak`.
- `conversation.chat.assistant-actions` – the speaker action in the completed turn's action row.
- `conversation.composer.dock` – a null-rendering per-session watcher that speaks the final reply of each completed turn and runs the caption highlight.
- `shell.overlay` – the floating playback bar, id `copyspeak-playback`.

The switches live in the page's `localStorage` under `dsh.copyspeak-voice.auto-read`, `dsh.copyspeak-voice.captions`, and `dsh.copyspeak-voice.follow`. Auto-read and captions are off when their key is absent; follow is on.

## Icon

The sidebar panel icon is the outlined read mark that the message action shows while idle: a speaker with two waves in `currentColor`, so it follows light and dark themes. The action shows the filled speaker in the brand color while the message plays. `icon.svg` is the colored app-logo mark for the Plugin Manager, which renders it as an `<img>`. It shows a tilted rounded box with a transparent face, a speaker, and sound waves in a mint and teal palette. The playback bar embeds that same colored logo at its head as a data URI, so it needs no asset build step.

## Limits

- The control server answers no CORS preflight, so the plugin never learns why CopySpeak rejected a request. A silent reading usually means a CopySpeak configuration problem (voice profile, engine credential).
- CopySpeak truncates the text to its configured `max_text_length`.
- Code blocks are removed from the spoken text.
- The highlight paints only while the reply is on screen and only while `/status` answers. It also stays hidden for a fragment whose words cannot be found in the rendered text, such as a number the sanitizer spells out or a reply in a spaceless script where the spoken words and the rendered words tokenize differently.
- Auto-read starts at the next completed turn. Enabling it never replays old turns.
- A paused reading keeps the `wait: true` request open. The action row keeps its Stop glyph, and auto-read waits until the reading resumes and ends. Stop ends the reading at once.
- An app older than the pause and seek routes ignores them. The pause glyph then reverts at the next snapshot, and a seek does nothing.
- Seek stays enabled while the reading is paused. A reading resumed after a seek plays from the new position.
- Without `/status`, the floating bar still stops and scrolls. Pause and seek have no app state to show, and the highlight stays hidden.
- The switches, the pause state, and the playback state are per page tab.

## Files

- `package.json` – bundle manifest, `dsh.bundle` patch, `dsh.client` declaration.
- `cordis.patch.yml` – inserts the `copyspeak-voice` Loader row.
- `index.js` – no-op host half, keeps the bundle row loadable.
- `client.js` – the browser half: panel and action glyphs, preferences, CopySpeak transport and pause/seek controls, caption alignment and highlight, follow scrolling, the icon-only playback bar, and five slot registrations.
- `locale/en.json` – Plugin Manager title and description.
- `icon.svg` – the colored app-logo mark for the Plugin Manager, rendered as an `<img>`.

## Install

The install adds the bundle `@local/copyspeak-voice` to the active DSH profile and links this directory. The profile reads the files directly, so later edits reach the running app. The plugin stays installed across DSH restarts.

### Before you install

- CopySpeak is installed and running on this machine. See [Requirements](#requirements).
- The session uses the DSH profile that gets the plugin. An install from another profile puts the plugin there.

### Install the bundle

1. Call the `plugin_manager` tool with `action: install_bundle` and the absolute path of this directory:

   ```
   target: D:\Projects\CopySpeak\copyspeak-dsh
   ```

   Use the path of the checkout that holds `package.json`. On another machine, that path differs.

2. Read the result. `application: applied` means the plugin runs now. A warning or a pending install script needs a decision before you continue.
3. Open Plugins and find **CopySpeak read aloud** in the Installed list. The card shows the colored app logo as its icon.
4. Look for the CopySpeak icon in the sidebar. Open the panel. The status line reads **CopySpeak is running.**
5. Send a message, then click the speaker in its action row. CopySpeak reads the reply.

The tool call needs the `danger-full-access` preset, or one approval, because the install writes outside this directory.
