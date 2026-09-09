//! Exact source↔spoken text mapping through whitespace normalization plus
//! same-length punctuation/homoglyph substitution and known punctuation rewrites.
//!
//! Browser companion needs word offsets in the RAW selected text, while
//! synthesis speaks sanitized text. When sanitization only collapsed
//! whitespace and/or applied pure 1 UTF-16 unit → 1 unit character
//! substitutions (curly→straight quotes, en-dash→hyphen, Cyrillic→Latin
//! homoglyphs, unicode spaces), the mapping is exact: every spoken char came
//! from one known source char. Em-dashes, ellipses, and paired parentheses carry
//! their source positions through comma/space rewriting and artifact cleanup.
//! Anything else (URL rewriting, citation removal, LLM post-processing) degrades
//! to passage-only highlighting upstream. No fuzzy matching.

use crate::sanitize::{
    cleanup::cleanup_artifacts,
    tts_normalize::{canonical_same_length, PAREN_REGEX},
};

/// A spoken-text char → source-text char map (one source char per spoken char).
#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceMap {
    /// `spoken_to_source[i]` = source char index of spoken char `i`.
    spoken_to_source: Vec<usize>,
    /// UTF-16 offset of each char boundary in the collapsed (spoken) text;
    /// `offsets[i]` is the u16 offset of spoken char `i`, with a final entry
    /// for the end of the text.
    spoken_char_offsets: Vec<usize>,
}

/// Collapse every run of whitespace to a single space and trim the ends.
/// This mirrors the newline-stripping pass in the sanitizer closely enough
/// that equality with `sanitize_text` output proves no other pass fired.
pub fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(ch);
        }
    }
    out
}

/// Build an exact map through whitespace, same-length substitutions, and
/// em-dash/ellipsis/parenthesis rewrites. Unexplained differences remain unmappable.
pub fn whitespace_map(source: &str, collapsed: &str) -> Option<WhitespaceMap> {
    let collapsed_form = collapse_whitespace(source);
    let mut spoken_to_source: Vec<usize> = Vec::with_capacity(collapsed.chars().count());
    let mut spoken_char_offsets: Vec<usize> = Vec::with_capacity(collapsed.chars().count() + 1);
    let mut source_index = 0usize;
    let mut u16_offset = 0usize; // running UTF-16 offset in the collapsed text
    let mut pending_ws = false;
    for ch in source.chars() {
        if ch.is_whitespace() {
            pending_ws = true;
            source_index += 1;
        } else {
            if pending_ws && !spoken_to_source.is_empty() {
                // The inserted space points at the last source whitespace char.
                spoken_to_source.push(source_index - 1);
                spoken_char_offsets.push(u16_offset);
                u16_offset += 1; // the inserted space is one UTF-16 unit
            }
            pending_ws = false;
            spoken_to_source.push(source_index);
            spoken_char_offsets.push(u16_offset);
            u16_offset += ch.len_utf16();
            source_index += 1;
        }
    }
    spoken_char_offsets.push(u16_offset);
    let map = WhitespaceMap {
        spoken_to_source,
        spoken_char_offsets,
    };
    let canonical = canonical_same_length(&collapsed_form);
    if collapsed == collapsed_form || collapsed == canonical {
        return Some(map);
    }
    punctuation_map(&canonical, collapsed, map)
}

/// Replay only the approved rewrites with provenance, then compose the known
/// cleanup deletions/space insertions. Never search ahead for matching words.
fn punctuation_map(source: &str, spoken: &str, map: WhitespaceMap) -> Option<WhitespaceMap> {
    let mut delimiters = std::collections::BTreeMap::new();
    for matched in PAREN_REGEX.find_iter(source) {
        delimiters.insert(matched.start(), true);
        delimiters.insert(matched.end() - 1, false);
    }
    if delimiters.is_empty() && !source.contains(['—', '…']) {
        return None;
    }
    let mut rewritten = Vec::new();
    for ((byte, ch), origin) in source.char_indices().zip(map.spoken_to_source) {
        match delimiters.get(&byte) {
            Some(true) => rewritten.extend([(',', origin), (' ', origin)]),
            Some(false) => rewritten.push((',', origin)),
            None if ch == '—' => rewritten.extend([(',', origin), (' ', origin)]),
            None if ch == '…' => rewritten.extend([('.', origin); 3]),
            None => rewritten.push((ch, origin)),
        }
    }
    let text: String = rewritten.iter().map(|&(ch, _)| ch).collect();
    // Leading generated commas are stripped by sanitize_tts's orphan pass.
    // Do not extend that permission to unrelated leading source punctuation.
    let cleaned = cleanup_artifacts(&text);
    let expected = if source.starts_with(['—', '(']) {
        cleaned.trim_start_matches([',', ' '])
    } else {
        &cleaned
    };
    if expected != spoken {
        return None;
    }

    let mut input = rewritten.into_iter().peekable();
    let mut spoken_to_source = Vec::new();
    let mut spoken_char_offsets = vec![0];
    let mut offset = 0;
    let mut previous_origin = 0;
    for ch in spoken.chars() {
        loop {
            match input.peek().copied() {
                Some((next, origin)) if next == ch => {
                    input.next();
                    previous_origin = origin;
                    spoken_to_source.push(origin);
                    break;
                }
                Some((' ' | ',', _)) => {
                    input.next();
                }
                Some(_) if ch == ' ' => {
                    // Cleanup inserts a space after a comma/colon/semicolon.
                    spoken_to_source.push(previous_origin);
                    break;
                }
                _ => return None,
            }
        }
        offset += ch.len_utf16();
        spoken_char_offsets.push(offset);
    }
    // Cleanup can only discard spaces/commas from this rewritten candidate.
    if input.any(|(ch, _)| !matches!(ch, ' ' | ',')) {
        return None;
    }
    Some(WhitespaceMap {
        spoken_to_source,
        spoken_char_offsets,
    })
}

/// Diagnostic: where `spoken` stops being explainable as whitespace-collapse +
/// supported normalization of `source`. Mirrors `whitespace_map`'s acceptance
/// gate and includes bounded text excerpts for triage.
pub fn first_divergence(source: &str, spoken: &str) -> Option<String> {
    let collapsed_form = collapse_whitespace(source);
    if whitespace_map(source, spoken).is_some() {
        return None;
    }
    Some(format!(
        "spoken differs from supported source normalization; collapse(source)={:?} ({} chars), spoken={:?} ({} chars)",
        truncate_for_log(&collapsed_form),
        collapsed_form.chars().count(),
        truncate_for_log(spoken),
        spoken.chars().count(),
    ))
}

/// Bound log payloads when a divergence dump fires on huge selections.
fn truncate_for_log(text: &str) -> String {
    const MAX: usize = 160;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX).collect();
    out.push('…');
    out
}

impl WhitespaceMap {
    /// Map a UTF-16 span of the spoken text to the source text.
    ///
    /// `start` maps to its own source char; `end` is exclusive so it maps to
    /// one past the source char of spoken char `end - 1`. Returns `None` when
    /// a bound splits a surrogate pair — callers degrade to no word highlight.
    pub fn source_span_utf16(
        &self,
        source: &str,
        start: usize,
        end: usize,
    ) -> Option<(usize, usize)> {
        let spoken_char_start = self.utf16_to_char_index(start)?;
        let spoken_char_end = self.utf16_to_char_index(end)?;
        if spoken_char_start >= spoken_char_end || spoken_char_end > self.spoken_to_source.len() {
            return None;
        }
        let char_to_u16: Vec<usize> = std::iter::once(0)
            .chain(source.chars().scan(0usize, |acc, ch| {
                *acc += ch.len_utf16();
                Some(*acc)
            }))
            .collect();
        let source_char_start = self.spoken_to_source[spoken_char_start];
        let source_char_last = self.spoken_to_source[spoken_char_end - 1];
        let source_start = *char_to_u16.get(source_char_start)?;
        let source_end = *char_to_u16.get(source_char_last + 1)?;
        debug_assert!(source_start < source_end && source_end <= source.encode_utf16().count());
        Some((source_start, source_end))
    }

    /// Exact UTF-16 offset → spoken char index. `None` when the offset lands
    /// inside a surrogate pair (not a char boundary) or past the end.
    fn utf16_to_char_index(&self, offset: usize) -> Option<usize> {
        // `partition_point` on strictly-less returns the index of the first
        // entry >= offset, which equals the char index when `offset` is an
        // exact char start and overshoots it when the offset splits a pair.
        let idx = self.spoken_char_offsets.partition_point(|&o| o < offset);
        if idx >= self.spoken_char_offsets.len() || self.spoken_char_offsets[idx] != offset {
            return None;
        }
        Some(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_joins_paragraphs() {
        assert_eq!(collapse_whitespace("a\n\nb"), "a b");
        assert_eq!(collapse_whitespace("  x \t y  "), "x y");
        assert_eq!(collapse_whitespace("no-change"), "no-change");
        assert_eq!(collapse_whitespace("   "), "");
    }

    #[test]
    fn map_round_trips_multiline() {
        let source = "First para.\n\nSecond one here.";
        let collapsed = collapse_whitespace(source);
        let map = whitespace_map(source, &collapsed).unwrap();
        let spoken_start = collapsed.find("Second").unwrap();
        let (s, e) = map
            .source_span_utf16(source, spoken_start, spoken_start + 6)
            .unwrap();
        assert_eq!(&source[s..e], "Second");
    }

    #[test]
    fn map_rejects_non_whitespace_transforms() {
        let source = "See https://example.com/x now";
        // Sanitizer rewrote the URL: not a whitespace collapse.
        let rewritten = "See link now";
        assert!(whitespace_map(source, rewritten).is_none());
    }

    #[test]
    fn map_accepts_same_length_punctuation_substitution() {
        // Word/Docs-style curly quotes: sanitizer speaks the straight form.
        let source = "The “platform” didn’t ship";
        let spoken = "The \"platform\" didn't ship";
        let map = whitespace_map(source, spoken).unwrap();
        let (s, e) = map.source_span_utf16(source, 4, 14).unwrap();
        assert_eq!(utf16_slice(source, s, e), "“platform”");
    }

    #[test]
    fn map_accepts_homoglyph_fold() {
        // Cyrillic а (0x0430) in the raw text folds to Latin 'a' when spoken.
        let source = "The сat sat";
        let spoken = "The cat sat";
        let map = whitespace_map(source, spoken).unwrap();
        let (s, e) = map.source_span_utf16(source, 4, 7).unwrap();
        assert_eq!(utf16_slice(source, s, e), "сat");
    }

    #[test]
    fn map_rejects_deletions() {
        // Leading-comma strip and emoji removal are deletions, not
        // same-length substitutions — they still degrade to passage-only.
        let source = ", They didn’t know";
        let spoken = "They didn't know";
        assert!(whitespace_map(source, spoken).is_none());
        let source = "Nice 👍 work";
        let spoken = "Nice  work";
        assert!(whitespace_map(source, spoken).is_none());
    }

    #[test]
    fn map_handles_repeated_words_exactly() {
        let source = "buffalo buffalo\nbuffalo";
        let collapsed = collapse_whitespace(source);
        let map = whitespace_map(source, &collapsed).unwrap();
        // Third occurrence maps to the third occurrence, never the first.
        let (s, e) = map.source_span_utf16(source, 16, 23).unwrap();
        assert_eq!(&source[s..e], "buffalo");
        assert_eq!(s, 16);
    }

    #[test]
    fn punctuation_rewrites_preserve_every_word_source_span() {
        let gwern = "When I came across this quote, I was struck by its relevance to one of Eliezer Yudkowsky’s ‘beisutsukai’ posts about finding the successor to quantum mechanics, “The Failures of Eld Science”.\nI meant to write an essay on how interesting it is that we intellectually know that many of our current theories must be wrong, and even have pretty good ideas as to which ones, but we still cannot psychologically tackle them with the same energy as if we had some anomaly or paradox to explain, or have the benefit of hindsight. The students in Eliezer’s story know that quantum mechanics is wrong; someone with a well-verified observation contradicting quantum mechanics knows that it is wrong (replace ‘quantum’ with ‘classical’ as you wish). They will achieve better results than a battalion of conventional QMists.\nBut nothing quite gelled.";
        let words = regex::Regex::new(r"\p{L}+").unwrap();
        for source in [
            gwern,
            "Anthropic — and previously OpenAI — has resigned.",
            "buffalo (buffalo) buffalo — buffalo.",
            "text (aside), more (another). After.",
            "text(aside)more—After.",
            "— Opening words (aside).",
            "(Opening words) after.",
            "Text —",
            "𝄞 Before (aside) after — final.",
            "Before （aside） after.",
            "Before (outer (inner) end) after.",
            "Your disclaimer fell wide… The factor I had in mind.",
            "𝄞 Word… word — word… final.",
            "“The Manhattan Project”, Brennan said, “was launched with a specific technological end in sight: a weapon of great power, in time of war. But the error that Eld Science committed with respect to quantum physics had no immediate consequences for their technology. They were confused, but they had no desperate need for an answer. Otherwise the surrounding system would have removed all burdens from their effort to solve it. Surely the Manhattan Project must have done so—Taji? Do you know?”",
        ] {
            let spoken = crate::sanitize::sanitize_text(
                source,
                &crate::config::SanitizationConfig::default(),
            );
            let map = whitespace_map(source, &spoken)
                .unwrap_or_else(|| panic!("Rejected {source:?} -> {spoken:?}"));
            assert_eq!(first_divergence(source, &spoken), None);
            let source_words: Vec<_> = words.find_iter(source).collect();
            let spoken_words: Vec<_> = words.find_iter(&spoken).collect();
            assert_eq!(source_words.len(), spoken_words.len());
            for (raw, said) in source_words.into_iter().zip(spoken_words) {
                let start = spoken[..said.start()].encode_utf16().count();
                let end = start + said.as_str().encode_utf16().count();
                assert_eq!(
                    map.source_span_utf16(source, start, end),
                    Some((
                        source[..raw.start()].encode_utf16().count(),
                        source[..raw.end()].encode_utf16().count(),
                    )),
                    "Wrong origin for {:?} in {source:?}", said.as_str(),
                );
            }
            if source.starts_with('𝄞') {
                assert_eq!(map.source_span_utf16(source, 0, 2), Some((0, 2)));
                assert_eq!(map.source_span_utf16(source, 1, 2), None);
            }
            if let Some(byte) = source.find('…') {
                let origin = source[..byte].encode_utf16().count();
                let start = spoken[..spoken.find("...").unwrap()].encode_utf16().count();
                for offset in 0..3 {
                    assert_eq!(
                        map.source_span_utf16(source, start + offset, start + offset + 1),
                        Some((origin, origin + 1)),
                    );
                }
            }
        }
    }

    #[test]
    fn punctuation_mapping_still_rejects_unexplained_changes() {
        for (source, spoken) in [
            ("Same — same same.", "Same, same."),
            ("Same (same) same.", "Same, different, same."),
            ("See — https://example.com now.", "See, link now."),
            ("Nice — 👍 work.", "Nice, work."),
            ("Wait — … now.", "Wait, now."),
            ("Before (unclosed after.", "Before, unclosed after."),
        ] {
            assert!(whitespace_map(source, spoken).is_none(), "{source:?}");
            assert!(first_divergence(source, spoken).is_some());
        }
    }

    #[test]
    fn map_leading_whitespace() {
        let source = "\n\nLeading words";
        let collapsed = collapse_whitespace(source);
        let map = whitespace_map(source, &collapsed).unwrap();
        let (s, e) = map.source_span_utf16(source, 0, 7).unwrap();
        assert_eq!(&source[s..e], "Leading");
    }

    /// Slice `source` by UTF-16 offsets (the browser's index space).
    fn utf16_slice(source: &str, start: usize, end: usize) -> String {
        let units: Vec<u16> = source.encode_utf16().collect();
        String::from_utf16_lossy(&units[start..end])
    }

    #[test]
    fn map_survives_emoji() {
        let source = "a 👍🏽 b\nc";
        let collapsed = collapse_whitespace(source);
        assert_eq!(collapsed, "a 👍🏽 b c");
        let map = whitespace_map(source, &collapsed).unwrap();
        // UTF-16 offsets: a=0, sp=1, 👍=2-3, 🏽=4-5, sp=6, b=7, sp=8, c=9.
        let (s, e) = map.source_span_utf16(source, 7, 8).unwrap();
        assert_eq!(utf16_slice(source, s, e), "b");
        // A bound inside a surrogate pair is rejected, not mis-mapped.
        assert!(map.source_span_utf16(source, 3, 4).is_none());
        let (s, e) = map.source_span_utf16(source, 9, 10).unwrap();
        assert_eq!(utf16_slice(source, s, e), "c");
    }

    #[test]
    fn first_divergence_flags_real_divergence_and_accepts_clean_text() {
        // A deletion (leading-comma strip) diverges and gets flagged.
        let source = ", They didn’t know";
        let spoken = crate::sanitize::sanitize_text(
            source,
            &crate::config::SanitizationConfig::default(),
        );
        assert!(first_divergence(source, &spoken).is_some());

        // The live 2026-09-09 false degrade: plain-ASCII selection, one char
        // missing after sanitization. Whatever the culprit pass, this pins
        // the diagnostic contract: clean text reports None, mutated text
        // reports Some.
        let plain = "This is the strange state we're in: the system could do almost anything, but it requires the user to already know what to ask for. Too much of the work of discovering what's possible falls on the person, when it should fall on the system. You'd expect something this advanced to reveal its own capabilities, gradually, contextually, in ways that match your actual work. We're not there yet.";
        assert_eq!(first_divergence(plain, plain), None);
        assert!(first_divergence(plain, &format!("{plain}!")).is_some());
    }
}
