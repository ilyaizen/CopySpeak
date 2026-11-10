# CopySpeak read aloud

A DSH plugin that reads assistant replies aloud through the CopySpeak desktop app.

## What you get

- A speaker action in the action row of each finished assistant message.
- An auto-read toggle at the DSH sidebar foot: "Read replies aloud / On / Off".
- Clicking the speaker action speaks that message. Clicking it again stops the reading.
- With auto-read on, each new reply in the open session is read when it settles.
- The action returns to the speaker state when playback ends.

## Requirements

- CopySpeak runs on this machine with its control server on `127.0.0.1:43117` (the default).
- The reading uses the app's active voice profile.

## How it works

Two browser halves share one transport to the CopySpeak control server:

```
POST http://127.0.0.1:43117/speak
{"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
```

The request uses `mode: 'no-cors'` and a `text/plain` content type. The browser then sends no CORS preflight, and CopySpeak parses the JSON body without inspecting the content type. The response is opaque, so the plugin can tell that the request reached CopySpeak but cannot read a CopySpeak error message. `wait: true` makes the response arrive when playback ends, which restores the idle action state.

Stop uses `POST /stop {"request_id": "<id>"}`, so the plugin cancels exactly its own reading and leaves other integrations alone.

Slots:

- `conversation.chat.assistant-actions` – the per-message speaker action.
- `sidebar.footer.action` – the auto-read toggle beside Settings.
- `conversation.composer.dock` – a null-rendering per-session watcher that speaks each newly settled reply.

The auto-read preference lives in the page's `localStorage` under `dsh.copyspeak-voice.auto-read`.

## Icon

The plugin mark is a silhouette of the CopySpeak app logo: the speaker and its two sound waves with the gradient card and layer colors removed. The in-UI glyph uses `currentColor`, so it follows light and dark themes. The plugin-list icon uses one neutral gray fill because the Plugin Manager renders it as an `<img>`.

## Limits

- The control server answers no CORS preflight, so the plugin never learns why CopySpeak rejected a request. A silent reading usually means a CopySpeak configuration problem (voice profile, engine credential).
- CopySpeak truncates the text to its configured `max_text_length`.
- Code blocks are removed from the spoken text.
- Auto-read starts at the next settled reply. Enabling it never replays old replies.
- The auto-read preference and playback state are per page tab.

## Files

- `package.json` – bundle manifest, `dsh.bundle` patch, `dsh.client` declaration.
- `cordis.patch.yml` – inserts the `copyspeak-voice` Loader row.
- `index.js` – no-op host half, keeps the bundle row loadable.
- `client.js` – the browser half: glyph, auto-read store, CopySpeak transport, and three slot registrations.
- `locale/en.json` – Plugin Manager title and description.
- `icon.svg` – the gray silhouette of the CopySpeak mark.

## Install

```
plugin_manager install_bundle D:\Projects\CopySpeak\copyspeak-dsh
```

The profile links this directory, so a saved `client.js` reaches an open page through Client HMR.
