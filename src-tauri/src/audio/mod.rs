// Audio playback via rodio.
// Supports interrupt and queue modes.
// Also extracts amplitude envelopes from WAV data for the HUD waveform display.
//
// NOTE: rodio's OutputStream is not Send+Sync, so we use a dedicated audio thread
// with channels to communicate with it. This allows AudioPlayerHandle to be Send+Sync.

mod format;
mod player;
pub(crate) mod stream;
mod wav;

// Re-export all public types so external code can use `crate::audio::*` unchanged.
pub use format::convert_audio_format;
pub use player::{AudioPlayer, PlaybackState};
pub use wav::{concat_wav_files, extract_envelope};

/// Pre-computed amplitude envelope for HUD waveform visualization.
/// Contains N normalized RMS values (0.0–1.0) evenly spaced across the audio duration.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AmplitudeEnvelope {
    /// Normalized RMS values, one per bar in the HUD.
    pub values: Vec<f32>,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}
