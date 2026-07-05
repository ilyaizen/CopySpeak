# Engines & Profiles Unification

> **Date:** 2026-07-04
> **Status:** Implementing
> **Scope:** Make the Engine page honor the profile/credential boundary; replace the legacy embedded KittenTTS installer with a generic, script-driven CLI installer launcher; add Windows installers for every local engine; document engines in one public matrix.

## Problem

The Engine page violated its own boundary (`docs/profile-engine-settings.md`).
Per-engine components (`elevenlabs-engine.svelte`, `cartesia-engine.svelte`,
`openai-engine.svelte`, `edge-engine.svelte`, `local-engine.svelte`) edited
voice / model / stability / args directly into the global provider structs.
Those same fields are owned by voice profiles, and `helpers.rs` mirrors the
active profile back into the provider structs at synthesis time — so a value
set on the Engine page could be silently overwritten by the active profile.
That is a DRY/SOLID violation and a footgun.

Separately, engine install was half-built:

- `commands/install.rs` embedded the legacy root `install-kittentts.ps1`
  (which manages system Python) via `include_str!`, contradicting the
  `uv`-based installer plan.
- Only `scripts/install-chatterbox.ps1` existed as a real uv installer.
- The Engine page had a placeholder: _"Guided installers … future release."_

## Decisions (locked with user, 2026-07-04)

1. **Single thin engine component.** Replace the five per-engine components
   with one catalog-driven `engine-page.svelte` tab layout. Delete the old
   components and their tests.
2. **Engine page = credentials + test + installer only.** Voice, model,
   format, stability, args, etc. live exclusively in profiles + the engine
   catalog. The Engine page never edits them.
   - Cloud tabs: API key input (+ endpoint for Microsoft) + "Test setup"
     (reuses `test_tts_engine_config`, which builds a backend from the global
     provider defaults — i.e. the engine's default voice).
   - Local tabs: "Install" button (launches the installer) + docs. The
     installer's own smoke test is the verification path.
3. **Simple CLI installers, not a TUI.** Each installer is a PowerShell
   script with a colored banner, automatic uv-based install from the network,
   and a pause-at-end window. A button in the app spawns it detached via the
   new `install_engine(name)` IPC.
4. **Installer set:** `kitten`, `piper`, `kokoro`, `pocket`, `edge-tts`
   (new), plus existing `chatterbox` and the `uv` bootstrap. No new engines.
5. **`install_engine` IPC** resolves `scripts/install-<name>.ps1` and spawns
   it in a detached PowerShell window (pwsh → powershell fallback). Returns
   immediately; the script owns the window. No streaming in v1.
6. **One public doc:** `docs/engines.md` — a single matrix with a section per
   engine (setup, voices, cost, offline/cloud, installer command, API-key
   link). Promote to per-file only when one engine's section outgrows the page.
7. **Breaking change is fine** (pre-alpha). Users who set a voice on the
   Engine page will find it on Profiles after migration. Documented in
   CHANGELOG under **Breaking Changes**.

## Change set

### Backend (`src-tauri/`)

- **`commands/install.rs`** — rewritten:
  - Removed `get_installer_script_path`, `run_kittentts_installer`, and the
    `include_str!` of the legacy root installer + `kittentts-cli.py`.
  - Added `install_engine(engine: String) -> Result<(), String>`:
    - maps engine name → `scripts/install-<name>.ps1`
    - resolves the script path from dev (`CARGO_MANIFEST_DIR/../scripts`) or
      exe-relative candidates
    - spawns detached pwsh/powershell running the script with a
      pause-at-end wrapper
  - `ponytail:` script bundling for production builds is a later concern;
    dev path resolution is sufficient for pre-alpha.
- **`commands/config.rs`** — removed orphan IPC `get_data_dir` and
  `get_home_dir` (only caller was the deleted `local-engine.svelte` preview).
- **`main.rs`** — handler list: removed `get_installer_script_path`,
  `run_kittentts_installer`, `get_data_dir`, `get_home_dir`; added
  `install_engine`.

### Frontend (`src/`)

- **`engine/engine-page.svelte`** — rewritten around a single `ENGINE_TABS`
  registry. One shared layout renders credentials / installer / test per tab.
  Drops the cloud API-key Dialog (inlined per tab) and the catalog fetch
  (profile manager owns the catalog now).
- **Deleted** (dead after rewrite):
  - `engine/openai-engine.svelte`
  - `engine/elevenlabs-engine.svelte`
  - `engine/cartesia-engine.svelte`
  - `engine/edge-engine.svelte`
  - `engine/local-engine.svelte`
  - `engine/elevenlabs-engine.test.ts`
  - `engine/local-engine.test.ts`
  - `engine/eng02-minimal.test.ts` (self-referential, no value)
- **`engine/engine-page.test.ts`** — rewritten to the new tab set.

### Installers (`scripts/`)

- **`lib/copyspeak-engine-install.ps1`** — extended with `Write-EngineBanner`
  and `Confirm-Install` (shared CLI chrome).
- **New:** `install-kittentts.ps1`, `install-piper.ps1`, `install-kokoro.ps1`,
  `install-pocket.ps1`, `install-edge-tts.ps1`. uv-based, automatic, with
  `-Force` and `-SmokeTest` flags matching `install-chatterbox.ps1`.
- **New wrappers:** `kitten/copyspeak-kitten.py`, `piper/copyspeak-piper.py`
  (mirror `chatterbox/copyspeak-chatterbox.py`). kokoro/pocket/edge expose
  console scripts, so they use `uv tool install` and need no wrapper.
- **Deleted:** root `install-kittentts.ps1`, root `kittentts-cli.py`,
  `docs_internal/kittentts-cli.py` (superseded by the uv installer + wrapper).

### Docs

- **New:** `docs/engines.md` — public engine matrix.
- **Updated:** `docs/profile-engine-settings.md` note, `CHANGELOG.md`.

## Engine → installer → profile mapping

| Engine tab | Installer                | Profile engine | Profile preset | Verify                               |
| ---------- | ------------------------ | -------------- | -------------- | ------------------------------------ |
| Edge-TTS   | install-edge-tts.ps1     | edge           | —              | test_tts_engine_config("edge")       |
| Cartesia   | — (API key)              | cartesia       | —              | test_tts_engine_config("cartesia")   |
| ElevenLabs | — (API key)              | elevenlabs     | —              | test_tts_engine_config("elevenlabs") |
| OpenAI     | — (API key)              | openai         | —              | test_tts_engine_config("openai")     |
| Google     | — (API key)              | google         | —              | test_tts_engine_config("google")     |
| Microsoft  | — (API key + endpoint)   | microsoft      | —              | test_tts_engine_config("microsoft")  |
| Kitten TTS | install-kittentts.ps1    | local          | kitten-tts     | installer smoke test                 |
| Piper TTS  | install-piper.ps1        | local          | piper          | installer smoke test                 |
| Kokoro TTS | install-kokoro.ps1       | local          | kokoro-tts     | installer smoke test                 |
| Pocket TTS | install-pocket.ps1       | local          | pocket-tts     | installer smoke test                 |
| Chatterbox | install-chatterbox.ps1   | local          | chatterbox     | installer smoke test                 |
| HTTP       | — (configure in profile) | http           | —              | —                                    |

## Verification (manual; per repo rules, ask before running)

- `cargo check`, `bun run check`, `bun run test` after edits.
- Fresh config: Engine page shows all 12 tabs; cloud tabs accept a key and
  Test passes with a valid key; local tabs open the installer window.
- Profile manager still lists voices/options per engine; switching profiles
  drives synthesis end to end.
