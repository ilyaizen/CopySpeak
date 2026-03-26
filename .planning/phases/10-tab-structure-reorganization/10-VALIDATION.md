---
phase: 10
slug: tab-structure-reorganization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property               | Value                     |
| ---------------------- | ------------------------- |
| **Framework**          | vitest                    |
| **Config file**        | vitest.config.ts (exists) |
| **Quick run command**  | `bun run test`            |
| **Full suite command** | `bun run test`            |
| **Estimated runtime**  | ~15 seconds               |

---

## Sampling Rate

- **After every task commit:** Run `bun run test`
- **After every plan wave:** Run `bun run test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID  | Plan | Wave | Requirement | Test Type   | Automated Command                 | File Exists | Status     |
| -------- | ---- | ---- | ----------- | ----------- | --------------------------------- | ----------- | ---------- |
| 10-01-01 | 01   | 1    | SETT-08     | visual      | Manual: sidebar shows 3 tabs      | N/A         | ⬜ pending |
| 10-01-02 | 01   | 1    | SETT-01     | visual      | Manual: General sections correct  | N/A         | ⬜ pending |
| 10-01-03 | 01   | 1    | SETT-02     | visual      | Manual: Advanced sections correct | N/A         | ⬜ pending |
| 10-01-04 | 01   | 1    | SETT-03     | visual      | Manual: About sections correct    | N/A         | ⬜ pending |
| 10-02-01 | 02   | 2    | SETT-09     | integration | Manual: scroll highlight works    | N/A         | ⬜ pending |

_Status: ⬜ pending· ✅ green · ❌ red · ⚠️ flaky_

---

## Wave 0 Requirements

_Existing infrastructure covers all phase requirements._

No new test infrastructure needed — this is a pure UI reorganization with manual verification via visual inspection.

---

## Manual-Only Verifications

| Behavior               | Requirement | Why Manual       | Test Instructions                                                               |
| ---------------------- | ----------- | ---------------- | ------------------------------------------------------------------------------- |
| Sidebar shows 3 tabs   | SETT-08     | Visual layout    | Run `bun run tauri dev`, navigate to Settings, count sidebar items              |
| General tab sections   | SETT-01     | Visual structure | Verify sections: Startup, Appearance, Playback, HUD, History, Triggers, Hotkeys |
| Advanced tab sections  | SETT-02     | Visual structure | Verify sections: Pagination, Sanitization                                       |
| About tab sections     | SETT-03     | Visual structure | Verify sections: App Info, Import/Export                                        |
| Scroll-aware highlight | SETT-09     | Interaction      | Scroll within a tab, verify sidebar highlights correct section                  |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
