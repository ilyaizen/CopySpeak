// Text pagination module for splitting long texts into fragments at sentence boundaries.
// Handles intelligent fragmentation to ensure TTS engines get complete, coherent segments.

use crate::config::PaginationConfig;

/// A text fragment created from splitting a larger text.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TextFragment {
    /// The fragment text content
    pub text: String,
    /// Zero-based index of this fragment in sequence
    pub index: usize,
    /// Total number of fragments
    pub total: usize,
    /// UTF-16 [start, end) of this fragment's text inside the text that was
    /// paginated. Downstream consumers (browser companion) use this to
    /// project fragment-local caption offsets back to full-text space.
    pub source_start: usize,
    pub source_end: usize,
}

impl TextFragment {
    /// Create a new text fragment spanning its whole text.
    pub fn new(text: String, index: usize, total: usize) -> Self {
        let source_end = text.encode_utf16().count();
        Self {
            text,
            index,
            total,
            source_start: 0,
            source_end,
        }
    }

    /// Create a fragment and record where its text came from in the source.
    /// Offsets are UTF-16 into the paginated text.
    pub fn with_source_span(
        text: String,
        index: usize,
        total: usize,
        source_start: usize,
        source_end: usize,
    ) -> Self {
        Self {
            text,
            index,
            total,
            source_start,
            source_end,
        }
    }

    /// Returns true if this is the first fragment.
    #[allow(dead_code)]
    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    /// Returns true if this is the last fragment.
    #[allow(dead_code)]
    pub fn is_last(&self) -> bool {
        self.index == self.total - 1
    }

    /// Returns a formatted label like "Part 1 of 3".
    #[allow(dead_code)]
    pub fn label(&self) -> String {
        format!("Part {} of {}", self.index + 1, self.total)
    }
}

/// Sentence boundary position in text.
#[derive(Debug, Clone, PartialEq)]
struct SentenceBoundary {
    /// Character position of the sentence end (inclusive)
    position: usize,
    /// The character that marks the sentence end
    delimiter: char,
}

/// Detect sentence boundary positions in the text.
/// Returns a list of positions where sentences end.
fn detect_sentence_boundaries(text: &str) -> Vec<SentenceBoundary> {
    let mut boundaries = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Check for sentence-ending punctuation
        if is_sentence_end(c) {
            // `!`/`?` glued to a following word (`Hello!World`) are token
            // punctuation, not sentence ends: the run up to the next whitespace
            // carries letters, so cutting there would sever the token.
            if matches!(c, '!' | '?')
                && chars.get(i + 1).is_some_and(|next| !next.is_whitespace())
                && chars[i + 1..]
                    .iter()
                    .take_while(|next| !next.is_whitespace())
                    .any(|next| next.is_alphanumeric())
            {
                i += 1;
                continue;
            }

            // A period that joins two word characters is a token separator, not
            // a sentence end: decimals (3.14), versions (v0.2.10), domains and
            // paths (example.com, file.rs), and dotted identifiers. Treating
            // them as boundaries is what let pagination cut mid-token.
            if is_intra_token_period(&chars, i) {
                i += 1;
                continue;
            }

            // Skip if this looks like an abbreviation (e.g., "Mr.", "Dr.", "etc.")
            if is_abbreviation(&chars, i) {
                i += 1;
                continue;
            }

            // This is a sentence boundary
            boundaries.push(SentenceBoundary {
                position: i,
                delimiter: c,
            });
        }

        i += 1;
    }

    // If no boundaries found, treat the entire text as one sentence
    if boundaries.is_empty() && !text.is_empty() {
        boundaries.push(SentenceBoundary {
            position: text.chars().count().saturating_sub(1),
            delimiter: if let Some(&last_char) = chars.last() {
                last_char
            } else {
                '.'
            },
        });
    }

    boundaries
}

/// Check if a character is a sentence-ending punctuation: Latin, CJK, Devanagari
/// danda, Arabic question mark, Urdu full stop, Armenian, Ethiopic, Myanmar, Khmer.
fn is_sentence_end(c: char) -> bool {
    matches!(
        c,
        '.' | '!' | '?' | '。' | '！' | '？' | '।' | '॥' | '؟' | '۔' | '։' | '።' | '။' | '។'
    )
}

/// True when the `.` at `pos` sits between two word characters, so it belongs to
/// a single token rather than ending a sentence. Covers decimals (`3.14`),
/// versions (`v0.2.10`), domains and file names (`example.com`, `file.rs`), IPs
/// (`192.168.1.1`), dotted initialisms (`U.S.A.`, `Ph.D.`) and dotted
/// identifiers (`System.Console`).
///
/// A genuine sentence period is followed by whitespace, a quote/bracket, or
/// end-of-text — not by another word character. Erring toward "one token" is
/// deliberate: the requirement is to never break a word, and treating a run-on
/// sentence (`blue.It`) as a single token only means it paginates on whitespace
/// instead of after the period, which is still word-safe.
fn is_intra_token_period(chars: &[char], pos: usize) -> bool {
    if chars.get(pos) != Some(&'.') {
        return false;
    }
    if chars
        .get(pos + 1)
        .is_some_and(|next| matches!(next, '.' | '/' | '\\'))
    {
        return true;
    }
    let prev_is_word = pos > 0
        && (is_token_char(chars[pos - 1]) || matches!(chars[pos - 1], '_' | '/' | '\\' | '.'));
    match chars.get(pos + 1) {
        // A word character after the period means one token (`example.com`).
        Some(&next) => {
            prev_is_word && (is_token_char(next) || matches!(next, '_' | '/' | '\\' | '.'))
        }
        // A trailing period is intra-token only when it closes an initialism
        // (`U.S.A.`, `Ph.D.`) — a single uppercase letter that is itself
        // preceded by a period. A trailing sentence end (`too.`) stays a
        // boundary, so run-on prose still splits where a sentence ends.
        None => prev_is_word && chars[pos - 1].is_uppercase() && pos >= 2 && chars[pos - 2] == '.',
    }
}

/// Characters that can appear inside an unbroken token. Digits and letters are
/// the common case; the surrounding tests only need to know that the period did
/// not separate two words.
fn is_token_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Check if the punctuation at position is likely part of an abbreviation.
fn is_abbreviation(chars: &[char], pos: usize) -> bool {
    // Need at least 1 character before the period
    if pos < 1 {
        return false;
    }

    // Check for special multi-period abbreviations (these take priority)
    // "e.g.", "i.e.", "etc.", "n.b.", "vs."
    if pos >= 3 {
        let four_char: String = chars[pos - 3..=pos]
            .iter()
            .collect::<String>()
            .to_lowercase();
        if four_char == "e.g." || four_char == "i.e." || four_char == "n.b." || four_char == "vs." {
            return true;
        }
    }

    // Check for "etc." (5 chars including the leading space/boundary)
    if pos >= 4 {
        let five_char: String = chars[pos - 4..=pos]
            .iter()
            .collect::<String>()
            .to_lowercase();
        if five_char == " etc." || five_char == ".etc." {
            return true;
        }
    }

    // Extract the word before the period by scanning backwards
    let mut word_start = pos - 1;
    while word_start > 0 && chars[word_start - 1].is_alphabetic() {
        word_start -= 1;
    }

    // The word must be preceded by whitespace or start of text (not part of a larger word)
    if word_start > 0 && !chars[word_start - 1].is_whitespace() && chars[word_start - 1] != '.' {
        return false;
    }

    let word: String = chars[word_start..pos]
        .iter()
        .collect::<String>()
        .to_lowercase();

    // Known title/honorific abbreviations (case-insensitive)
    matches!(
        word.as_str(),
        "mr" | "mrs"
            | "ms"
            | "dr"
            | "sr"
            | "jr"
            | "prof"
            | "rev"
            | "gen"
            | "gov"
            | "sgt"
            | "cpl"
            | "pvt"
            | "lt"
            | "capt"
            | "col"
            | "maj"
            | "cmdr"
            | "st"
            | "ave"
            | "blvd"
            | "dept"
            | "est"
            | "approx"
            | "inc"
            | "ltd"
            | "corp"
            | "no"
            | "vol"
            | "fig"
            | "ed"
            | "al"
    )
}

/// Split text into fragments at sentence boundaries, respecting the maximum fragment size.
/// Each fragment will be as close to the target size as possible without splitting sentences.
///
/// # Arguments
/// * `text` - The text to split into fragments
/// * `config` - Pagination configuration with fragment size setting
///
/// # Returns
/// A vector of text fragments. Returns a single fragment if pagination is disabled or text is short.
pub fn paginate_text(text: &str, config: &PaginationConfig) -> Vec<TextFragment> {
    // If pagination is disabled or text is empty, return as single fragment
    if !config.enabled || text.is_empty() {
        return vec![TextFragment::new(text.to_string(), 0, 1)];
    }

    let max_size = config.fragment_size as usize;
    let text_len = text.chars().count();

    // If text fits in one fragment, return as single fragment
    if text_len <= max_size {
        return vec![TextFragment::new(text.to_string(), 0, 1)];
    }

    // Detect sentence boundaries
    let boundaries = detect_sentence_boundaries(text);

    let chars: Vec<char> = text.chars().collect();

    // If no boundaries found, force split at max_size (fallback)
    if boundaries.is_empty() {
        log::warn!(
            "[Pagination] No sentence boundaries found, forcing split at {} chars",
            max_size
        );
        let pieces = force_split_ranges(text, max_size);
        return build_fragments(text, &chars, pieces);
    }

    // Build fragments by grouping sentences within max_size.
    //
    // We iterate over sentence boundaries and accumulate sentences into the
    // current fragment until adding the next sentence would exceed max_size,
    // then we cut. Pieces are kept as untrimmed char ranges so exact source
    // spans survive the trim.
    let mut pieces: Vec<(usize, usize)> = Vec::new(); // char ranges in `text`
    let mut fragment_start = 0; // start of the current accumulating fragment

    for (idx, boundary) in boundaries.iter().enumerate() {
        let sentence_end = boundary.position + 1; // position after the delimiter
        let current_fragment_len = sentence_end - fragment_start;

        // Check if including this sentence would exceed the limit
        if current_fragment_len > max_size {
            // If we already have accumulated sentences before this one, flush them
            if idx > 0 {
                let prev_end = boundaries[idx - 1].position + 1;
                if prev_end > fragment_start {
                    let fragment: String = chars[fragment_start..prev_end].iter().collect();
                    if !fragment.trim().is_empty() {
                        pieces.push((fragment_start, prev_end));
                    }
                    fragment_start = prev_end;
                }
            }

            // Now check if this single sentence is itself longer than max_size.
            // If so, force-split it, then continue from after this sentence.
            let lone_len = sentence_end - fragment_start;
            if lone_len > max_size {
                let lone_text: String = chars[fragment_start..sentence_end].iter().collect();
                for (start, end) in force_split_ranges(&lone_text, max_size) {
                    pieces.push((fragment_start + start, fragment_start + end));
                }
                fragment_start = sentence_end;
            }
            // Otherwise this sentence starts a new fragment, continue accumulating
        }
        // else: sentence fits within max_size from fragment_start, keep accumulating
    }

    // Add the final fragment (text from fragment_start to end). The tail is
    // often a single unpunctuated sentence (chat, a list, a pasted URL), so an
    // oversized tail must be split rather than accepted as one giant fragment.
    if fragment_start < chars.len() {
        let final_text: String = chars[fragment_start..].iter().collect();
        if !final_text.trim().is_empty() {
            if chars.len() - fragment_start > max_size {
                for (start, end) in force_split_ranges(&final_text, max_size) {
                    pieces.push((fragment_start + start, fragment_start + end));
                }
            } else {
                pieces.push((fragment_start, chars.len()));
            }
        }
    }

    // Nothing splittable (e.g. whitespace-only input) — speak it verbatim. A
    // single computed piece must still be built: collapsing it back to the
    // whole text would discard the computed slice and lose source spans.
    if pieces.is_empty() {
        return vec![TextFragment::new(text.to_string(), 0, 1)];
    }

    build_fragments(text, &chars, pieces)
}

/// Assemble trimmed [`TextFragment`]s from untrimmed char ranges, preserving
/// exact UTF-16 source spans (trim offsets applied at the point of slicing,
/// never recovered by searching the text afterwards).
fn build_fragments(_text: &str, chars: &[char], pieces: Vec<(usize, usize)>) -> Vec<TextFragment> {
    let char_to_u16: Vec<usize> = std::iter::once(0)
        .chain(chars.iter().scan(0usize, |acc, ch| {
            *acc += ch.len_utf16();
            Some(*acc)
        }))
        .collect();
    let total = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| {
            let slice: String = chars[start..end].iter().collect();
            let trimmed = slice.trim();
            let leading = slice.chars().count() - slice.trim_start().chars().count();
            let trailing = slice.chars().count() - slice.trim_end().chars().count();
            TextFragment::with_source_span(
                trimmed.to_string(),
                index,
                total,
                char_to_u16[start + leading],
                char_to_u16[end - trailing],
            )
        })
        .collect()
}

/// Ceiling for a single unbreakable token. Tokens at or below this are kept
/// whole so a word is never severed; anything larger (a blob no engine will
/// accept anyway) is hard-split as a last resort. 4096 matches the provider
/// input limit already enforced in the synthesis path (OpenAI rejects past it).
const TOKEN_HARD_CAP: usize = 4096;

/// Fallback: split text into `max_size`-bounded ranges when no usable sentence
/// boundaries exist. Returns untrimmed char ranges into `text`.
///
/// Cuts land on whitespace whenever one is reachable, so a word is never broken
/// in half. A token longer than `max_size` is kept whole (up to
/// [`TOKEN_HARD_CAP`]) rather than split; only a token past that ceiling is
/// hard-cut, because no engine would accept it otherwise.
fn force_split_ranges(text: &str, max_size: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if max_size == 0 {
        return if len == 0 { Vec::new() } else { vec![(0, len)] };
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    while start < len {
        // Skip leading whitespace so a fragment never begins with a blank.
        while start < len && chars[start].is_whitespace() {
            start += 1;
        }
        if start >= len {
            break;
        }

        let limit = (start + max_size).min(len);
        if limit >= len {
            ranges.push((start, len));
            break;
        }

        // Prefer the last whitespace at or before the limit, then pull it just
        // past the boundary so the whitespace stays with the preceding piece.
        let end = match (start..limit).rev().find(|&i| chars[i].is_whitespace()) {
            Some(space) => {
                let mut cut = space + 1;
                while cut < limit && chars[cut].is_whitespace() {
                    cut += 1;
                }
                cut
            }
            None => {
                match (start + 1..limit)
                    .rev()
                    .find(|&index| is_cjk_clause_delimiter(chars[index]))
                {
                    Some(sep) => sep + 1,
                    None => {
                        // One unbreakable token: keep it whole so no word is
                        // severed, capped so an enormous blob is still bounded.
                        let mut token_end = start;
                        while token_end < len && !chars[token_end].is_whitespace() {
                            token_end += 1;
                        }
                        if token_end > start + TOKEN_HARD_CAP {
                            log::warn!(
                                "[Pagination] unbreakable token of {} chars exceeds {}; hard-splitting",
                                token_end - start,
                                TOKEN_HARD_CAP
                            );
                            start + TOKEN_HARD_CAP
                        } else {
                            token_end
                        }
                    }
                }
            }
        };

        if end <= start {
            // Defensive: never emit an empty range and never stall.
            ranges.push((start, limit));
            start = limit;
        } else {
            ranges.push((start, end));
            start = end;
        }
    }
    ranges
}

/// Full-width CJK clause punctuation. CJK prose has no inter-word whitespace, so
/// these are the only meaningful break opportunities inside a long run: a break
/// after a full-width comma reads naturally, an arbitrary character cut does not.
fn is_cjk_clause_delimiter(c: char) -> bool {
    matches!(
        c,
        // ，。！？ Ideographic comma/full stop + full-width ! ? ｜ 、
        '\u{FF0C}' | '\u{3002}' | '\u{FF01}' | '\u{FF1F}' | '\u{3001}'
            // ： ； （ ） 【 】 《 》 「 」 『 』
            | '\u{FF1A}' | '\u{FF1B}' | '\u{FF08}' | '\u{FF09}'
            | '\u{3010}' | '\u{3011}' | '\u{300A}' | '\u{300B}'
            | '\u{300C}' | '\u{300D}' | '\u{300E}' | '\u{300F}'
    )
}

/// Check if text should be paginated based on length and configuration.
pub fn should_paginate(text: &str, config: &PaginationConfig) -> bool {
    if !config.enabled {
        return false;
    }
    text.chars().count() > config.fragment_size as usize
}

/// Get the number of fragments that would be created for the given text.
#[allow(dead_code)]
pub fn estimate_fragment_count(text: &str, config: &PaginationConfig) -> usize {
    if !config.enabled || text.is_empty() {
        return 1;
    }

    let max_size = config.fragment_size as usize;
    let text_len = text.chars().count();

    if text_len <= max_size {
        return 1;
    }

    // Estimate based on sentence boundaries
    let boundaries = detect_sentence_boundaries(text);
    if boundaries.is_empty() {
        return (text_len + max_size - 1) / max_size; // Ceiling division
    }

    // Count how many fragments would be created (mirrors paginate_text logic)
    let mut count = 1;
    let mut fragment_start = 0;

    for (idx, boundary) in boundaries.iter().enumerate() {
        let sentence_end = boundary.position + 1;
        let current_fragment_len = sentence_end - fragment_start;

        if current_fragment_len > max_size {
            if idx > 0 {
                let prev_end = boundaries[idx - 1].position + 1;
                if prev_end > fragment_start {
                    count += 1;
                    fragment_start = prev_end;
                }
            }

            let lone_len = sentence_end - fragment_start;
            if lone_len > max_size {
                count += (lone_len + max_size - 1) / max_size - 1;
                fragment_start = sentence_end;
            }
        }
    }

    count
}

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod tests;
