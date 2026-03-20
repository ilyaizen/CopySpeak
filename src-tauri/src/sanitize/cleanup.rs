// Pass 3: Final cleanup — spacing, punctuation artifacts, and trailing whitespace.

use regex::Regex;

/// Clean up spacing and punctuation artifacts introduced by prior normalization passes.
pub(crate) fn cleanup_artifacts(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref MULTI_SPACE: Regex = Regex::new(r" {2,}").unwrap();
        static ref SPACE_BEFORE_PUNCT: Regex = Regex::new(r" +([,.:;!?])").unwrap();
        static ref REPEATED_COMMA: Regex = Regex::new(r",(\s*,)+").unwrap();
        static ref COMMA_BEFORE_PERIOD: Regex = Regex::new(r",\s*\.").unwrap();
        static ref PUNCT_NO_SPACE: Regex = Regex::new(r"([,;:])([A-Za-z])").unwrap();
        static ref TRAILING_COMMA: Regex = Regex::new(r",\s*$").unwrap();
        static ref MULTI_NEWLINE: Regex = Regex::new(r"\n{3,}").unwrap();
    }

    let mut result = text.to_string();

    // Run core cleanup twice to catch cascading artifacts
    for _ in 0..2 {
        result = MULTI_SPACE.replace_all(&result, " ").to_string();
        result = SPACE_BEFORE_PUNCT.replace_all(&result, "$1").to_string();
        result = REPEATED_COMMA.replace_all(&result, ",").to_string();
        result = COMMA_BEFORE_PERIOD.replace_all(&result, ".").to_string();
        result = PUNCT_NO_SPACE.replace_all(&result, "$1 $2").to_string();
    }

    // Remove trailing comma
    result = TRAILING_COMMA.replace_all(&result, "").to_string();

    // Collapse excessive blank lines
    result = MULTI_NEWLINE.replace_all(&result, "\n\n").to_string();

    // Trim each line and the whole string
    result
        .lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SanitizationConfig;
    use crate::sanitize::{sanitize_text, tts_normalize::sanitize_tts};

    #[test]
    fn test_cleanup_double_spaces() {
        assert_eq!(cleanup_artifacts("word  more"), "word more");
        assert_eq!(cleanup_artifacts("word   more"), "word more");
    }

    #[test]
    fn test_cleanup_space_before_comma() {
        assert_eq!(cleanup_artifacts("word , more"), "word, more");
        assert_eq!(cleanup_artifacts("word  , more"), "word, more");
    }

    #[test]
    fn test_cleanup_double_commas() {
        assert_eq!(cleanup_artifacts("word,, more"), "word, more");
        assert_eq!(cleanup_artifacts("word, , more"), "word, more");
        assert_eq!(cleanup_artifacts("word,,, more"), "word, more");
    }

    #[test]
    fn test_cleanup_comma_before_period() {
        assert_eq!(cleanup_artifacts("word,."), "word.");
        assert_eq!(cleanup_artifacts("word, ."), "word.");
    }

    #[test]
    fn test_cleanup_missing_space_after_punct() {
        assert_eq!(cleanup_artifacts("word,next"), "word, next");
        assert_eq!(cleanup_artifacts("word;next"), "word; next");
    }

    #[test]
    fn test_cleanup_trailing_comma() {
        assert_eq!(cleanup_artifacts("word,"), "word");
        assert_eq!(cleanup_artifacts("word, "), "word");
    }

    #[test]
    fn test_cleanup_preserves_ellipsis() {
        assert_eq!(cleanup_artifacts("word..."), "word...");
        assert_eq!(cleanup_artifacts("wait... more"), "wait... more");
    }

    #[test]
    fn test_sanitize_tts_parentheses_no_artifacts() {
        let result = sanitize_tts("text (aside) more");
        assert_eq!(result, "text, aside, more");
    }

    #[test]
    fn test_sanitize_tts_parentheses_adjacent_comma() {
        let result = sanitize_tts("text (aside), more");
        assert_eq!(result, "text, aside, more");
    }

    #[test]
    fn test_sanitize_tts_no_double_commas_or_space_before_comma() {
        let input = "integrity (wholeness), grace (composure) and kindness under pressure (and a balanced proportion), seeing things";
        let result = sanitize_tts(input);
        assert!(!result.contains(",,"), "Double commas in: {}", result);
        assert!(!result.contains(" ,"), "Space before comma in: {}", result);
        assert!(!result.ends_with(','), "Trailing comma in: {}", result);
    }

    #[test]
    fn test_sanitize_tts_em_dash_spacing() {
        let result = sanitize_tts("word\u{2014}another");
        assert_eq!(result, "word, another");
    }

    // ── Full pipeline tests ─────────────────────────────────────────────

    #[test]
    fn test_full_sanitize_enabled() {
        let config = SanitizationConfig {
            enabled: true,
            ..Default::default()
        };
        let input = "# Title\n**bold** w/o issue & 10km etc.";
        let result = sanitize_text(input, &config);
        assert!(!result.contains('#'));
        assert!(!result.contains("**"));
        assert!(result.contains("without"));
        assert!(result.contains("and"));
        assert!(result.contains("kilometers"));
        assert!(result.contains("et cetera"));
    }

    #[test]
    fn test_full_sanitize_disabled() {
        let config = SanitizationConfig {
            enabled: false,
            ..Default::default()
        };
        let input = "# Title w/o **bold**";
        let result = sanitize_text(input, &config);
        assert_eq!(result, input);
    }

    #[test]
    fn test_full_sanitize_clean_output() {
        let config = SanitizationConfig {
            enabled: true,
            ..Default::default()
        };
        let input = "According to [1], the result (see appendix), was **significant** & ~100km/h.";
        let result = sanitize_text(input, &config);
        assert!(!result.contains(",,"), "Double commas in: {}", result);
        assert!(!result.contains(" ,"), "Space before comma in: {}", result);
        assert!(!result.contains("  "), "Double spaces in: {}", result);
    }

    // ── Integration tests for full preprocessing pipeline ────────────────

    #[test]
    fn test_full_preprocessing_pipeline_realistic_text() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: true,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: true },
            ..Default::default()
        };

        let input = r#"# Introduction

According to **recent studies** [1], the speed limit is ~100 km/h (e.g. on highways). Dr. Smith & Prof. Johnson reported that the company's revenue grew from $2bn to $5bn.

Visit https://example.com for more info & contact user@example.com.

- Item 1
- Item 2
- Item 3

> This is an important quote from the research.

`code snippet` should not be spoken.

Use `sudo apt-get install` for setup.

See [documentation](https://docs.example.com) for details.

## Key Points

The project is valued at **100M** with 5k users. Cost is €50 per unit & ~10% profit margin."#;

        let result = sanitize_text(input, &config);

        assert!(!result.contains('#'), "Should not contain markdown headers");
        assert!(!result.contains("**"), "Should not contain bold markers");
        assert!(!result.contains("[1]"), "Should not contain citations");
        assert!(
            !result.contains("[documentation]"),
            "Should not contain markdown links"
        );
        assert!(!result.contains("- "), "Should not contain list markers");
        assert!(!result.contains("> "), "Should not contain blockquote markers");
        assert!(!result.contains('`'), "Should not contain code markers");
        assert!(!result.contains("https://"), "Should not contain URLs");
        assert!(result.contains("for example"), "Should expand 'e.g.'");
        assert!(result.contains("approximately"), "Should expand '~'");
        assert!(result.contains("kilometers"), "Should expand 'km'");
        assert!(result.contains("and"), "Should expand '&'");
        assert!(result.contains("per"), "Should expand '/' in ratios");
        assert!(result.contains("Doctor"), "Should expand 'Dr.'");
        assert!(result.contains("Professor"), "Should expand 'Prof.'");
        assert!(result.contains("billion"), "Should expand 'bn'");
        assert!(result.contains("million"), "Should expand 'M'");
        assert!(result.contains("thousand"), "Should expand 'k'");
        assert!(result.contains("dollars"), "Should expand '$'");
        assert!(result.contains("euros"), "Should expand '€'");
        assert!(
            result.contains("at example.com"),
            "Should expand '@' in email"
        );
        assert!(!result.contains("  "), "Should not have double spaces");
        assert!(!result.contains(",,"), "Should not have double commas");
        assert!(result.len() > 50, "Output should have reasonable length");
    }

    #[test]
    fn test_full_preprocessing_pipeline_disabled() {
        let config = SanitizationConfig {
            enabled: false,
            ..Default::default()
        };

        let input = "# Title\n**Bold** text w/ issue & ~100 km/h.\nVisit https://example.com and email user@example.com.";

        let result = sanitize_text(input, &config);
        assert_eq!(
            result, input,
            "Text should pass through unchanged when sanitization is disabled"
        );
    }

    #[test]
    fn test_full_preprocessing_pipeline_markdown_only() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: true,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: false },
            ..Default::default()
        };

        let input = "# Title\n**Bold** text & ~100 km/h.";

        let result = sanitize_text(input, &config);

        assert!(!result.contains('#'));
        assert!(!result.contains("**"));
        assert!(
            result.contains('&'),
            "Should not expand '&' when TTS normalization is disabled"
        );
        assert!(
            result.contains('~'),
            "Should not expand '~' when TTS normalization is disabled"
        );
    }

    #[test]
    fn test_full_preprocessing_pipeline_tts_normalization_only() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: false,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: true },
            ..Default::default()
        };

        let input = "# Title\n**Bold** text & ~100 km/h.";

        let result = sanitize_text(input, &config);

        assert!(
            result.contains('#'),
            "Should keep markdown headers when markdown stripping is disabled"
        );
        assert!(
            result.contains("**"),
            "Should keep bold markers when markdown stripping is disabled"
        );
        assert!(
            result.contains("and"),
            "Should expand '&' when TTS normalization is enabled"
        );
        assert!(
            result.contains("approximately"),
            "Should expand '~' when TTS normalization is enabled"
        );
        assert!(
            result.contains("kilometers"),
            "Should expand 'km' when TTS normalization is enabled"
        );
    }

    #[test]
    fn test_full_preprocessing_pipeline_empty_after_filtering() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: true,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: true },
            ..Default::default()
        };

        let input = "``````";
        let result = sanitize_text(input, &config);
        assert!(
            result.trim().is_empty(),
            "Empty text should produce empty result"
        );
    }

    #[test]
    fn test_full_preprocessing_pipeline_preserves_multiline_structure() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: true,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: true },
            ..Default::default()
        };

        let input = "# First Section\nThis is the first paragraph.\n\n## Second Section\nThis is the second paragraph.";

        let result = sanitize_text(input, &config);

        // TTS normalization strips newlines into spaces for clean speech output
        assert!(!result.contains('\n'), "TTS normalization strips newlines");
        assert!(!result.contains('#'));
    }

    #[test]
    fn test_full_preprocessing_pipeline_currency_edge_cases() {
        let config = SanitizationConfig {
            enabled: true,
            markdown: crate::config::MarkdownSanitizationConfig {
                enabled: false,
                ..Default::default()
            },
            tts_normalization: crate::config::TtsNormalizationConfig { enabled: true },
            ..Default::default()
        };

        let input = "The total is $19.99 and €12.50 and £3.75.";

        let result = sanitize_text(input, &config);

        assert!(
            result.contains("19.99 dollars"),
            "Should handle $ with decimals"
        );
        assert!(
            result.contains("12.50 euros"),
            "Should handle € with decimals"
        );
        assert!(
            result.contains("3.75 pounds"),
            "Should handle £ with decimals"
        );
    }
}
