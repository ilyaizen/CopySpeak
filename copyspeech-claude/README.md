# CopySpeech for Claude Code

Reads Claude's replies aloud through the running CopySpeech app. It shows a play button on every reply and marks the spoken word in a captions pane. It works in the Claude desktop app and in the terminal.

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

Hover a reply and click the play button at its bottom right corner, over the end of the last line. It reads **Read excerpt** while either **Skip bullet points and sections** or **Read only part of the reply** is on (both are on by default), and **Read aloud** when both are off. The desktop app draws the CopySpeech mark inside the button, before the label; the terminal labels it **▶ Read excerpt**. While CopySpeech reads, a row above the prompt shows **Pause** (or **Resume**) and **Stop** for that reading. **Stop** and `/copyspeech-stop` stop CopySpeech as its tray's **Stop reading** does: they also cancel every queued and running synthesis, not only this reply's. Inline `code` is read, without its backticks. A dot between names in it is read as "dot": `export.ts` is read as export dot ts. Fenced code blocks and link targets are not read.

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
- **Show captions** – on by default. Opens the captions pane while CopySpeech reads.

Selection reads always read in full.

## Captions

- Reads CopySpeech's `GET /status` every 120 ms while a reading plays.
- Matches the caption words to the spoken text, which leaves out skipped lines and an excerpt gap: half the words of a caption fragment must match. Across an excerpt gap, it needs a run of 3 words to jump.
- Opens a **CopySpeech captions** pane when a reading starts and closes it when the reading stops. Replies keep Claude Code's own Markdown look.
- Shows the reading's replies in the pane as plain text, with a chip on the spoken word in reverse video: the text color behind it, with the background color over it. Headings show in bold, not in heading size. Fenced code blocks and tables show their raw text and get no chip.
- Scrolls the pane to center the paragraph that holds the spoken word. A paragraph taller than the pane shows its top.
- The desktop app docks the pane beside the chat. The terminal docks it beside a fullscreen transcript from 110 columns, else shows it above the prompt. A pane that auto-read opens waits for 144 columns.
- Holds the last word between words.
- Stops 2 s after playback goes quiet, or after 60 s with no playback.

## Double playback rule

The repo's Claude Code hook (`scripts/claude-copyspeech-hook.mjs`, see [docs/agent-voice.md](../docs/agent-voice.md)) also speaks each final reply. If it is wired and **Read new replies aloud** is on, both speak. Use one of them for automatic reads.

## Limits

- The desktop app draws the mark over the button: a click on the mark itself can miss the button.
- Claude Code has no hook for the desktop app's selection bar ("Start in a side chat", "Reply", "Dictate"). **Read aloud with CopySpeech** stays in the right-click menu.
- Indented code blocks are read: Claude Code replies use indentation for nested lists too.
- The transcript does not scroll by itself to follow the spoken word. The captions pane does.
- A reading that CopySpeech runs for other text shows no chip.

## Develop

Load the folder for one session with `claude --plugin-dir copyspeech-claude`, or name it in `CLAUDE_CODE_PLUGIN_DIRS`. Then run the commands below with the same Claude Code version that runs the session. Version 2.1.319 has no right-click menu, so it warns that the `ContextMenu` hook never runs and the menu tests fail:

```
claude plugin validate copyspeech-claude
claude plugin test copyspeech-claude
```
