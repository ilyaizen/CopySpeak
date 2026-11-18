# CopySpeech for Hermes Desktop

Reads each new completed assistant reply aloud through the running CopySpeech app, and gives you playback controls: a play button on every reply, a Read aloud pill on any text you select inside a reply, and a playback bar above the chat input.

Verified live in the app on 2026-11-16: the highlight moves with speech, excerpt auto-read works, and replies do not replay. The explicit-Save Settings page and follow playback are not yet verified live. The playback controls (message play buttons, selection pill, bar) are new and not yet verified live.

## Play a reply

Hover a reply and click the speaker button in its action row, beside Copy and Read aloud. While CopySpeech reads that reply, the button shows a stop square and clicking it stops the reading. The button reads the reply's rendered text; code blocks and reasoning are not read.

## Read aloud on selection

Select text inside a reply and a small **Read aloud** pill appears above the selection. Click it and CopySpeech reads only the selected words. The caption highlight keeps painting inside the former selection. Escape, a scroll, or clearing the selection removes the pill.

## Playback bar

While a reading this plugin started is playing, a small bar sits at the top left of the chat input, inside the composer outline:

- **Pause / resume** the reading.
- **Back 5 seconds** and **forward 5 seconds**.
- **Stop** the reading.
- **Show the spoken word**: scrolls the reply so the spoken line sits in view.

The bar works for automatic reads too, and it follows a pause made in the CopySpeech window. The palette command **CopySpeech: Stop reading** stops the current reading.

## Settings

Settings → Plugins → CopySpeech.

- **Read new replies aloud** – off by default.
- **Skip bullet points and sections** – off by default. Automatic and play-button reads skip headings, bullet and numbered items, horizontal rules, and table rows. The text under a heading still reads. Selection reads always read in full.
- **Read only part of the reply** – on by default. Reads top, bottom, or both. Automatic and play-button reads follow it. Selection reads always read in full.
- **Paragraph counts** – 1 to 20 for top and bottom.
- **Highlight spoken words** – on by default. Highlights the word CopySpeech speaks inside the reply. Works for automatic reads, play-button reads, and selection reads.
  - **Glide between words** – off by default. Shown only with highlight on. Moves one pill from word to word instead of switching the highlight.
- **Follow playback** – off by default. Scrolls the reply to keep the spoken word in view. Works with highlights off.
- **Show play buttons on replies** – on by default.
- **Read aloud on selection** – on by default.

Changes apply only when you press **Save**. The page shows **Unsaved changes** with Save and Cancel until then. Cancel discards unsaved edits. Replies keep using the saved values until you Save.

Save writes only the fields you changed, compared with the stored settings. An edit that you change back to the saved value is dropped.

Palette command `CopySpeech: Toggle auto-read` saves at once and changes only the first setting. It keeps your unsaved edits to other settings. If a palette toggle or another window saves the same value as a pending edit, that edit is dropped. Otherwise your pending edit wins on Save.

## Captions

- Needs CopySpeech running and the reply open in the focused chat.
- Reads CopySpeech's `GET /status` every 120 ms while a reading plays, for highlights, follow, or the bar. With all off and no reading, the plugin sends no status requests.
- Paints with the CSS Custom Highlight API (`::highlight(copyspeech-word)`). No Hermes source or bundle is patched. Without that API, plain highlights paint nothing; glide and follow still work.
- With glide on, a fixed white `mix-blend-mode: difference` pill moves between words in one rAF loop. It follows scroll, clips to every ancestor whose overflow hides or scrolls content and to the viewport, and hides when its word scrolls out of view.
- A play-button read paints inside that reply. A selection read paints inside the former selection. An automatic read paints inside the newest assistant reply.
- Finds the reply's text in the assistant message, skipping reasoning blocks and code blocks.
- Stops 2 s after the last matched word, or after 60 s with no match. Clears on stop or on disable.

## Follow playback

- Scrolls the reply's own scroll box, found from the word's ancestors. No Hermes selector.
- Scrolls only when the spoken word leaves the middle band (15–82 % of the box). It moves the line to 34 %.
- Smooth scrolling. Reduce Motion makes the move an instant jump, even where the page sets smooth scrolling.
- Pauses for 5 s on a wheel, a scroll key, a touch anywhere in the reply, or a mouse press on the scroller's own box (its scrollbar, or background no child covers).
- A touch on a word pauses follow, because a touch can start a scroll. A mouse press on a word does not.
- Does not scroll while a text selection in the reply is active. Clearing the selection lets it resume.
- Turning Follow on or off mid-reading applies at the next status poll (about 120 ms).

## Install

Create a junction so the app loads the repo file in place:

```
New-Item -ItemType Junction -Path "$env:LOCALAPPDATA\hermes\desktop-plugins\copyspeech-hermes" -Target "D:\Projects\CopySpeech\copyspeech-hermes"
```

Edits to `plugin.js` reload in the app. Load errors appear in `desktop.log` as `runtime load failed (copyspeech-hermes)`.

## Double playback rule

If Hermes's own "Read replies aloud" is on, both readers speak. The plugin skips every automatic reply while that setting is on. Turn the built-in setting off to use this plugin. The built-in per-message **Read aloud** button plays through Hermes's own ladder, not CopySpeech's controls; the plugin's speaker button beside it plays through CopySpeech.

## Limits

- Captions match on text. A reading that CopySpeech runs for other text paints nothing.
- Two windows on one chat may race on a claim. The window is small.
- CopySpeech must run. If it does not, the plugin shows one error toast.
- A reply that scrolls far out of view can unmount; its captions then stop while the audio continues. Follow playback keeps the reply mounted.
- The built-in Read aloud button and a plugin reading can overlap, because Hermes cannot see CopySpeech's playback. CopySpeech queues them.
