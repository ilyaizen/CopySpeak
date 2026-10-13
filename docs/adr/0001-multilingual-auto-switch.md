# 0001: Switch voice profile per reading by detected language

Status: Accepted, 2026-10-13

## Context

A user keeps one voice profile per language. Double-copy and browser readings speak whatever text was copied, so German text read by an English voice sounds wrong. Switching the active profile by hand before each reading is friction, and changing it globally would alter the voice for every other reading.

## Decision

- **Detect** the language of the utterance with `whatlang` (`src-tauri/src/language.rs`). A result counts only when the text has at least 10 characters and the detector reports it reliable. Anything else is "unknown".
- **Tag** a profile with an optional `language` (ISO 639-1, from the detector's 70 codes). Untagged profiles are never picked by language.
- **Choose** in `speak_queued_internal` only: the first profile tagged with the detected language. The active profile wins a tie.
- **Fall back** to the active profile when auto-switch is off, the text is short or unreliable, no profile matches, or the match is the active profile.
- **Scope**: double-copy and browser readings (`speak_queued`). Hotkey, Play page, tray and control-server speaks (`speak_now`) always use the active profile.
- **Per utterance**: the switched profile is not saved and `set_active_profile` is not called.
- **No retry**: if the switched profile fails, its error surfaces. Falling back silently would hide a broken profile.
- **Labels follow the profile**: for a live reading, engine, voice, telemetry, history, saved-audio cache and the HUD all read the profile that spoke. A reading's fragments share one profile.

## Alternatives considered

- **Persist the switch as the active profile.** Rejected: one reading would change the voice for everything after it.
- **Retry on the active profile after a failure.** Rejected: it hides the failure and speaks in a voice the user did not pick for that language.
- **Detect per fragment.** Rejected: short fragments are often unreliable, and a reading mixing voices sounds broken.

## Consequences

- Short, mixed-language or very famous sentences keep the active profile. The detector refuses the classic English pangram by design.
- The language list exists in two places: `iso_639_1` in `language.rs` and `DETECTED_LANGUAGE_CODES` in `profile-language.ts`. `profile-language.test.ts` fails when they drift, so change both together.
- Speak paths differ on purpose. A future change that adds switching to `speak_now` must also decide how its engine labels and cache keys work.
- History replay HUD names the engine and voice that spoke, not the active profile (`saved_provider_voice` in `hud.rs`). Rows store the voice id, not its label, so the label is that of the profile with the same engine and voice, as in the history list; without one, the voice id in title case. Renaming a profile therefore renames its past replays' HUD label.
