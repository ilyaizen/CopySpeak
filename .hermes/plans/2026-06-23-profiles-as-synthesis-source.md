# CopySpeak: Profiles as the Synthesis Source of Truth (Phase 2)

> Supersedes the unfinished parts of `2026-06-22_164103-profile-integrations-engine-settings.md`.
> Phase 1 (schema v2, catalog, backend resolution, control server, CLI) is committed (`b72219b`).

## Context

Phase 1 made the **backend** resolve synthesis from the active profile (`resolve_effective` → `active_profile_id` → `create_backend_from_effective`). But the **app still behaves engine-first**:

- The footer "engine dropdown" (`app-footer.svelte` `switchEngine`) and the engine settings page write `active_backend` + per-engine global voice/preset directly. They never touch `active_profile_id`. So the user's visible control still bypasses profiles.
- Four backend readers still treat `active_backend` as the authority instead of the active profile: `test_tts_engine`, HUD `get_provider_voice`, `TtsConfig::validate`, and the control-server engine override.
- The default profile is named `"Default"`, not the desired `Engine - Voice` (e.g. `Cartesia - Katie`).

**Goal:** profiles fully own synthesis orchestration; the engine settings page becomes the home for API keys, engine-specific settings/model defaults (fallbacks), a proper engine test, and docs/guides (installers later). The footer becomes a profile switcher.

**Decisions (confirmed):** full `active_backend` decommission; engine page = keys+models+test+docs (no voice picking there); footer = profile switcher.

---

## Workstreams

### A. Default profile named `Engine - Voice`

Files: `src-tauri/src/config/tts.rs`

- Add helper `profile_display_name(engine: &TtsEngine, voice_label: Option<&str>, voice: &str) -> String` →
  `"{engine catalog label} - {voice_label or prettified voice}"` e.g. `Cartesia - Katie`.
  Reuse engine label from `tts/catalog.rs` (`EngineCatalogEntry.label`); fall back to voice id if no label.
- `VoiceProfile::default()` (tts.rs:614) → name from helper instead of `"Default"`.
- `migrate_tts_config()` (tts.rs:698): default profile created from legacy config (line 711) gets the helper name; resolve `voice_label` from catalog when known (Cartesia Katie etc.).
- Keep `id: "default"` stable (undeletable, referenced everywhere). Only the **name** changes.

### B. Backend `active_backend` decommission

Make the active profile the authority; `active_backend` becomes a derived mirror only.

- `helpers.rs`: add `active_engine(tts: &TtsConfig) -> TtsEngine` = `resolve_effective(tts).engine`. Use it everywhere a reader currently reads `tts.active_backend`.
- `commands/tts/health.rs` `test_tts_engine`: build backend from the **active profile** (`resolve_effective` + `create_backend_from_effective`) instead of `create_backend(active_backend)`. This also fixes the footer's local-engine availability check, which calls `test_tts_engine`.
- `commands/.../hud.rs` `get_provider_voice`: derive provider + voice from `resolve_effective`, not `active_backend`.
- `config/tts.rs` `validate()` (line 745): validate the **active profile's** engine + that profile's options, not the loose global `active_backend`/local fields.
- `control_server.rs`: drop the `active_backend = parse_engine(...)` mutation (line ~226). `persist_selection` only sets `active_profile_id`. Keep `engine`/`effect` as request-local debug shorthands (already non-mutating otherwise).
- Keep the `active_backend` field in `TtsConfig` for serialization compatibility, but write it = active profile's engine whenever the active profile changes (single helper `sync_active_backend_mirror`), so old readers/configs stay coherent during transition.

### C. New IPC: lightweight profile switch

Files: `src-tauri/src/commands/tts/` (profiles module or helpers), `src-tauri/src/main.rs`

- Add `set_active_profile(id: String) -> Result<(), String>`: validate id exists, set `active_profile_id`, sync the `active_backend` mirror, save, emit `config-changed`. Avoids the footer round-tripping the whole `AppConfig`.
- Register in `main.rs` `generate_handler!`.
- (Profiles list already available to the footer via existing config; no new read command needed.)

### D. Footer → profile switcher

Files: `src/lib/components/layout/app-footer.svelte`, `src/lib/types.ts` (no new types expected)

- Replace the engine list (`ENGINES`, `switchEngine`, `currentEngineId`, `getEngineLabel`, `getVoiceLabelForEngine`) with a **profile list** read from `config.tts.profiles`.
- Display: active profile's `name` (already `Engine - Voice`); optional secondary line = voice label.
- Selecting a profile → `invoke("set_active_profile", { id })` (workstream C), then refresh.
- Availability dot: keep, but check the **active profile's** engine via the existing `test_tts_engine`/credential-check commands (now profile-aware from B). Avoid per-profile probing on open (window-flash concern) — only check the active one, mirror current behavior.
- Drop `DEFAULT_VOICES`/preset-sniffing helpers that existed only to label engines.

### E. Engine settings page repurpose

Files: `src/lib/components/engine/engine-page.svelte`, child engine components under `src/lib/components/engine/*`, `src/routes/engine/+page.svelte`

- Remove "active engine" semantics: no `active_backend` writes, no "active" badge, no tab-switch-sets-active. Tabs become pure navigation between engine config panels.
- Each engine panel keeps/owns: **API key** + credential check, **model** + **output format** + other account/global knobs (these are the documented *fallback defaults* when a profile omits them), a **Test this engine** button (uses the engine's own global config, independent of the active profile — a true engine smoke test), and a **docs/guides** link from `tts/catalog.rs` `docs_url`.
- Remove per-engine **voice** selection from this page (voice belongs to profiles). If a quick "browse voices" affordance is wanted it can link to profiles; not required here.
- Reserve a placeholder section/anchor for future **installers** (local engines) — copy only, no logic this pass.

### F. Docs + changelog

Files: `docs/profile-engine-settings.md`, `CHANGELOG.md`

- Update the doc's UX section to reflect: footer = profile switcher; engine page = keys/models/test/docs; default profile naming.
- CHANGELOG `[Unreleased]`: Changed (profiles drive synthesis end-to-end; engine page repurposed; default profile naming), Added (`set_active_profile` IPC), Fixed (`test_tts_engine`/HUD/validate now profile-aware).

---

## Critical files

- `src-tauri/src/config/tts.rs` — A (naming), B (validate, mirror)
- `src-tauri/src/commands/tts/helpers.rs` — B (`active_engine`), C
- `src-tauri/src/commands/tts/health.rs` — B (test_tts_engine)
- HUD command file (`get_provider_voice`) — B
- `src-tauri/src/control_server.rs` — B
- `src-tauri/src/main.rs` — C (register)
- `src/lib/components/layout/app-footer.svelte` — D
- `src/lib/components/engine/engine-page.svelte` (+ children) — E

## Reuse (don't reinvent)

- `tts/catalog.rs` — engine labels + `docs_url` for naming (A) and docs links (E).
- `helpers.rs::resolve_effective` / `create_backend_from_effective` — single resolution path for B.
- Existing credential-check IPC (`check_openai_credentials`, etc.) and `test_tts_engine` for footer/engine-page status.

## Risks

- Engine page is the riskiest UI change (multiple child components). Do B (backend) + A first so the app stays coherent, then D, then E.
- Removing voice from engine page must not orphan per-engine voice fields in config — keep the fields (used as fallbacks + migration), just stop editing them there.

---

## Verification (ask before running — repo rule)

Rust: `cd src-tauri && cargo test config::tests && cargo test commands::tts && cargo test tts::catalog`
Frontend: `bun run test app-footer && bun run test profile-manager`
Type/svelte: `bun run check`

Manual (running app):
1. Footer shows active profile as `Engine - Voice`; switching profiles changes synthesis voice.
2. Engine page: set OpenAI key + model, hit "Test this engine" — speaks regardless of active profile; no "active engine" badge; docs link opens.
3. Switch active profile in footer → engine page does **not** change active state (decoupled).
4. HUD shows the active profile's provider/voice.
5. `POST /speak {profile}` does not change active profile; `persist_selection:true` does, and updates the footer.
6. Fresh/legacy config migrates: default profile appears as e.g. `Cartesia - Katie`.
