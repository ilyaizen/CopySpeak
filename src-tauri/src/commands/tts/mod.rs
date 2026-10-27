// TTS command sub-modules.
//
// This module splits the original monolithic `tts.rs` into focused files:
//
//   helpers.rs     — SynthesisGuard, create_backend, voice_for_backend, engine_str
//   health.rs      — test_tts_engine
//   synthesis.rs   — speak_now, speak_queued, speak_history_entry
//   credentials.rs — check_elevenlabs_credentials, check_openai_credentials
//   voices.rs      — list_elevenlabs_voices, get_elevenlabs_voice_by_id, get_elevenlabs_output_formats

mod credentials;
mod health;
pub(crate) mod helpers;
mod profiles;
mod synthesis;
mod voices;

// Glob re-exports so Tauri's `generate_handler!` can find the hidden
// `__cmd__*` items that the `#[tauri::command]` macro emits alongside each
// public function.
pub use credentials::*;
pub use health::*;
pub(crate) use helpers::{prewarm_active_profile, spawn_background_prewarm};
pub use profiles::*;
pub use synthesis::*;
pub use voices::*;
