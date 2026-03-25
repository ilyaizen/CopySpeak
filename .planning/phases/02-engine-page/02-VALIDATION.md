---
phase: 02
slug: engine-page
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-05
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Vitest |
| **Config file** | `vitest.config.ts` |
| **Quick run command** | `bun run test` |
| **Full suite command** | `bun run test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `bun run test`
- **After every plan wave:** Run `bun run test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | ENG-01 | unit | `bun run test --filter=engine-tabs` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | ENG-02 | unit | `bun run test --filter=local-engine` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 1 | ENG-02 | unit | `bun run test --filter=http-engine` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | ENG-02 | unit | `bun run test --filter=openai-engine` | ❌ W0 | ⬜ pending |
| 02-02-03 | 02 | 1 | ENG-02 | unit | `bun run test --filter=elevenlabs-engine` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 2 | SET-01 | grep | `grep -r "tts\." src/routes/settings/` | ✅ | ⬜ pending |
| 02-03-02 | 03 | 2 | STA-01 | unit | `bun run test --filter=draft-persistence` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/lib/components/engine/engine-tabs.test.ts` — Tab switching, draft persistence (ENG-01, STA-01)
- [ ] `src/lib/components/engine/local-engine.test.ts` — CLI config (ENG-02)
- [ ] `src/lib/components/engine/http-engine.test.ts` — HTTP config (ENG-02)
- [ ] `src/lib/components/engine/openai-engine.test.ts` — OpenAI config (ENG-02)
- [ ] `src/lib/components/engine/elevenlabs-engine.test.ts` — ElevenLabs config (ENG-02)
- [ ] `src/routes/settings/+page.svelte` — SET-01 verification via grep

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual tab styling matches brutalist pattern | ENG-01 | Subjective visual | Navigate to Engine page, verify tabs match existing UI patterns |
| Toast notifications display correctly | ENG-02 | Timing/visual | Edit a field, save, verify toast appears with correct message |
| Save bar positioning | STA-01 | Layout/visual | Edit config, verify save bar appears bottom-right |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
