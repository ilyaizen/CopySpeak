// Sanitize: multi-pass text cleaning pipeline for TTS.
// Removes markdown formatting, normalizes text for speech, and cleans up artifacts.

pub(crate) mod cleanup;
mod markdown;
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
        result = markdown::strip_markdown(&result, &config.markdown);
    }

    // Pass 2: TTS text normalization
    if config.tts_normalization.enabled {
        result = sanitize_tts(&result);
    }

    // Final pass: clean up spacing and punctuation artifacts from all prior passes
    cleanup::cleanup_artifacts(&result)
}

/// Sanitize text that legitimately carries `[...]` emote tags — the
/// post-process output path. Tag spans are masked with an inert word
/// placeholder for the pipeline and restored afterwards; everything else is
/// sanitized exactly like [`sanitize_text`]. The plain citation pass would
/// delete single-word tags (`[sighs]`) while multi-word ones survived by
/// accident, so the plain path stays untouched and only this variant is
/// tag-safe. Placeholders are plain alphanumeric words because every
/// symbolic/private-use codepoint is stripped as unpronounceable.
pub fn sanitize_text_tag_safe(text: &str, config: &SanitizationConfig) -> String {
    if !config.enabled {
        return text.to_string();
    }
    let tags = crate::emote::find_tag_spans(text);
    if tags.is_empty() {
        return sanitize_text(text, config);
    }
    let mut masked = String::with_capacity(text.len());
    let mut last = 0usize;
    for (index, tag) in tags.iter().enumerate() {
        masked.push_str(&text[last..tag.start_byte]);
        // Indexed so no two tags collapse into one restore slot; digits next
        // to letters match no sanitizer expansion rule.
        masked.push_str(&format!("zqemote{index}zq"));
        last = tag.end_byte;
    }
    masked.push_str(&text[last..]);

    let sanitized = sanitize_text(&masked, config);
    let mut restored = sanitized;
    for (index, tag) in tags.iter().enumerate().rev() {
        let sentinel = format!("zqemote{index}zq");
        if !restored.contains(&sentinel) {
            log::warn!(
                "[PostProcess] emote tag {} was lost during re-sanitize",
                tag.text
            );
            continue;
        }
        restored = restored.replace(&sentinel, &tag.text);
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SanitizationConfig;

    fn default_config() -> SanitizationConfig {
        SanitizationConfig::default()
    }

    /// The post-process output path: bracket emote tags survive the
    /// re-sanitize even though the plain citation pass would eat them.
    #[test]
    fn tag_safe_resanitize_keeps_emote_tags() {
        let out = sanitize_text_tag_safe(
            "[sighs] Well [clears throat] fine [appalled]!",
            &default_config(),
        );
        assert_eq!(out, "[sighs] Well [clears throat] fine [appalled]!");
    }

    /// The pre-LLM path is unchanged: citations and markdown links are
    /// stripped exactly as before.
    #[test]
    fn plain_sanitize_still_strips_citations_and_links() {
        let out = sanitize_text(
            "See [1] now [doc](https://example.com) and [sighs]",
            &default_config(),
        );
        assert!(!out.contains('['), "{out:?}");
    }

    /// Tag safety only exempts the brackets; everything else in the tagged
    /// text still runs through the full pipeline.
    #[test]
    fn tag_safe_resanitize_still_cleans_surrounding_text() {
        let out = sanitize_text_tag_safe("[happy]  See   **bold** text", &default_config());
        assert!(!out.contains("**"), "{out:?}");
        assert!(!out.contains("  "), "{out:?}");
        assert!(out.contains("See bold text"), "{out:?}");
    }
}
