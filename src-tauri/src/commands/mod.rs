// Tauri IPC commands.
// These are the functions callable from the Svelte frontend via @tauri-apps/api.
// Each command is thin — it delegates to the appropriate module.

mod config;
mod history;
mod logging;
mod playback;
mod queue;
mod tts;
mod update;
mod install;

// Re-export all public commands and types so `main.rs` can use `commands::*` unchanged.
pub use config::*;
pub use history::*;
pub use install::*;
pub use logging::*;
pub use playback::*;
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

/// Event emitted during synthesis to show progress with ETA.
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct SynthesisProgressEvent {
    /// Estimated total duration in milliseconds (if available)
    pub estimated_total_ms: Option<u64>,
    /// Time elapsed since synthesis started in milliseconds
    pub elapsed_ms: u64,
    /// Current fragment index (0-based) for paginated texts
    pub fragment_index: usize,
    /// Total number of fragments (1 for non-paginated)
    pub fragment_total: usize,
    /// Whether this is a paginated synthesis
    pub is_paginated: bool,
    /// Confidence levelfor the estimate (0-1)
    pub confidence: f32,
    /// Text being synthesized (truncated)
    pub text_preview: String,
}
