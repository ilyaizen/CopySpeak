// Tauri IPC commands.
// These are the functions callable from the Svelte frontend via @tauri-apps/api.
// Each command is thin — it delegates to the appropriate module.

mod config;
mod history;
mod install;
mod logging;
mod playback;
mod post_process;
mod queue;
mod tts;
mod update;

// Re-export all public commands and types so `main.rs` can use `commands::*` unchanged.
pub use config::*;
pub use history::*;
pub use install::*;
pub use logging::*;
pub use playback::*;
pub use post_process::*;
pub use queue::*;
pub use tts::*;
pub use update::*;

/// Cached audio for replay without re-synthesis.
#[derive(Default)]
pub struct CachedAudio {
    pub wav_bytes: Option<Vec<u8>>,
    pub text: Option<String>,
}

/// Event emitted during pagination playback.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginationEvent {
    pub total: usize,
    pub current_index: usize,
    pub is_paginated: bool,
}

/// Event emitted when a fragment's audio is ready for streaming playback.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioFragmentEvent {
    /// Base64-encoded WAV audio data
    pub audio_base64: String,
    /// Zero-based index of this fragment
    pub fragment_index: usize,
    /// Total number of fragments
    pub fragment_total: usize,
    /// Whether this is the final fragment
    pub is_final: bool,
    /// Text being spoken in this fragment
    pub text: String,
}
