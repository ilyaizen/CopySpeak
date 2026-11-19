# CopySpeech for Claude Code

Reads Claude's replies aloud through the running CopySpeech app. It shows the spoken word inside the reply, a play button on every reply, and a playback bar above the prompt. It works in the Claude desktop app and in the terminal.

## Install

CopySpeech must run, with its control server on `http://127.0.0.1:43117`.

At the prompt of a terminal session:

```
/plugin install copyspeech --marketplace ilyaizen/CopySpeech
```

From a shell:

```
claude plugin install copyspeech --marketplace ilyaizen/CopySpeech
```

In the desktop app, where `/plugin` does not run: click **+** next to the prompt box, then **Plugins**, **Add marketplace** with `ilyaizen/CopySpeech`, then **Add plugin**.

## Play a reply

Hover a reply and click **▶ Read aloud** under it. While CopySpeech reads that reply, the button shows **■ Stop reading**. Code blocks and link targets are not read.

## Read a selection

Select text inside a reply, right-click, and choose **Read aloud with CopySpeech**. CopySpeech reads only the selected words. Not yet verified live.

## Playback bar

While a reading this plugin started plays, a bar sits above the prompt:

- **Pause / Resume** the reading.
- **−5s** and **+5s** seek.
- **■ Stop** the reading.
- **Show spoken word** scrolls the reply that holds the spoken word into view.

`/copyspeech-stop` also stops the reading.

## Settings

Change the settings in one of three ways:

- **Terminal:** run `claude`, type `/config`, and find the CopySpeech rows. A change applies at once.
- **Any surface, auto-read only:** type `/copyspeech-autoread`. It turns **Read new replies aloud** on or off and saves it, as `/config` does.
- **Desktop app:** the Code tab has no settings screen for plugin options. Add a `pluginConfigs` block to `~/.claude/settings.json`:

```json
"pluginConfigs": {
  "copyspeech": {
    "options": {
      "autoRead": false,
      "skipSections": true,
      "excerpt": true,
      "excerptMode": "both",
      "excerptTop": 2,
      "excerptBottom": 2,
      "highlight": "chip"
    }
  }
}
```

The key is the plugin's name as Claude Code loaded it: `copyspeech` or `copyspeech@inline` for a folder loaded with `--plugin-dir`, else the installed plugin's name. A setting you leave out keeps its default.

- **Read new replies aloud** – off by default. Reads every text block of the finished turn as one reading.
- **Skip bullet points and sections** – on by default. Automatic and play-button reads skip headings, bullet and numbered items, horizontal rules, and table rows. The text under a heading still reads. A reply made only of those lines reads nothing.
- **Read only part of the reply** – on by default. Automatic and play-button reads keep the top and bottom paragraphs.
- **Part to read** – both (default), top, or bottom.
- **Top paragraphs** and **Bottom paragraphs** – 1 to 20, 2 by default.
- **Spoken-word highlight** – how the spoken word shows inside the reply:
  - `chip` (default): a tinted background behind the word. The desktop app paints the accent at 30 % behind the word; the terminal paints a solid accent chip.
  - `code`: the word becomes inline code. Later words can shift, because code uses another font.
  - `bold`: the word becomes bold.
  - `off`: no highlight.

Selection reads always read in full.

## Captions

- Reads CopySpeech's `GET /status` every 120 ms while a reading plays.
- Matches the caption words to the reply text: half the words of a caption fragment must match. Across an excerpt gap, it needs a run of 3 words to jump.
- Holds the last word between words.
- Stops 2 s after playback goes quiet, or after 60 s with no playback.
- With `chip`, the reply draws as plain text spans for the whole reading, so the layout changes once at the start and once at the stop. Code blocks and tables keep their normal look.

## Double playback rule

The repo's Claude Code hook (`scripts/claude-copyspeech-hook.mjs`, see [docs/agent-voice.md](../docs/agent-voice.md)) also speaks each final reply. If it is wired and **Read new replies aloud** is on, both speak. Use one of them for automatic reads.

## Limits

- The transcript does not scroll by itself to follow the spoken word. Use **Show spoken word**.
- There is no gliding pill between words.
- Chip corners are square.
- A reading that CopySpeech runs for other text paints nothing.

## Develop

Load the folder for one session with `claude --plugin-dir copyspeech-claude`, or name it in `CLAUDE_CODE_PLUGIN_DIRS`. Then run:

```
claude plugin validate copyspeech-claude
claude plugin test copyspeech-claude
```
