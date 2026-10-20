//! `[sighs]`-style emote tags: bracket spans Dramatize's prompt adds and the
//! engine interprets instead of speaking.
//!
//! One scanner, three consumers: the tag-safe sanitizer (post-process output
//! re-sanitize), the caption paths (estimated and native), and
//! `words_preserved`. Spans carry both byte and UTF-16 offsets — string
//! slicing needs bytes, caption word offsets and the browser's string indices
//! are UTF-16.

/// One complete `[...]` tag in a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TagSpan {
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    /// The tag including its brackets (`"[sighs]"`).
    pub text: String,
}

/// Find every complete tag: up to 64 units of content, no brackets or
/// newlines inside, no nesting (a second `[` restarts the candidate — tags
/// never nest, and `[a[b]` is likelier mangled markup than a tag). Longer or
/// unterminated bracket content is left for the normal sanitizer passes.
pub(crate) fn find_tag_spans(text: &str) -> Vec<TagSpan> {
    let mut spans = Vec::new();
    let mut open: Option<(usize, usize)> = None; // (utf16, byte)
    let mut utf16 = 0usize;
    for (byte, ch) in text.char_indices() {
        match ch {
            '[' => open = Some((utf16, byte)),
            ']' => {
                if let Some((open_utf16, open_byte)) = open {
                    let content_units = utf16 - open_utf16 - 1;
                    if (1..=64).contains(&content_units) {
                        spans.push(TagSpan {
                            start_utf16: open_utf16,
                            end_utf16: utf16 + 1,
                            start_byte: open_byte,
                            end_byte: byte + ch.len_utf8(),
                            text: text[open_byte..byte + ch.len_utf8()].to_string(),
                        });
                    }
                    open = None;
                }
            }
            '\n' => open = None,
            _ => {}
        }
        utf16 += ch.len_utf16();
    }
    spans
}

/// Remove tag spans from `text`, collapsing the whitespace they leave behind
/// to single spaces. The result's offsets are NOT those of `text`; consumers
/// that need offset mapping use [`find_tag_spans`] directly.
pub(crate) fn strip_tags(text: &str) -> String {
    let spans = find_tag_spans(text);
    if spans.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for span in &spans {
        out.push_str(&text[last..span.start_byte]);
        last = span.end_byte;
    }
    out.push_str(&text[last..]);
    // Tags act as word separators; the removal often leaves doubled spaces or
    // a leading one, and caption text reads better without them.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<String> {
        find_tag_spans(text)
            .into_iter()
            .map(|span| span.text)
            .collect()
    }

    #[test]
    fn finds_single_and_multi_word_tags() {
        assert_eq!(
            texts("[sighs] well [clears throat] fine"),
            vec!["[sighs]", "[clears throat]"]
        );
    }

    #[test]
    fn unterminated_and_oversized_brackets_are_not_tags() {
        assert!(texts("a [b").is_empty());
        assert!(texts(&format!("[{}]", "x".repeat(65))).is_empty());
        assert!(texts("line one [tag\nline two]").is_empty());
    }

    #[test]
    fn citations_look_like_tags_here() {
        // By shape [1] IS a bracket tag; the plain sanitizer keeps stripping
        // it on the pre-LLM path, and the LLM prompt forbids it in output.
        assert_eq!(texts("See [1] now"), vec!["[1]"]);
    }

    #[test]
    fn strip_removes_tags_and_collapses_whitespace() {
        assert_eq!(
            strip_tags("[happy] Hello world [sighs] again"),
            "Hello world again"
        );
        assert_eq!(strip_tags("no tags"), "no tags");
        assert_eq!(strip_tags(""), "");
    }

    #[test]
    fn spans_carry_consistent_byte_and_utf16_offsets() {
        let text = "\u{1F44D} [sighs] hi"; // emoji: 2 UTF-16 units, 4 bytes
        let spans = find_tag_spans(text);
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(&text[span.start_byte..span.end_byte], "[sighs]");
        let units: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(
            String::from_utf16_lossy(&units[span.start_utf16..span.end_utf16]),
            "[sighs]"
        );
    }
}
