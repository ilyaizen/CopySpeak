// Pass 1: Markdown syntax stripping.

use regex::Regex;

/// Strip markdown syntax from text, respecting per-feature config toggles.
pub(super) fn strip_markdown(
    text: &str,
    config: &crate::config::MarkdownSanitizationConfig,
) -> String {
    let mut result = text.to_string();
    if config.strip_code_blocks {
        result = strip_code_blocks(&result);
    }
    if config.strip_inline_code {
        result = strip_inline_code(&result);
    }
    if config.strip_links {
        result = strip_links(&result);
    }
    if config.strip_headers {
        result = strip_headers(&result);
    }
    if config.strip_bold_italic {
        result = strip_bold_italic(&result);
    }
    if config.strip_lists {
        result = strip_lists(&result);
    }
    if config.strip_blockquotes {
        result = strip_blockquotes(&result);
    }
    result
}

fn strip_code_blocks(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref CODE_BLOCK_REGEX: Regex = Regex::new(r"```[\s\S]*?```").unwrap();
    }
    CODE_BLOCK_REGEX.replace_all(text, "").to_string()
}

fn strip_inline_code(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref INLINE_CODE_REGEX: Regex = Regex::new(r"`[^`]+`").unwrap();
    }
    INLINE_CODE_REGEX.replace_all(text, "").to_string()
}

fn strip_links(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref LINK_REGEX: Regex = Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    }
    LINK_REGEX.replace_all(text, "$1").to_string()
}

fn strip_headers(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref HEADER_REGEX: Regex = Regex::new(r"(?m)^(#{1,6})\s+(.*)$").unwrap();
    }
    HEADER_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let content = caps.get(2).map_or("", |m| m.as_str());
            if content.ends_with(['.', '?', '!', ':', ';']) {
                content.to_string()
            } else {
                format!("{}.", content)
            }
        })
        .to_string()
}

fn strip_bold_italic(text: &str) -> String {
    let result = text.replace("**", "").replace("__", "");
    result.replace('*', "").replace('_', "")
}

fn strip_lists(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref LIST_REGEX: Regex =
            Regex::new(r"(?m)^[ \t]*[-*+]\s+(.+)$|^[ \t]*\d+\.\s+(.+)$").unwrap();
    }
    LIST_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            let content = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map_or("", |m| m.as_str())
                .trim();

            if content.is_empty() {
                return String::new();
            }

            if content.ends_with(['.', '?', '!', ':', ';']) {
                content.to_string()
            } else {
                format!("{}.", content)
            }
        })
        .to_string()
}

fn strip_blockquotes(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref BLOCKQUOTE_REGEX: Regex = Regex::new(r"(?m)^[ \t]*>\s+").unwrap();
    }
    BLOCKQUOTE_REGEX.replace_all(text, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_headers() {
        assert_eq!(strip_headers("# Header"), "Header.");
        assert_eq!(strip_headers("## Header"), "Header.");
        assert_eq!(strip_headers("### Header"), "Header.");
        assert_eq!(strip_headers("Normal text"), "Normal text");
        // Should not double-up existing sentence-ending punctuation
        assert_eq!(strip_headers("# Already done!"), "Already done!");
        assert_eq!(strip_headers("# Section:"), "Section:");
        assert_eq!(strip_headers("# Question?"), "Question?");
    }

    #[test]
    fn test_strip_code_blocks() {
        assert_eq!(strip_code_blocks("Text\n```code```\nMore"), "Text\n\nMore");
        assert_eq!(strip_code_blocks("```print()```"), "");
    }

    #[test]
    fn test_strip_inline_code() {
        assert_eq!(strip_inline_code("Use `sudo` command"), "Use  command");
        assert_eq!(strip_inline_code("No code here"), "No code here");
    }

    #[test]
    fn test_strip_links() {
        assert_eq!(
            strip_links("Visit [Google](https://google.com)"),
            "Visit Google"
        );
        assert_eq!(strip_links("[Link](url)"), "Link");
    }

    #[test]
    fn test_strip_bold_italic() {
        assert_eq!(strip_bold_italic("**bold**"), "bold");
        assert_eq!(strip_bold_italic("*italic*"), "italic");
        assert_eq!(strip_bold_italic("__bold__"), "bold");
        assert_eq!(strip_bold_italic("_italic_"), "italic");
    }

    #[test]
    fn test_strip_lists_legend_example() {
        let config = crate::config::MarkdownSanitizationConfig::default();
        let input = "## Legend\n\n- **Added**: New features\n- **Changed**: Changes in existing functionality.\n- **Deprecated**: Soon-to-be removed features.\n- **Removed**: Removed features";
        let result = strip_markdown(input, &config);
        assert_eq!(result, "Legend.\n\nAdded: New features.\nChanged: Changes in existing functionality.\nDeprecated: Soon-to-be removed features.\nRemoved: Removed features.");
    }

    #[test]
    fn test_strip_blockquotes() {
        assert_eq!(strip_blockquotes("> Quote"), "Quote");
        assert_eq!(strip_blockquotes("> Quote text"), "Quote text");
    }

    #[test]
    fn test_strip_markdown_all() {
        let config = crate::config::MarkdownSanitizationConfig::default();
        let input = "# Title\n**bold** and *italic*\n- item\n> quote\n`code`";
        let result = strip_markdown(input, &config);
        assert!(!result.contains('#'));
        assert!(!result.contains("**"));
        assert!(!result.contains('>'));
    }

    #[test]
    fn test_strip_markdown_respects_inline_code_toggle() {
        // With inline code stripping disabled (default), backtick content preserved
        let mut config = crate::config::MarkdownSanitizationConfig::default();
        config.strip_inline_code = false;
        let input = "Use `sudo` command";
        let result = strip_markdown(input, &config);
        assert!(
            result.contains('`'),
            "Inline code should be preserved when strip_inline_code=false"
        );

        // With inline code stripping enabled
        config.strip_inline_code = true;
        let result = strip_markdown(input, &config);
        assert!(
            !result.contains('`'),
            "Inline code should be removed when strip_inline_code=true"
        );
    }

    #[test]
    fn test_strip_markdown_respects_code_block_toggle() {
        let mut config = crate::config::MarkdownSanitizationConfig::default();
        config.strip_code_blocks = true;
        let input = "Text\n```code```\nMore";
        let result = strip_markdown(input, &config);
        assert!(
            !result.contains("```"),
            "Code blocks should be removed when strip_code_blocks=true"
        );

        config.strip_code_blocks = false;
        let result = strip_markdown(input, &config);
        assert!(
            result.contains("```"),
            "Code blocks should be preserved when strip_code_blocks=false"
        );
    }
}
