# Phase 4: First-Run Onboarding - Context

**Gathered:** 2026-03-05
**Status:** Ready for planning

<domain>
## Phase Boundary

First-run onboarding page that guides new users to configure their TTS engine. Appears only on fresh installs (detected by missing config file), presents a full-screen experience without app navigation, and allows users to set up or skip. Once completed or skipped, the onboarding never appears again. Actual engine installation assistance is a future phase.

</domain>

<decisions>
## Implementation Decisions

### Page Layout
- Full-screen onboarding page (no app-header, no footer)
- Modal-like experience that must be completed or dismissed before reaching main app
- No access to Play/Engine/Settings tabs while onboarding is active

### First-Run Detection
- Check if config file exists
- If config missing → show onboarding
- If config exists → skip onboarding, go directly to main app

### Skip Behavior
- Optional onboarding - users can skip
- When skipped, still creates a minimal config file
- Once config exists (even minimal), onboarding never appears again
- No "remind me later" - it's either complete or skip permanently

### OpenCode's Discretion
- Page content and steps (welcome message, engine selection, health check, etc.)
- Number of steps or single-page design
- Completion criteria (what counts as "complete")
- Post-onboarding transition (redirect to Play tab or other)
- Skip button placement and messaging
- Visual design and branding
- Success/error states during onboarding flow

</decisions>

<specifics>
## Specific Ideas

- Should feel like a standard first-run wizard (common pattern in desktop apps)
- Simple and quick - not a lengthy tutorial
- Guides users toward engine setup without forcing it

</specifics>

<deferred>
## Deferred Ideas

- TTS engine installation assistance (pip install commands, dependency checks) — future phase
- Re-onboarding for major version upgrades — future consideration
- Onboarding for existing users after update — not needed (config already exists)

</deferred>

---

*Phase: 04-startup-onboarding*
*Context gathered: 2026-03-05*
