//! Order-preserving word alignment between the raw selection and the spoken
//! (sanitized) text.
//!
//! The browser companion needs caption word offsets in the RAW selected text,
//! while synthesis speaks sanitized text. Sanitization is a chain of regex
//! rewrites that carries no provenance, so this module used to *replay* a
//! hand-picked subset of them (whitespace collapse, same-length folds,
//! em-dash/ellipsis/parenthesis rewrites) and reject everything else. That made
//! word highlighting all-or-nothing for a whole reading, and a single `%`
//! expanding to " percent" was enough to switch it off (2026-09-10).
//!
//! Instead of a second, partial implementation of the sanitizer, align the two
//! texts' word tokens directly: identical words map exactly, a rewritten run
//! maps to the source run it replaced, and a word the sanitizer invented out of
//! punctuation maps to that punctuation. Only the individual word that cannot
//! be explained loses its highlight — never the reading.

use crate::sanitize::tts_normalize::canonical_same_length;

/// A UTF-16 `[start, end)` span — the browser's index space.
type Span = (usize, usize);

/// Most words the sanitizer can invent in place of one source run
/// (`%` → "percent", `km/h` → "kilometers per hour"). Deletions are unbounded
/// in the other direction: stripping a code block or a URL removes many source
/// words at once.
const MAX_INSERTED_WORDS: usize = 8;

/// Spoken-word → source-word span map. Always constructible.
pub struct TextAlignment {
    /// Spoken word spans, ascending and non-overlapping.
    spoken: Vec<Span>,
    /// Source span each spoken word came from, `None` when nothing in the
    /// source explains it.
    source: Vec<Option<Span>>,
}

/// Align `spoken` (the sanitized text) back onto `source` (the raw selection).
pub fn align(source: &str, spoken: &str) -> TextAlignment {
    let source_u16: Vec<u16> = source.encode_utf16().collect();
    let (source_spans, source_keys) = tokenize(source);
    let (spoken_spans, spoken_keys) = tokenize(spoken);
    let mut mapped: Vec<Option<Span>> = vec![None; spoken_spans.len()];

    let (mut i, mut j) = (0usize, 0usize);
    while i < source_keys.len() && j < spoken_keys.len() {
        if source_keys[i] == spoken_keys[j] {
            mapped[j] = Some(source_spans[i]);
            i += 1;
            j += 1;
            continue;
        }
        let Some((di, dj)) = resync(&source_keys[i..], &spoken_keys[j..]) else {
            // Nothing downstream lines up again: keep the highlight on the
            // rest of the selection rather than drifting word by word.
            let rest = Some((source_spans[i].0, source_spans[source_spans.len() - 1].1));
            mapped[j..].fill(rest);
            break;
        };
        // The rewritten spoken words came from the source words they replaced,
        // or — for a pure insertion — from the punctuation between the
        // surrounding words, which is what the sanitizer expanded.
        let block = if di > 0 {
            Some((source_spans[i].0, source_spans[i + di - 1].1))
        } else {
            gap_before(&source_u16, &source_spans, i)
        };
        mapped[j..j + dj].fill(block);
        i += di;
        j += dj;
    }

    TextAlignment {
        spoken: spoken_spans,
        source: mapped,
    }
}

impl TextAlignment {
    /// Map a UTF-16 span of the spoken text back to the raw selection.
    ///
    /// Returns the union of the source spans of every spoken word the range
    /// touches. `None` when the range covers no word at all (punctuation-only
    /// caption entries) or only words the sanitizer invented with no source.
    pub fn source_span_utf16(&self, start: usize, end: usize) -> Option<Span> {
        if start >= end {
            return None;
        }
        // First spoken word that ends after `start`; spans are ascending.
        let first = self.spoken.partition_point(|&(_, word_end)| word_end <= start);
        let mut span: Option<Span> = None;
        for (index, &(word_start, _)) in self.spoken.iter().enumerate().skip(first) {
            if word_start >= end {
                break;
            }
            let Some((source_start, source_end)) = self.source[index] else {
                continue;
            };
            span = Some(match span {
                Some((known_start, known_end)) => {
                    (known_start.min(source_start), known_end.max(source_end))
                }
                None => (source_start, source_end),
            });
        }
        span
    }
}

/// Split text into maximal runs of alphanumeric characters, returning each
/// run's UTF-16 span and a comparison key folded through the sanitizer's own
/// same-length substitution table (curly quotes, dashes, homoglyphs, fullwidth
/// forms) so a folded word still matches its source.
///
/// Punctuation is deliberately not part of a token: it is exactly what the
/// sanitizer rewrites, and it is never the unit a caption highlights.
///
/// ponytail: alphanumeric runs, so a script written without word spacing (CJK)
/// tokenizes as one long word and highlights the whole run; add script-aware
/// segmentation if a voice for one of those ships.
fn tokenize(text: &str) -> (Vec<Span>, Vec<String>) {
    let mut spans = Vec::new();
    let mut keys = Vec::new();
    let mut offset = 0usize; // running UTF-16 offset
    let mut current: Option<(usize, String)> = None;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current
                .get_or_insert_with(|| (offset, String::new()))
                .1
                .push(ch);
        } else if let Some((start, word)) = current.take() {
            spans.push((start, offset));
            keys.push(canonical_same_length(&word));
        }
        offset += ch.len_utf16();
    }
    if let Some((start, word)) = current {
        spans.push((start, offset));
        keys.push(canonical_same_length(&word));
    }
    (spans, keys)
}

/// Smallest `(source_skip, spoken_skip)` that puts both streams back on a
/// shared word, in ascending total displacement. Source skips are unbounded
/// (a stripped code block deletes many words); spoken skips are capped at
/// [`MAX_INSERTED_WORDS`].
fn resync(source: &[String], spoken: &[String]) -> Option<(usize, usize)> {
    for total in 1..source.len() + MAX_INSERTED_WORDS {
        for spoken_skip in 0..=total.min(MAX_INSERTED_WORDS) {
            let source_skip = total - spoken_skip;
            if source_skip < source.len() && agrees(source, spoken, source_skip, spoken_skip) {
                return Some((source_skip, spoken_skip));
            }
        }
    }
    None
}

/// True when the two streams share a word at the given skips. The word that
/// follows must agree too when both streams still have one: a lone common word
/// ("the") is far too weak an anchor to resynchronize on.
fn agrees(source: &[String], spoken: &[String], source_skip: usize, spoken_skip: usize) -> bool {
    let Some(word) = source.get(source_skip) else {
        return false;
    };
    if spoken.get(spoken_skip) != Some(word) {
        return false;
    }
    match (source.get(source_skip + 1), spoken.get(spoken_skip + 1)) {
        (Some(next_source), Some(next_spoken)) => next_source == next_spoken,
        _ => true,
    }
}

/// The source text between word `i - 1` and word `i`, trimmed of whitespace —
/// the punctuation a purely inserted spoken word was expanded from (`%` →
/// "percent"). `None` when the words are only separated by whitespace.
fn gap_before(source: &[u16], spans: &[Span], i: usize) -> Option<Span> {
    let mut start = if i == 0 { 0 } else { spans[i - 1].1 };
    let mut end = spans.get(i).map_or(source.len(), |&(word_start, _)| word_start);
    while start < end && is_space(source[start]) {
        start += 1;
    }
    while end > start && is_space(source[end - 1]) {
        end -= 1;
    }
    (start < end).then_some((start, end))
}

/// Whitespace test for a single UTF-16 unit. Surrogate halves are never
/// whitespace, so treating them as non-space is exact.
fn is_space(unit: u16) -> bool {
    char::from_u32(unit as u32).is_some_and(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slice `source` by UTF-16 offsets (the browser's index space).
    fn utf16_slice(source: &str, (start, end): Span) -> String {
        let units: Vec<u16> = source.encode_utf16().collect();
        String::from_utf16_lossy(&units[start..end])
    }

    /// Map the spoken word `word` (its `occurrence`-th appearance) back to the
    /// raw text and return what it highlights.
    fn highlight(source: &str, spoken: &str, word: &str, occurrence: usize) -> Option<String> {
        let byte = spoken
            .match_indices(word)
            .nth(occurrence)
            .unwrap_or_else(|| panic!("{word:?} #{occurrence} not in {spoken:?}"))
            .0;
        let start = spoken[..byte].encode_utf16().count();
        let end = start + word.encode_utf16().count();
        align(source, spoken)
            .source_span_utf16(start, end)
            .map(|span| utf16_slice(source, span))
    }

    #[test]
    fn plain_whitespace_collapse_maps_every_word() {
        let source = "First para.\n\nSecond one here.";
        let spoken = "First para. Second one here.";
        assert_eq!(highlight(source, spoken, "Second", 0).as_deref(), Some("Second"));
        assert_eq!(highlight(source, spoken, "First", 0).as_deref(), Some("First"));
    }

    /// The live 2026-09-10 failure: one `%` in a 1,944-character selection
    /// expanded to " percent", and the whole reading lost word highlighting.
    #[test]
    fn percent_expansion_costs_only_the_inserted_word() {
        let source = "Around 10-20% of measles cases result in hospitalization.";
        let spoken = crate::sanitize::sanitize_text(
            source,
            &crate::config::SanitizationConfig::default(),
        );
        assert!(spoken.contains("percent"), "{spoken:?}");
        for word in ["Around", "10", "20", "of", "measles", "hospitalization"] {
            assert_eq!(
                highlight(source, &spoken, word, 0).as_deref(),
                Some(word),
                "{word:?} lost its origin"
            );
        }
        // The invented word points at the punctuation it was expanded from.
        assert_eq!(highlight(source, &spoken, "percent", 0).as_deref(), Some("%"));
    }

    #[test]
    fn deletions_do_not_drift_the_words_after_them() {
        let source = "See https://example.com/x now, really now.";
        let spoken = crate::sanitize::sanitize_text(
            source,
            &crate::config::SanitizationConfig::default(),
        );
        assert_eq!(highlight(source, &spoken, "See", 0).as_deref(), Some("See"));
        assert_eq!(highlight(source, &spoken, "really", 0).as_deref(), Some("really"));
    }

    #[test]
    fn same_length_folds_still_map_to_the_raw_form() {
        // Curly quotes and a Cyrillic homoglyph both fold 1 unit → 1 unit.
        assert_eq!(
            highlight("The \u{201C}platform\u{201D} shipped", "The \"platform\" shipped", "platform", 0)
                .as_deref(),
            Some("platform")
        );
        assert_eq!(
            highlight("The \u{0441}at sat", "The cat sat", "cat", 0).as_deref(),
            Some("\u{0441}at")
        );
    }

    #[test]
    fn repeated_words_map_to_their_own_occurrence() {
        let source = "buffalo buffalo\nbuffalo";
        let spoken = "buffalo buffalo buffalo";
        let alignment = align(source, spoken);
        assert_eq!(alignment.source_span_utf16(16, 23), Some((16, 23)));
        assert_eq!(alignment.source_span_utf16(0, 7), Some((0, 7)));
    }

    #[test]
    fn punctuation_only_and_empty_ranges_have_no_word() {
        let alignment = align("a, b", "a, b");
        assert_eq!(alignment.source_span_utf16(1, 2), None, "the comma");
        assert_eq!(alignment.source_span_utf16(2, 2), None, "empty range");
        assert_eq!(alignment.source_span_utf16(400, 410), None, "past the end");
    }

    #[test]
    fn surrogate_pairs_keep_the_following_words_aligned() {
        let source = "a \u{1F44D}\u{1F3FD} b\nc";
        let spoken = "a  b c";
        let alignment = align(source, spoken);
        // Source UTF-16: a=0, sp=1, thumb=2..4, tone=4..6, sp=6, b=7, sp=8, c=9.
        assert_eq!(alignment.source_span_utf16(3, 4), Some((7, 8)), "b, not the emoji");
        assert_eq!(alignment.source_span_utf16(5, 6), Some((9, 10)));
    }

    /// Every word of real prose keeps its exact origin through the punctuation
    /// rewrites (em dash, ellipsis, parentheses) the sanitizer applies.
    #[test]
    fn punctuation_rewrites_preserve_every_word_source_span() {
        let gwern = "When I came across this quote, I was struck by its relevance to one of Eliezer Yudkowsky\u{2019}s \u{2018}beisutsukai\u{2019} posts about finding the successor to quantum mechanics, \u{201C}The Failures of Eld Science\u{201D}.\nI meant to write an essay on how interesting it is that we intellectually know that many of our current theories must be wrong, and even have pretty good ideas as to which ones, but we still cannot psychologically tackle them with the same energy as if we had some anomaly or paradox to explain, or have the benefit of hindsight. The students in Eliezer\u{2019}s story know that quantum mechanics is wrong; someone with a well-verified observation contradicting quantum mechanics knows that it is wrong (replace \u{2018}quantum\u{2019} with \u{2018}classical\u{2019} as you wish). They will achieve better results than a battalion of conventional QMists.\nBut nothing quite gelled.";
        let words = regex::Regex::new(r"\p{L}+").unwrap();
        for source in [
            gwern,
            "Anthropic \u{2014} and previously OpenAI \u{2014} has resigned.",
            "buffalo (buffalo) buffalo \u{2014} buffalo.",
            "text (aside), more (another). After.",
            "text(aside)more\u{2014}After.",
            "\u{2014} Opening words (aside).",
            "(Opening words) after.",
            "Text \u{2014}",
            "\u{1D11E} Before (aside) after \u{2014} final.",
            "Before \u{FF08}aside\u{FF09} after.",
            "Before (outer (inner) end) after.",
            "Your disclaimer fell wide\u{2026} The factor I had in mind.",
            "\u{1D11E} Word\u{2026} word \u{2014} word\u{2026} final.",
            ", They didn\u{2019}t know the answer.",
            "Nice \u{1F44D} work, everyone.",
        ] {
            let spoken = crate::sanitize::sanitize_text(
                source,
                &crate::config::SanitizationConfig::default(),
            );
            let alignment = align(source, &spoken);
            let source_words: Vec<_> = words.find_iter(source).collect();
            let spoken_words: Vec<_> = words.find_iter(&spoken).collect();
            assert_eq!(source_words.len(), spoken_words.len(), "{source:?}");
            for (raw, said) in source_words.into_iter().zip(spoken_words) {
                let start = spoken[..said.start()].encode_utf16().count();
                let end = start + said.as_str().encode_utf16().count();
                assert_eq!(
                    alignment.source_span_utf16(start, end),
                    Some((
                        source[..raw.start()].encode_utf16().count(),
                        source[..raw.end()].encode_utf16().count(),
                    )),
                    "Wrong origin for {:?} in {source:?}",
                    said.as_str(),
                );
            }
        }
    }

    /// A wholesale rewrite has no anchors left; the highlight stays on the
    /// remaining selection instead of walking off onto unrelated words.
    #[test]
    fn unrecoverable_divergence_falls_back_to_the_remaining_passage() {
        let source = "alpha bravo charlie delta";
        let spoken = "alpha zulu yankee xray";
        let alignment = align(source, spoken);
        assert_eq!(alignment.source_span_utf16(0, 5), Some((0, 5)), "alpha still exact");
        let (start, end) = alignment.source_span_utf16(6, 10).unwrap();
        assert_eq!(utf16_slice(source, (start, end)), "bravo charlie delta");
    }
}
