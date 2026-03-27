# Feature Research

**Domain:** TTS engine configuration page + desktop app startup health check UX
**Researched:** 2026-03-04
**Confidence:** MEDIUM (patterns drawn from analogous products: Home Assistant TTS, AllTalk TTS, VS Code language servers, audio DAW plugin UX — no single authoritative source covers this exact niche)

---

## Feature Landscape

### Table Stakes (Users Expect These)

These are features users assume any engine-configuration UI provides. Missing them makes the Engine page feel half-built or untrustworthy.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Backend selector (CLI / ElevenLabs / OpenAI / HTTP) | Users need to switch engines; this is the page's reason for being | LOW | Already exists in Settings; this is a move + polish operation |
| Credential / command fields per backend | Every credential-bearing tool shows a form for keys, paths, URLs | LOW | Already in Settings; needs relocation and per-backend scoping |
| Live "Test Engine" button with pass/fail feedback | Audio config tools universally offer a playback test before committing | MEDIUM | `health_check()` trait exists in Rust; need frontend to surface result |
| Specific error diagnosis (command not found vs auth failure vs bad voice) | Generic "failed" is unacceptable; users need to know what to fix | MEDIUM | `TtsError` variants map directly: `CommandNotFound`, `CommandFailed`, `InvalidWav` |
| Voice name / ID entry field | Users must be able to set which voice the engine uses | LOW | Exists; just needs a dedicated home on Engine page |
| Speed slider with numeric display | Voice speed is a primary quality knob; always expected on a TTS config page | LOW | Exists; relocation only |
| Inline installation guidance when engine is missing | Tools like VS Code extensions and AllTalk TTS show setup help when a dependency is not found | MEDIUM | Currently backlogged as "TTS Engine Quick Install Guide" — must move to P1 for this milestone |
| Startup health check that surfaces broken state without blocking the app | App must not silently fail on launch with a broken engine | MEDIUM | Spec'd in PROJECT.md; zero implementations currently exist |
| Non-blocking "engine unhealthy" banner on startup | Warning must be visible but must not prevent app use — users may be deliberately offline | LOW | Show banner/prompt, not a modal that requires dismissal before app loads |

### Differentiators (Competitive Advantage)

These go beyond the baseline and make the engine setup experience genuinely good for CopySpeak's audience (technical users comfortable with CLI tools and local AI models).

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Engine presets (Piper / kokoro / Edge TTS / etc.) | One-click config for known engines eliminates fiddly command + args entry | LOW | Preset system exists in config already; Engine page should expose it as a selector |
| Step-by-step install instructions per engine preset | Technical users still need hand-holding for Python env + model downloads | MEDIUM | Currently backlogged; belongs on Engine page, not a separate wizard. Show instructions inline when preset is selected or engine is unhealthy |
| Diagnostic detail panel on test failure | Show exact stderr, exit code, or HTTP error body so power users can debug without opening logs | MEDIUM | `CommandFailed { code, stderr }` already captured in Rust; frontend needs to expose it collapsible |
| Voice list fetch + dropdown for cloud backends | ElevenLabs returns a voice list from the API; showing it avoids copy-pasting opaque voice IDs | HIGH | Requires a new Tauri command to call ElevenLabs `/v1/voices`; adds API surface area |
| "Speak test phrase" button (audio preview, not just health check) | Synthesizes a short phrase end-to-end — confirms audio path works, not just the engine binary | MEDIUM | Different from health_check: runs synthesize() → plays audio → user hears it |
| Copy command snippet to clipboard for install | Shows the exact `pip install kokoro-tts` or model download command; copy button reduces friction | LOW | Pure frontend, no backend needed |
| Auto-detect python / python3 for CLI backends | Windows Python install naming is inconsistent; try both and show which works | MEDIUM | Useful for Piper preset specifically; can be part of health_check logic |

### Anti-Features (Deliberately NOT Building This Milestone)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full engine installation wizard (multi-step modal flow) | Users unfamiliar with CLI tools want hand-holding | Scope bloat for a milestone that is about page structure, not an installer; creates fragile dependency on external installer APIs that break | Inline install guidance (markdown snippet + copy button) on the Engine page is 80% of the value at 20% of the cost |
| Automatic engine download / install from within the app | Eliminates manual setup steps | Requires elevated permissions, external network calls during install, version pinning, and rollback logic — enormous scope; Tauri sandboxing makes it hard | Document the install command and let users run it |
| Voice preview carousel / audition UI for local engines | ElevenLabs and similar cloud products offer voice browsing | Local CLI engines do not have a voice API — you must know the voice name in advance; building a fake carousel creates a misleading affordance | Show voice name field + "speak test phrase" to verify the voice is working |
| Saving multiple engine configurations as named presets | Users want to switch between "Fast English" and "Natural voice" setups | Voice preset management is explicitly out of scope for this milestone per PROJECT.md | Defer to v0.3; preset selector can reference the single Engine config today |
| Real-time synthesis latency measurement / benchmark display | Power users curious about speed | Adds async complexity; result varies by text length; misleads users into micro-optimizing | Performance comparison table in docs (already exists in tts_backends.md) |
| Guided onboarding flow for first-time users (multi-page wizard) | Users new to TTS need step-by-step | Adds a separate code path for "first run" state that diverges from the Engine page steady state; doubles maintenance burden | Treat first run the same as "engine unhealthy": show non-blocking banner → route to Engine page → inline setup help there |

---

## Feature Dependencies

```
[Startup Health Check]
    └──requires──> [Health Check Tauri Command (health_check trait exists)]
    └──triggers──> [Non-blocking Banner / Prompt in Play view]
                       └──routes to──> [Engine Page]

[Engine Page - Backend Selector]
    └──enables──> [Per-backend credential fields]
    └──enables──> [Per-backend install guidance]
    └──enables──> [Test Engine Button]

[Test Engine Button]
    └──requires──> [Backend Selector + credentials saved]
    └──uses──> [health_check() Tauri command]
    └──enhances──> ["Speak Test Phrase" button]

["Speak Test Phrase" Button]
    └──requires──> [Test Engine Button passes (engine is alive)]
    └──requires──> [synthesize() + audio playback IPC path]

[Engine Preset Selector]
    └──enhances──> [Per-backend install guidance] (preset selection determines which guide to show)
    └──enhances──> [Voice field] (preset sets a sensible default voice)

[Voice List Fetch for Cloud Backends]
    └──requires──> [Backend Selector = ElevenLabs or OpenAI]
    └──requires──> [API key field to be saved]
    └──conflicts──> [Static voice name field] (replace text field with dropdown when list is available)

[3-Tab Navigation (Play / Engine / Settings)]
    └──required by──> [Engine Page] (page needs a route before it can exist)
    └──must not break──> [existing Play and Settings views]
```

### Dependency Notes

- **Startup health check requires Engine page to exist:** The banner must link somewhere meaningful. Build the Engine page first, then wire up the startup check.
- **"Speak test phrase" depends on health check passing:** Do not allow audio synthesis test if the engine binary or API key check has already failed — show the health check result first.
- **Voice list fetch conflicts with static voice name field:** For ElevenLabs and OpenAI backends, replace the text field with a fetched dropdown once the API key is present and valid. The text field remains for CLI and HTTP backends where no API exists.
- **Preset selector enhances install guidance:** Which preset is selected determines which install snippet to show. They are tightly coupled in the UI even if technically independent.

---

## MVP Definition (This Milestone)

### Launch With (this milestone's Engine page)

These features make the Engine page functional and trustworthy. Without them the page is worse than the current Settings section.

- [ ] 3-tab navigation (Play / Engine / Settings) — Engine page needs a route
- [ ] Backend selector (CLI / ElevenLabs / OpenAI / HTTP) — moved from Settings
- [ ] Per-backend credential and command fields — moved from Settings
- [ ] Engine preset selector for CLI backend — exposes existing preset system
- [ ] Live "Test Engine" button with specific error diagnosis — surfaces TtsError variants
- [ ] Non-blocking startup health check with banner prompt routing to Engine page — core milestone deliverable
- [ ] Inline install instructions per preset (markdown snippet + copy button) — replaces backlogged "Quick Install Guide" feature; must be inline not a wizard

### Add After Validation (next milestone or v0.3)

- [ ] "Speak test phrase" audio preview button — high value but depends on working health check; add once Test Engine UX is stable
- [ ] Voice list fetch + dropdown for ElevenLabs — high value; adds significant backend API surface, defer until Engine page base is solid
- [ ] Diagnostic detail panel (collapsible stderr / HTTP body) — power-user feature; add when basic error messages prove insufficient

### Future Consideration (v0.3+)

- [ ] Voice preset manager (named multi-engine configs) — explicitly out of scope per PROJECT.md
- [ ] Auto-detect python vs python3 on Windows — useful quality-of-life improvement but not blocking
- [ ] Automatic engine installation from within app — do not build this milestone

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| 3-tab navigation | HIGH | LOW | P1 |
| Backend selector + fields (moved from Settings) | HIGH | LOW | P1 |
| Engine preset selector | HIGH | LOW | P1 |
| Startup health check + non-blocking banner | HIGH | MEDIUM | P1 |
| Test Engine button with error diagnosis | HIGH | MEDIUM | P1 |
| Inline install instructions per preset | HIGH | LOW | P1 |
| "Speak test phrase" audio preview | MEDIUM | MEDIUM | P2 |
| Voice list fetch for cloud backends | MEDIUM | HIGH | P2 |
| Diagnostic detail panel (stderr/HTTP body) | LOW | LOW | P2 |
| Voice preset manager | HIGH | HIGH | P3 (defer) |
| Auto-detect python/python3 | LOW | MEDIUM | P3 (defer) |
| Full install wizard | LOW | HIGH | P3 (do not build) |

---

## Competitor / Analogue Feature Analysis

| Feature | Home Assistant TTS | AllTalk TTS UI | VS Code Language Servers | CopySpeak Engine Page (plan) |
|---------|-------------------|----------------|--------------------------|-------------------------------|
| Engine/provider selector | Dropdown of integrations | Tab per engine | Auto-detected, not user-chosen | Backend selector (CLI/ElevenLabs/OpenAI/HTTP) |
| Voice selection | Dropdown fetched from provider | Dropdown of installed models | N/A | Text field (CLI); fetched dropdown (ElevenLabs) |
| Test/preview | "Try" plays sample in browser | Generate tab with playback | N/A | "Test Engine" + "Speak test phrase" |
| Broken engine UX | Integration shows error badge | Silently fails with no model | Status bar warning + link to fix | Non-blocking startup banner → route to Engine page |
| Install help | Links to integration docs | TTS Engine Settings tab with download button | Notification with "Install extension" action | Inline markdown instructions + copy button per preset |
| Error specificity | Generic connection error | Generic failure | Specific: "missing binary", path shown | Specific: maps TtsError variant to user message |

---

## Sources

- Home Assistant TTS integration documentation: https://www.home-assistant.io/integrations/tts/
- Home Assistant Cloud TTS (voice preview pattern): https://www.nabucasa.com/config/tts/
- AllTalk TTS V2 QuickStart Guide (engine setup, model download UX): https://github.com/erew123/alltalk_tts/wiki/AllTalk-V2-QuickStart-Guide
- AllTalk TTS SillyTavern docs (configuration page structure): https://docs.sillytavern.app/extensions/alltalk/
- TTS Voice Wizard getting started: https://ttsvoicewizard.com/docs/getting-started/gettingStarted
- Setup wizard UX best practices (LogRocket): https://blog.logrocket.com/ux-design/creating-setup-wizard-when-you-shouldnt/
- Wizard design pattern (UX Planet): https://uxplanet.org/wizard-design-pattern-8c86e14f2a38
- Carbon Design System notification pattern (banner UX): https://carbondesignsystem.com/patterns/notification-pattern/
- Notification banner UX (Astro UX DS): https://www.astrouxds.com/components/notification-banner/
- VS Code language server extension guide (inline error handling): https://code.visualstudio.com/api/language-extensions/language-server-extension-guide
- CopySpeak docs_internal/tts_backends.md — engine backend detail, TtsError variants, preset configs
- CopySpeak docs_internal/implemented_features.md — existing implemented/backlogged features
- CopySpeak .planning/PROJECT.md — milestone scope, constraints, out-of-scope list

---
*Feature research for: TTS engine configuration page + startup health check UX (CopySpeak milestone)*
*Researched: 2026-03-04*
