# 0001: Switch voice profile per reading by detected language

Status: Accepted, 2026-10-15

## Context

A user keeps one voice profile per language. Every speak path reads whatever text it was given, so German text read by an English voice sounds wrong. Switching the active profile by hand before each reading is friction, and changing it globally would alter the voice for every other reading.

Before any engine sees the text, the sanitizer also mangled non-English text: Cyrillic letters were folded to Latin lookalikes, the zero-width non-joiner was stripped, non-Latin sentence ends were not recognized, and English rewrites (`Dr.` → "Doctor", `m` → "million") ran on every language.

## Decision

- **Detect** the language of the utterance with `whatlang` (`src-tauri/src/language.rs`). A result counts only when the text has at least 10 characters and the detector reports it reliable. Anything else is "unknown".
- **Tag** a profile with an optional `language` (ISO 639-1, from the detector's 70 codes). `None` means the profile speaks any language. Profiles are tagged from their voice's catalog language: schema v7 fills untagged profiles, and the picker sets the tag on every voice change.
- **Match** per reading, with `resolve_effective_for_text`:
  1. Detection is off or unknown: the active profile, no hint.
  2. The active profile speaks any language, or its language is the detected one: the active profile, with the detected language as a hint.
  3. The first profile tagged with the detected language **and ready** is the target, with the hint.
  4. Otherwise: the active profile, with the hint.
- **Ready** means installed (local engines) or has credentials (cloud engines). A custom CLI (`Local`) profile is ready when its command can run; HTTP profiles are always ready. An unready profile is never a target, so an uninstalled bundled profile cannot take readings from a non-English active profile.
- **Hint**: the detected language travels with the request as `language_hint`. ElevenLabs sends it as `language_code` and Cartesia as `language`; other engines ignore it. Norwegian is `nb` in the detector and `no` at both providers.
- **Scope**: every speak path (double-copy, browser, hotkey, tray, Play page, control server and CLI), except a request that names a profile. A named profile is used as given and skips switching.
- **Per reading**: the switched profile is not saved, and `set_active_profile` is not called.
- **Playback follows the profile that spoke**: each reading emits `reading-profile` with its speed, pitch and effect, so the playback store applies them to that reading. Engine, voice, telemetry, history, the saved-audio cache and the HUD read the same profile. History replays use the row's saved speed and the current profile's pitch and effects, with the active engine and the row's voice id. A replay of a row spoken by another engine pairs that voice id with the active engine; this is known, not decided.
- **No retry**: if the switched profile fails, its error surfaces. Falling back silently would hide a broken profile.
- **Fragments**: a reading's fragments share one profile.

## Alternatives considered

- **Persist the switch as the active profile.** Rejected: one reading would change the voice for everything after it.
- **Match the first tagged profile, with the active profile winning only a tie (v1's rule).** Rejected: profiles are now auto-tagged from their voice. An untagged cloud active profile would lose English readings to an auto-tagged Kokoro `en` profile, and an unready profile could be picked.
- **Retry on the active profile after a failure.** Rejected: it hides the failure and speaks in a voice the user did not pick for that language.
- **Detect per fragment.** Rejected: short fragments are often unreliable, and a reading mixing voices sounds broken.
- **Switch only on double-copy and browser readings.** Rejected: the same text would sound different depending on how it was spoken.

## Consequences

- Short, mixed-language or very famous sentences keep the active profile. The detector refuses the classic English pangram by design.
- The v7 migration tags every untagged profile, including one the user meant as "any language". Such a profile is switched like any tagged one until its language is cleared.
- The English-only rewrites (abbreviations, units, symbols, slash forms) run only on English or unclear text. A non-English reading therefore keeps `$`, `€`, `%` and `&` unexpanded. Splitting out the symbol pass is a possible follow-up.
- The language list exists in two places: `iso_639_1` in `language.rs` and `DETECTED_LANGUAGE_CODES` in `profile-language.ts`. `profile-language.test.ts` fails when they drift, so change both together.
- A queued reading can retune the previous reading's tail, because the playback store applies `reading-profile` when the reading starts. Marked with a `ponytail:` comment in the playback store.
- Each local engine has one daemon slot. Two same-engine profiles in different languages reload the model on every switch, so the first reading after a flip is slow.
- History replay HUD labels the engine and voice of the active profile, as every other HUD does. The row's own engine and voice are not used for the label; whether they should be is not decided.
