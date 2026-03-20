// TTS backend abstraction.
// Each engine (CLI, HTTP, future sidecar) implements TtsBackend.
// The app doesn't care how speech is synthesized — only that it gets audio bytes back.

pub mod cli;
pub mod elevenlabs;
pub mod openai;

use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum TtsError {
    #[error("TTS command failed: {0}")]
    CommandFailed(String),

    #[error("TTS output file not found: {0}")]
    OutputNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TTS engine not available: {0}")]
    Unavailable(String),

    #[error("HTTP error: {0}")]
    Http(String),
}

/// Metadata for a voice option exposed in the settings UI.
#[derive(Debug, Clone, serde::Serialize)]
#[allow(dead_code)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub language: Option<String>,
    pub default: Option<bool>,
}

/// The core abstraction. Every TTS engine implements this.
/// Kept intentionally small — synthesize text, get audio bytes.
pub trait TtsBackend: Send + Sync {
    /// Human-readable name for settings UI display.
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Synthesize text into WAV audio bytes.
    /// Blocks until synthesis is complete (called from async context via spawn_blocking).
    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError>;

    /// Check if the engine binary/server is reachable.
    fn health_check(&self) -> Result<(), TtsError>;

    /// File extension for the audio bytes returned by `synthesize` (e.g. "wav", "mp3").
    /// Defaults to "wav" — only override when the backend returns a non-WAV format.
    fn file_extension(&self) -> &str {
        "wav"
    }

    /// Resolve voice ID to human-readable name for display and filenames.
    /// Returns lowercase, filesystem-safe name.
    #[allow(dead_code)]
    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}
