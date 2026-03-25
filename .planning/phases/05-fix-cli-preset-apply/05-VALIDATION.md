---
phase: 5
slug: fix-cli-preset-apply
status: compliant
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-06
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest 4.x + @testing-library/svelte 5.x |
| **Config file** | `vitest.config.ts` |
| **Quick run command** | `rtk vitest run src/lib/components/engine/local-engine.test.ts` |
| **Full suite command** | `rtk vitest run src/lib/components/engine/` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `rtk vitest run src/lib/components/engine/local-engine.test.ts`
- **After every plan wave:** Run `rtk vitest run src/lib/components/engine/`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** ~5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 1 | ENG-02 | unit | `rtk vitest run src/lib/components/engine/local-engine.test.ts` | ✅ | ✅ green |
| 5-01-02 | 01 | 1 | ENG-02 | unit | `rtk vitest run src/lib/components/engine/local-engine.test.ts` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Audit 2026-03-06

| Metric | Count |
|--------|-------|
| Gaps found | 3 |
| Resolved | 3 |
| Escalated | 0 |

### Gaps Resolved

| Gap | Root Cause | Fix Applied |
|-----|------------|-------------|
| All Svelte component tests failing (SSR error) | `vitest.config.ts` missing `svelteTesting()` plugin; `ssr.noExternal` config was in working tree but test env still loaded server Svelte | Added `svelteTesting()` from `@testing-library/svelte/vite` to plugins; restored `ssr: { noExternal: true }` |
| `InvalidCharacterError: "}}" did not match the Name production` on all local-engine tests | Stray `}}` at line 178 of `local-engine.svelte` after the `onchange` handler close | Removed the extra `}}` |
| TypeScript errors in `src/mocks/state.ts` | Unused `writable` import + `global` not typed in browser tsconfig | Removed `writable` import; replaced `global` with `globalThis` |

### Post-Fix Results

- **ENG-02 tests (3):** ✅ all pass
  - `ENG-02: selecting preset updates command` ✅
  - `ENG-02: selecting preset updates args_template` ✅
  - `ENG-02: switching to custom retains existing command/args` ✅
- **Engine suite total:** 70 pass / 6 fail
- **6 remaining failures:** Pre-existing Phase 3 test bugs (ambiguous query selectors, espeak text matching) — unrelated to Phase 5

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-06
