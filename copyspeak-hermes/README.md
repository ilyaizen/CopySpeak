# CopySpeak for Hermes Desktop

Reads each new completed assistant reply aloud through the running CopySpeak app.

Verified live in the app on 2026-11-16: the highlight moves with speech, excerpt auto-read works, and replies do not replay. The explicit-Save Settings page and follow playback are not yet verified live.

## Settings

Settings → Plugins → CopySpeak.

- **Read new replies aloud** – off by default.
- **Read only part of the reply** – on by default. Reads top, bottom, or both.
- **Paragraph counts** – 1 to 20 for top and bottom.
- **Highlight spoken words** – on by default. Highlights the word CopySpeak speaks inside the reply.
  - **Glide between words** – off by default. Shown only with highlight on. Moves one pill from word to word instead of switching the highlight.
- **Follow playback** – off by default. Scrolls the reply to keep the spoken word in view. Works with highlights off.

Changes apply only when you press **Save**. The page shows **Unsaved changes** with Save and Cancel until then. Cancel discards unsaved edits. Replies keep using the saved values until you Save.

Save writes only the fields you changed, compared with the stored settings. An edit that you change back to the saved value is dropped.

Palette command `CopySpeak: Toggle auto-read` saves at once and changes only the first setting. It keeps your unsaved edits to other settings. If a palette toggle or another window saves the same value as a pending edit, that edit is dropped. Otherwise your pending edit wins on Save.

## Captions

- Needs CopySpeak running and the reply open in the focused chat.
- Reads CopySpeak's `GET /status` every 120 ms while a reading plays, for highlights or follow. With both off, the plugin sends no status requests.
- Paints with the CSS Custom Highlight API (`::highlight(copyspeak-word)`). No Hermes source or bundle is patched. Without that API, plain highlights paint nothing; glide and follow still work.
- With glide on, a fixed white `mix-blend-mode: difference` pill moves between words in one rAF loop. It follows scroll, clips to every ancestor whose overflow hides or scrolls content and to the viewport, and hides when its word scrolls out of view.
- Finds the reply's text in the newest assistant message, skipping reasoning blocks.
- Stops 2 s after the last matched word, or after 60 s with no match. Clears on stop or on disable.

## Follow playback

- Scrolls the reply's own scroll box, found from the word's ancestors. No Hermes selector.
- Scrolls only when the spoken word leaves the middle band (15–82 % of the box). It moves the line to 34 %.
- Smooth scrolling. Reduce Motion makes the move an instant jump, even where the page sets smooth scrolling.
- Pauses for 5 s on a wheel, a scroll key, a touch anywhere in the reply, or a mouse press on the scroller's own box (its scrollbar, or background no child covers).
- A touch on a word pauses follow, because a touch can start a scroll. A mouse press on a word does not.
- Does not scroll while a text selection in the reply is active. Clearing the selection lets it resume.
- Turning Follow on or off mid-reading applies at the next status poll (about 120 ms).
- A reading runs only when highlights or follow are on as the reply starts. With both off, or with highlights unable to paint (no Highlight API, glide off) and follow off, no reading runs. Turning either on then applies from the next reply.

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
