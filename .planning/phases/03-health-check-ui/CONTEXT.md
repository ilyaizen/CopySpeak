# Phase 3: Health Check UI — Context

## Vision

Users can test their TTS engine directly from each backend configuration section on the Engine page, seeing immediate inline feedback about whether their engine is working and exactly what's wrong if it isn't.

## Design Decisions

### Test Button Placement
- **Per-backend test buttons** — Each backend section (CLI, ElevenLabs, OpenAI, HTTP) has its own "Test Engine" button
- Button only appears/active when that backend is the **currently selected backend**
- Positioned near the backend's configuration fields for contextual relevance

### Test Content
- Uses the **user's configured voice** from that backend
- Reads a **default sample phrase** ("This is a test of the text-to-speech engine.")
- Provides realistic validation — same voice/audio pipeline the app uses during clipboard triggering

### Feedback UI
- **Inline alert banner** appears in the same backend section as the test button
- Success: Green banner with checkmark and "Engine is working" message
- Failure: Red banner with error type and actionable message
- No toasts, no modals — feedback stays where user configures

### Error Messaging
- **All backend errors** get specific, human-readable messages:
  - Command not found → "kokoro-tts not found. Install CLI backend."
  - Auth failure → "Invalid API key. Check your ElevenLabs credentials."
  - Network error → "Cannot connect to OpenAI. Check your internet connection."
  - Timeout → "Request timed out. Server may be unavailable."
  - Parsing error → "Invalid response format from HTTP endpoint."
  - etc.

### Install Guidance
- **Inline install card** appears below the error when the cause is a missing dependency
- Shows copy-paste command (e.g., `pip install kokoro-tts`)
- Includes a copy button for convenience
- Only shows for CLI backends with clear install paths (kokoro-tts, piper)

## Implementation Notes

- Backend section already has local component (`cli-backend.svelte`, `elevenlabs-backend.svelte`, etc.)
- Test button should call existing `test_tts_engine` IPC command
- Result parsing needed: extract error type from `Result<(), String>` return
- Install guidance mapping: CLI backend → install commands; others → generic guidance

## Questions Resolved

✅ Where should the "Test Engine" button be? — In each backend section
✅ What should the test read aloud? — Configured voice + sample text
✅ How should test results be displayed? — Inline alert in test section
✅ Which error types need custom messaging? — All backend errors
✅ How should install guidance be displayed? — Inline install card

---
*Created: 2026-03-05*
