# CopySpeak

A lightweight Windows app that reads copied text aloud using local or cloud
text-to-speech engines. Copy the same text twice in quick succession to listen
without leaving the app you're working in.

- **Purpose:** Make listening to text as easy as copying it.
- **Workflow:** Double-copy, use a global hotkey, or paste text manually; CopySpeak
  cleans the text and speaks it with your chosen engine and voice.
- **Controls:** Voice profiles, adjustable playback, a floating HUD, and history
  for replaying previous readings.
- **Optional:** AI rewriting makes copied text more concise and listener-friendly.
- **Foundation:** Svelte 5 and TypeScript UI, Rust backend, Tauri 2 desktop shell.

The priority is reliable triggering and fast speech, with the app staying out of
the way in the system tray.
