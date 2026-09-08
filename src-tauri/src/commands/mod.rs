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
    pub captions: Option<crate::tts::captions::CaptionAlignment>,
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
    pub captions: Option<crate::tts::captions::CaptionAlignment>,
}

/// Event carrying one PCM chunk of streaming synthesis audio.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioStreamChunkEvent {
    /// Base64-encoded raw PCM data (empty on the terminal end-of-stream event)
    pub audio_base64: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    /// Zero-based index of the fragment this chunk belongs to
    pub fragment_index: usize,
    /// True only on the terminal zero-byte end-of-stream event
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_duration_ms: Option<u64>,
    pub captions: Option<crate::tts::captions::CaptionAlignment>,
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
