# CopySpeech read aloud

A DeepSeek Harness plugin that reads assistant replies aloud through the running CopySpeech desktop app.

## Features

- **Sidebar panel**: Read new replies aloud, captions, highlight animation, follow playback, and CopySpeech status.
- **Message action**: reads one reply aloud. Click again to stop.
- **Selection read**: a Read aloud pill beside Quote in a settled reply.
- **Playback bar**: pause or resume, rewind 5 seconds, skip 5 seconds, stop, and show the spoken line.
- **Captions**: highlights the spoken word inside the reply when CopySpeech reports timings.
- **Partial auto-read** (experimental, 0.1.1): reads only the first paragraphs, the last paragraphs, or both ends of each automatic reply. The default is 2 paragraphs from the top and 2 from the bottom.

## Settings

Settings live on the **CopySpeech sidebar panel** only. There is no CopySpeech page in DSH Settings.

Open the panel from the sidebar. When **Read new replies aloud** is on, **Read only part of the reply** appears. Turn it on, choose **Top**, **Bottom**, or **Top and bottom**, and set 1–20 paragraphs for each end. This limit applies to automatic reading only. The message action still reads the whole reply.

## Requirements

- CopySpeech runs on this machine with its control server on `127.0.0.1:43117`.
- Captions need a CopySpeech build with `GET /status` and an engine that reports timings.
- Pause and seek need `POST /pause`, `POST /resume`, and `POST /seek`.

## Install

1. Call `plugin_manager` with `action: install_bundle` and this directory's absolute path:

   ```
   target: D:\Projects\CopySpeech\copyspeak-dsh
   ```

2. Open **Plugins** and check that **CopySpeech read aloud** is installed.
3. Open the CopySpeech panel from the sidebar. The status line reads **CopySpeech is running.**
4. Send a message and click the speaker in the reply's action row.

The install writes to the active profile, so it needs `danger-full-access` or one approval.

## Limits

- Auto-read starts at the next completed turn. It never replays old turns.
- Code blocks are removed from the spoken text. A selection read speaks the rendered text, including code.
- Most control routes send no CORS headers, so the plugin cannot read CopySpeech error messages. A silent read usually means a CopySpeech voice or engine problem.
- The switches are per page tab.
