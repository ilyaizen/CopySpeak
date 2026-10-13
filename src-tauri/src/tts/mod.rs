// TTS backend abstraction.
// Each engine (CLI, HTTP, future sidecar) implements TtsBackend.
// The app doesn't care how speech is synthesized — only that it gets audio bytes back.

pub mod captions;
pub mod cartesia;
pub mod catalog;
pub mod cli;
pub mod edge;
pub mod elevenlabs;
mod elevenlabs_timing;
pub mod google;
pub mod http;
pub mod local_daemon;
pub mod microsoft;
pub mod openai;
pub mod stream;

use stream::ChunkStream;

use thiserror::Error;

/// Run an async block to completion from synchronous code: on the current
/// Tokio runtime when the caller is inside one, otherwise on a fresh runtime.
fn block_on_async<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(f),
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(f)
        }
    }
}

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
    ///
    /// Speed is intentionally NOT a parameter: playback speed is a frontend-only
    /// concern (applied via `audioEl.playbackRate`), mirroring how pitch is handled.
    /// Synthesis always runs at native speed; saved files do not bake in speed.
    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError>;

    fn synthesize_with_captions(
        &self,
        text: &str,
        voice: &str,
    ) -> Result<captions::SpeechAudio, TtsError> {
        self.synthesize(text, voice).map(Into::into)
    }

    /// Check if the engine binary/server is reachable.
    fn health_check(&self) -> Result<(), TtsError>;

    /// Load this engine's model ahead of the first utterance. Only local
    /// engines with a resident daemon do anything; everyone else is a no-op.
    #[allow(dead_code)]
    fn prewarm(&self, _voice: &str) {}

    /// Static claim: whether this backend (or model) can produce audio
    /// incrementally via [`TtsBackend::synthesize_streaming`]. Defaults to false
    /// for batch-only backends. The command layer picks the streaming path only
    /// when this and [`TtsBackend::runtime_ready`] both hold.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Whether streaming can start right now. Static claims stay in
    /// [`TtsBackend::supports_streaming`]; this covers warm-up state such as a
    /// local daemon that is still loading. Call sites require both.
    fn runtime_ready(&self) -> bool {
        true
    }

    /// Stream synthesized audio as PCM chunks.
    ///
    /// Only backends that override this and claim [`TtsBackend::supports_streaming`]
    /// are asked to stream. The default refuses instead of wrapping batch output,
    /// so a backend that is asked without a native stream fails loudly.
    #[allow(dead_code)]
    fn synthesize_streaming(&self, _text: &str, _voice: &str) -> Result<ChunkStream, TtsError> {
        Err(TtsError::Unavailable(format!(
            "{} has no native streaming",
            self.name()
        )))
    }

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

#[cfg(test)]
mod tests {
    use super::{block_on_async, TtsBackend, TtsError};

    struct BatchOnly;

    impl TtsBackend for BatchOnly {
        fn name(&self) -> &str {
            "batch-only"
        }

        fn synthesize(&self, _text: &str, _voice: &str) -> Result<Vec<u8>, TtsError> {
            Ok(Vec::new())
        }

        fn health_check(&self) -> Result<(), TtsError> {
            Ok(())
        }
    }

    #[test]
    fn backend_without_native_streaming_refuses_to_stream() {
        // Such a backend must fail loudly if asked, not silently wrap its batch output.
        let err = BatchOnly
            .synthesize_streaming("hi", "v")
            .err()
            .expect("batch-only backend must refuse to stream");
        assert!(
            matches!(&err, TtsError::Unavailable(reason) if reason.contains("no native streaming")),
            "{err}"
        );
    }

    #[test]
    fn block_on_async_runs_without_an_ambient_runtime() {
        assert_eq!(block_on_async(async { 7 }), 7);
    }

    #[test]
    fn block_on_async_runs_from_a_blocking_task_inside_a_runtime() {
        // Backends call this from spawn_blocking/std threads inside the app's
        // runtime; the helper must reuse that runtime instead of building one.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let value = rt.block_on(async {
            tokio::task::spawn_blocking(|| block_on_async(async { 9 }))
                .await
                .unwrap()
        });
        assert_eq!(value, 9);
    }
}
