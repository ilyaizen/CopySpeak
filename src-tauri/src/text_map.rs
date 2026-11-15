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
        let first = self
            .spoken
            .partition_point(|&(_, word_end)| word_end <= start);
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
pub(crate) fn tokenize(text: &str) -> (Vec<Span>, Vec<String>) {
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
    let mut end = spans
        .get(i)
        .map_or(source.len(), |&(word_start, _)| word_start);
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
#[path = "text_map_tests.rs"]
mod tests;
