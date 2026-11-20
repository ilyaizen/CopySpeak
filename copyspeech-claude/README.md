# CopySpeech for Claude Code

Reads Claude's replies aloud through the running CopySpeech app. It shows a play button on every reply and marks the spoken word inside the reply. It works in the Claude desktop app and in the terminal.

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

Hover a reply and click **Read aloud** at its bottom right corner, over the end of the last line. The desktop app draws the CopySpeech mark inside the button, before the label; the terminal labels it **▶ Read aloud**. While CopySpeech reads that reply, **Pause** (or **Resume**) and **Stop** stay at that corner. **Stop** and `/copyspeech-stop` stop CopySpeech as its tray's **Stop reading** does: they also cancel every queued and running synthesis, not only this reply's. Inline `code` is read, without its backticks. Fenced code blocks and link targets are not read.

## Read a selection

In the desktop app, select text inside a reply, right-click, and choose **Read aloud with CopySpeech**. CopySpeech reads only the selected words. The terminal has no right-click menu. Needs Claude Code 2.1.326 or later.

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
      "captions": true
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
- **Show captions** – on by default. Puts a chip on the spoken word inside the reply while CopySpeech reads.

Selection reads always read in full.

## Captions

- Reads CopySpeech's `GET /status` every 120 ms while a reading plays.
- Matches the caption words to the spoken text, which leaves out skipped lines and an excerpt gap: half the words of a caption fragment must match. Across an excerpt gap, it needs a run of 3 words to jump.
- Puts a chip on the spoken word: the accent color behind it, with inverted text. A background moves no word.
- Draws the reply being read as plain text from start to stop, since Markdown cannot paint a background behind a word. In the desktop app the reply's font changes once when reading starts and back when it stops, never mid-reading. Headings show in bold, not in heading size. Fenced code blocks and tables keep their Markdown look and get no chip.
- Holds the last word between words.
- Stops 2 s after playback goes quiet, or after 60 s with no playback.

## Double playback rule

The repo's Claude Code hook (`scripts/claude-copyspeech-hook.mjs`, see [docs/agent-voice.md](../docs/agent-voice.md)) also speaks each final reply. If it is wired and **Read new replies aloud** is on, both speak. Use one of them for automatic reads.

## Limits

- The desktop app draws the mark over the button: a click on the mark itself can miss the button.
- Claude Code has no hook for the desktop app's selection bar ("Start in a side chat", "Reply", "Dictate"). **Read aloud with CopySpeech** stays in the right-click menu.
- Indented code blocks are read: Claude Code replies use indentation for nested lists too.
- The transcript does not scroll by itself to follow the spoken word.
- A reading that CopySpeech runs for other text shows no chip.

## Develop

Load the folder for one session with `claude --plugin-dir copyspeech-claude`, or name it in `CLAUDE_CODE_PLUGIN_DIRS`. Then run the commands below with the same Claude Code version that runs the session. Version 2.1.319 has no right-click menu, so it warns that the `ContextMenu` hook never runs and the menu tests fail:

```
claude plugin validate copyspeech-claude
claude plugin test copyspeech-claude
```
