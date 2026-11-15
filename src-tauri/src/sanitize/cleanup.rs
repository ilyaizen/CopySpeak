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
#[path = "cleanup_tests.rs"]
mod tests;
