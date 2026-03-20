// Sanitize: multi-pass text cleaning pipeline for TTS.
// Removes markdown formatting, normalizes text for speech, and cleans up artifacts.

mod markdown;
pub(crate) mod cleanup;
pub(crate) mod tts_normalize;

use crate::config::SanitizationConfig;

// Re-export the public API
pub use tts_normalize::sanitize_tts;

/// Sanitize clipboard text by removing markdown formatting and normalizing for TTS.
/// Returns the sanitized text with all enabled sanitization passes applied.
///
/// # Pipeline Order
/// 1. **Markdown Stripping** (if enabled)
/// 2. **TTS Normalization** (if enabled)
/// 3. **Artifact Cleanup** (always runs)
pub fn sanitize_text(text: &str, config: &SanitizationConfig) -> String {
    if !config.enabled {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Pass 1: Strip markdown syntax
    if config.markdown.enabled {
        result = markdown::strip_markdown(&result);
    }

    // Pass 2: TTS text normalization
    if config.tts_normalization.enabled {
        result = sanitize_tts(&result);
    }

    // Final pass: clean up spacing and punctuation artifacts from all prior passes
    cleanup::cleanup_artifacts(&result)
}
