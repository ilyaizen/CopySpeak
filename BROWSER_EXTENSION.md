# Browser Extension

CopySpeak Companion is a Chromium extension for reading a text selection aloud
without leaving the current page. It works with the running Windows desktop app
through Chrome native messaging and a local named pipe.

## What it does

- Click the extension action or press `Alt+Shift+R` to read the current
  selection.
- Shows the selected passage and, when the engine supplies matching caption
  timings, follows the current word.
- Provides Pause, Resume, and Stop controls in the page.
- Stops when the selected content changes, the page navigates, or the tab closes.

Only ordinary, visible HTML text selections up to 65,536 UTF-16 units are
supported. Editors, PDFs, shadow DOM, and browsers without CSS Custom Highlight
support are rejected.

## Architecture

`browser-extension/` captures the selection and owns the page UI.
`browser-native-host/` relays Chrome native-messaging frames to
`\\\\.\\pipe\\copyspeak-browser`. The desktop bridge starts a normal CopySpeak
reading and projects audible caption timings back to the original selection.

Word highlighting is deliberately conservative: it is enabled only when the
sanitized text, paginated fragments, and caption text can be mapped exactly to
the original selection. Otherwise the passage remains highlighted without a
current-word marker.

## Developer setup

This is currently a developer preview, not a packaged extension.

1. Build the native host from `browser-native-host/` and update
   `browser-native-host/com.copyspeak.browser.json` with its absolute `.exe`
   path.
2. Register that manifest for the matching unpacked Chrome extension ID.
3. Run `bun run build` in `browser-extension/`, then load
   `browser-extension/dist/` as an unpacked extension in Chrome or Edge.
4. Start CopySpeak, select ordinary page text, then use the action or shortcut.

The manifest currently embeds a development executable path and a single
extension origin. A release needs an installer that writes the browser registry
entry, ships the host binary, and binds the production extension ID.
