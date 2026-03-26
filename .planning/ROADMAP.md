# Roadmap: CopySpeak

**Milestone:** v0.2 Settings Consolidation
**Started:** 2026-03-26
**Core Value:** Double-copy → instant speech must be flawless

## Phases

- [ ] **Phase 10: Tab Structure Reorganization** - Consolidate 8 settings tabs into 3 with correct content organization
- [ ] **Phase 11: Layout Consolidation** - Implement 2-column layout and minify redundant controls

---

## Phase Details

### Phase 10: Tab Structure Reorganization

**Goal:** Users navigate settings through 3 consolidated tabs with correct content organization and scroll-aware navigation highlighting.

**Depends on:** Nothing (first phase of v0.2 milestone)

**Requirements:** SETT-01, SETT-02, SETT-03, SETT-08, SETT-09

**Success Criteria** (what must be TRUE):

1. User sees exactly 3 navigation items in settings sidebar: General, Advanced, About
2. User finds Startup, Appearance, Playback, HUD, History, Triggers, Hotkeys sections correctly organized within General tab
3. User finds Pagination and Sanitization sections within Advanced tab
4. User sees App info and Import/Export sections within About tab
5. Active navigation item highlights correctly when scrolling within consolidated tabs

**Plans:** TBD

**UI hint:** yes

---

### Phase 11: Layout Consolidation

**Goal:** All settings display in a compact 2-column layout with minified dropdown controls for Pagination and HUD.

**Depends on:** Phase 10

**Requirements:** SETT-05, SETT-06, SETT-07

**Success Criteria** (what must be TRUE):

1. All setting rows show label with info tip on left column, control on right column in 2-column grid layout
2. User selects Pagination from a single dropdown containing Disabled and character limits (200, 400, 600... 2000)
3. User selects HUD position from a single dropdown with Disabled option followed by position options
4. Layout remains consistent across all tabs after consolidation

**Plans:** TBD

**UI hint:** yes

---

## Progress

| Phase                    | Plans Complete | Status      | Completed |
| ------------------------ | -------------- | ----------- | --------- |
| 10. Tab Structure        | 0/2            | Not started | -         |
| 11. Layout Consolidation | 0/2            | Not started | -         |

---

## Coverage

| Category      | Requirements                                | Phase    |
| ------------- | ------------------------------------------- | -------- |
| Tab Structure | SETT-01, SETT-02, SETT-03, SETT-08, SETT-09 | Phase 10 |
| Layout        | SETT-05, SETT-06, SETT-07                   | Phase 11 |

**Total:** 8 requirements mapped to 2 phases ✓

---

_Roadmap created: 2026-03-26_
