# Milestones

## v0.1 TTS Engine Overhaul (Shipped: 2026-03-26)

**Phases completed:** 1 phase, 4 plans, 8 tasks

**Key accomplishments:**

- HTTP engine removed — deleted http.rs, HttpTtsConfig, HTTP backend option; automatic config migration to 'local'
- CLI engines consolidated — piper, kokoro-tts, qwen3-tts as official presets; others hard-deleted
- Two-stage health checks — "Check API Key" (credential-only) + "Test Engine" (full synthesis) for ElevenLabs and OpenAI
- OpenAI complete voice list — 9 official voices in dropdown (alloy, ash, coral, echo, fable, nova, onyx, shimmer, verse)
- ElevenLabs voice dropdown — fetch from API, no raw ID input fallback
- HTTP dead code purged — orphaned tts-settings.svelte cleaned, requirement traceability added

**Known Gaps:**

- ENG-02: CLI preset selection doesn't auto-apply command/args (integration gap)
- OBD-02, OBD-03: Nav tabs visible during onboarding (integration gap — escape path without saving config)

---
