# CopySpeech

A lightweight Windows app that reads copied text aloud using local or cloud
text-to-speech engines. Copy the same text twice in quick succession to listen
without leaving the app you're working in.

- **Purpose:** Make listening to text as easy as copying it.
- **Workflow:** Double-copy, use a global hotkey, or paste text manually; CopySpeech
  cleans the text and speaks it with your chosen engine and voice.
- **Controls:** Voice profiles, adjustable playback, a floating HUD, and history
  for replaying previous readings.
- **Optional:** AI rewriting makes copied text more concise and listener-friendly.
- **Foundation:** Svelte 5 and TypeScript UI, Rust backend, Tauri 2 desktop shell.

The priority is reliable triggering and fast speech, with the app staying out of
the way in the system tray.

## Why the agent integrations exist

Coding agents write long, verbose output. Many people do not read it. They say
"go all the way" and accept the result. Unread output hides wrong assumptions,
and that becomes technical debt.

CopySpeech helps people follow and review what an agent says. Listening is easier
than reading a long reply. The Hermes, DSH, and Claude Code plugins put reading
aloud inside the agent UI. The goal is comprehension and good review practice,
not convenience alone.

What this means for the plugins:

- **Stay calm.** The UI must not distract from the text. It must not change layout
  or swap text while a reply is read.
- **Follow along.** Highlight the spoken word, so the listener keeps their place.
- **Keep the text readable.** The reply is what the user must understand.
  Controls stay small and out of the way.
