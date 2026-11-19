# Browser Extension

CopySpeech Companion is a Chromium and Firefox extension for reading a text selection aloud
without leaving the current page. It works with the running Windows desktop app
through Chrome native messaging and a local named pipe.

## What it does

- Click the extension action, press `Alt+Shift+T`, or right-click the
  selection and choose **Read with CopySpeech** to read the current selection.
  Chrome applies the suggested shortcut only on a fresh install; change it at
  `chrome://extensions/shortcuts`.
- Hover the extension icon when it shows `!` for the reason a reading failed.
- Shows the selected passage and, when the engine supplies matching caption
  timings, follows the current word.
- Text that CopySpeech reads after a double-copy or its hotkey is highlighted
  too, on websites you allowed, when your current selection there is the text
  being read. Whitespace differences are ignored. A copy from another app, a
  cleared selection, or different text highlights nothing. Two tabs that hold
  the same selection both highlight. Turn this off with **Highlight when I
  double-copy** in the extension settings.
- Offers Pause, Resume, Stop, and 5-second back/forward skip controls in a
  playback box for both kinds of reading. The box is off by default; turn on
  **Show the playback box** in the extension settings.
- A reading started from the extension stops when the selected content
  changes, the page navigates, or the tab closes. A double-copy or hotkey
  reading only loses its highlight then and keeps playing.

Only ordinary, visible HTML text selections up to 65,536 UTF-16 units are
supported. Editors, PDFs, shadow DOM, and browsers without CSS Custom Highlight
support are rejected.

Local HTML and rendered Markdown files (for example, Markdown Viewer) also support
double-copy highlighting. Enable **Allow access to file URLs** on CopySpeech
Companion's details page in `chrome://extensions` or `edge://extensions`, then
reload the file tab. This browser-controlled permission is separate from saved
website grants and must be enabled for CopySpeech Companion, not just the Markdown
viewer. Highlighting still requires ordinary selectable HTML in the file tab.

## Architecture

`browser-extension/` captures the selection and owns the page UI.
`browser-native-host/` relays Chrome native-messaging frames to
`\\.\pipe\copyspeech-browser`. The desktop bridge starts a normal CopySpeech
reading and projects audible caption timings back to the original selection.

Word highlighting degrades per word and per fragment, never per reading. The
raw selection and the sanitized text are aligned word by word, so a word the
sanitizer rewrote maps to the source run it replaced and a word it invented
(`%` → "percent") maps to the punctuation it expanded. A fragment whose engine
sent no usable caption timings keeps the passage highlighted without a
current-word marker, while the fragments around it still follow the word.

## Chrome registration

Installed Windows builds register Chrome automatically. On launch, CopySpeech
writes `com.copyspeech.browser.json` next to its bundled
`copyspeech-browser-host.exe` and points
`HKCU\Software\Google\Chrome\NativeMessagingHosts\com.copyspeech.browser` at it.
Dev builds skip this step, so a repo registration is never overwritten.

The extension ID is `hojbmioeiccpgkpnpblkdbeegjkcccad`. The manifest pins it
with a `key` field, so the ID does not change with the folder.

Windows Tauri builds (`bun run tauri dev` and `bun run tauri build`) bundle
`browser-native-host/target/release/copyspeech-browser-host.exe`. Build it
first, or the Tauri build fails on the missing resource:

```
cargo build --release --manifest-path browser-native-host/Cargo.toml
```

The Windows release workflow runs this step before the Tauri build.

## Developer setup

This is currently a developer preview, not a packaged extension.

1. Build the native host from `browser-native-host/` and update
   `browser-native-host/com.copyspeech.browser.json` with its absolute `.exe`
   path. Dev builds do not write this registration for you.
2. Register that manifest for the matching unpacked Chrome extension ID.
3. Run `bun run build` in `browser-extension/`, then load
   `browser-extension/dist/` as an unpacked extension in Chrome or Edge.
4. Start CopySpeech, select ordinary page text, then use the action or shortcut.

### Firefox

The Firefox build needs Firefox 153 or later. Earlier versions lack the
document-targeted messaging and injection the companion uses.

1. Run `bun run build:firefox` in `browser-extension/`. This writes
   `browser-extension/dist-firefox/` with a Firefox manifest derived from
   `manifest.json`.
2. Open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**,
   and select `dist-firefox/manifest.json`. Temporary add-ons unload on restart.
3. On the add-on's details page, turn on **Allow access to file URLs** if you
   want double-copy highlighting in local files.
4. Register the native host for Firefox. The host manifest uses
   `allowed_extensions` with the gecko ID `copyspeech-companion@ilyaizen.github.io`.
   - Linux: `browser-native-host/install-linux.sh` writes both manifests and
     registers Firefox in `~/.mozilla/native-messaging-hosts/`.
   - Windows: point the registry default value at the Firefox manifest:
     `reg add "HKCU\Software\Mozilla\NativeMessagingHosts\com.copyspeech.browser" /ve /t REG_SZ /d "D:\Projects\CopySpeech\browser-native-host\com.copyspeech.browser.firefox.json" /f`

The gecko ID is a release identity. Changing it breaks the native host's
`allowed_extensions` and any signed build, so keep it stable.

Still open for releases: the Windows build registers Chrome only. Edge and
Firefox registration, Linux and macOS packaging, and removing the registry
entry on uninstall are not covered yet.
