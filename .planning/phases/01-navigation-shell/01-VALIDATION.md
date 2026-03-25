---
phase: 1
slug: navigation-shell
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-04
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest |
| **Config file** | `vitest.config.ts` |
| **Quick run command** | `bun run test src/lib/components/layout/app-header.test.ts` |
| **Full suite command** | `bun run test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `bun run test src/lib/components/layout/app-header.test.ts`
- **After every plan wave:** Run `bun run test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 0 | NAV-01, NAV-02, NAV-03 | unit | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 1 | NAV-01 | unit | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ W0 | ⬜ pending |
| 1-01-03 | 01 | 1 | NAV-02 | unit | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ W0 | ⬜ pending |
| 1-01-04 | 01 | 1 | NAV-03 | unit | `bun run test src/lib/components/layout/app-header.test.ts` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/lib/components/layout/app-header.test.ts` — stubs for NAV-01, NAV-02, NAV-03

*Wave 0 creates the test file before Wave 1 implements the nav change.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Engine tab visible and clickable in running app | NAV-01 | E2E Tauri/WebView2 render | Run `bun run tauri dev`, click Engine tab, confirm route loads |
| URL persists on tray minimize/restore | NAV-03 | Requires Tauri tray integration | Minimize to tray, restore, confirm URL still `/engine` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
