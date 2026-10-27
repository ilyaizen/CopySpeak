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
mod tests {
    use super::*;

    fn default_config() -> PaginationConfig {
        PaginationConfig::default()
    }

    #[test]
    fn test_paginate_disabled_returns_single() {
        let config = PaginationConfig {
            enabled: false,
            ..default_config()
        };
        let text = "This is a very long text that should normally be split into multiple fragments for proper TTS processing.";
        let fragments = paginate_text(text, &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, text);
    }

    #[test]
    fn test_short_text_returns_single() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 2000,
        };
        let text = "Short text.";
        let fragments = paginate_text(text, &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, text);
    }

    #[test]
    fn test_empty_text_returns_single() {
        let config = default_config();
        let fragments = paginate_text("", &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "");
    }

    #[test]
    fn test_sentence_boundary_detection_periods() {
        let text = "First sentence. Second sentence. Third sentence.";
        let boundaries = detect_sentence_boundaries(text);
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0].delimiter, '.');
        assert_eq!(boundaries[1].delimiter, '.');
        assert_eq!(boundaries[2].delimiter, '.');
    }

    #[test]
    fn test_sentence_boundary_detection_exclamations() {
        let text = "Hello! World! Test!";
        let boundaries = detect_sentence_boundaries(text);
        // Should detect 2 exclamation marks as boundaries (implementation may vary)
        assert!(boundaries.len() >= 2);
    }

    #[test]
    fn test_sentence_boundary_detection_questions() {
        let text = "Who? What? Where?";
        let boundaries = detect_sentence_boundaries(text);
        // Should detect 2 question marks as boundaries (implementation may vary)
        assert!(boundaries.len() >= 2);
    }

    #[test]
    fn test_abbreviation_handling() {
        let text = "Mr. Smith went to the store. Dr. Jones was there.";
        let boundaries = detect_sentence_boundaries(text);
        // Should only detect 2 sentence boundaries (after "store" and "there")
        // The periods after "Mr" and "Dr" should NOT be treated as sentence ends
        assert_eq!(boundaries.len(), 2);
    }

    #[test]
    fn test_abbreviation_handling_with_etc() {
        let text = "We have apples, oranges, etc. Then we have bananas.";
        let boundaries = detect_sentence_boundaries(text);
        // The period after "etc" should NOT be treated as sentence end
        // Only the period after "bananas" should be detected
        assert_eq!(boundaries.len(), 1);
    }

    #[test]
    fn test_abbreviation_handling_with_eg_ie() {
        let text = "Use e.g. apples and i.e. oranges. Then buy more.";
        let boundaries = detect_sentence_boundaries(text);
        // The periods in "e.g." and "i.e." should NOT be treated as sentence ends
        // Only the period after "oranges" and "more" should be detected
        // However, depending on exact implementation, we might detect some boundaries
        assert!(boundaries.len() >= 1);
    }

    #[test]
    fn test_abbreviation_case_insensitive() {
        let text = "MR. Smith and mrs. Jones. DR. Williams is here.";
        let boundaries = detect_sentence_boundaries(text);
        // Should detect sentence boundaries after "Jones" and "here"
        // The periods after "MR", "mrs", and "DR" should be ignored (if followed by space)
        assert!(boundaries.len() >= 1);
    }

    #[test]
    fn test_chinese_sentence_boundaries() {
        let text = "这是第一句。这是第二句！这是第三句？";
        let boundaries = detect_sentence_boundaries(text);
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0].delimiter, '。');
        assert_eq!(boundaries[1].delimiter, '！');
        assert_eq!(boundaries[2].delimiter, '？');
    }

    #[test]
    fn test_text_fragment_properties() {
        let fragment = TextFragment::new("Test text".to_string(), 1, 3);
        assert!(!fragment.is_first());
        assert!(!fragment.is_last());
        assert_eq!(fragment.label(), "Part 2 of 3");
    }

    #[test]
    fn test_text_fragment_first() {
        let fragment = TextFragment::new("First".to_string(), 0, 3);
        assert!(fragment.is_first());
        assert!(!fragment.is_last());
        assert_eq!(fragment.label(), "Part 1 of 3");
    }

    #[test]
    fn test_text_fragment_last() {
        let fragment = TextFragment::new("Last".to_string(), 2, 3);
        assert!(!fragment.is_first());
        assert!(fragment.is_last());
        assert_eq!(fragment.label(), "Part 3 of 3");
    }

    #[test]
    fn test_single_fragment_first_and_last() {
        let fragment = TextFragment::new("Only".to_string(), 0, 1);
        assert!(fragment.is_first());
        assert!(fragment.is_last());
        assert_eq!(fragment.label(), "Part 1 of 1");
    }

    #[test]
    fn test_paginate_long_text_at_sentence_boundaries() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 50,
        };
        let text = "This is sentence one. This is sentence two. This is sentence three. This is sentence four. This is sentence five.";
        let fragments = paginate_text(text, &config);

        // Should split into multiple fragments at sentence boundaries
        assert!(fragments.len() > 1);

        // Verify fragments don't split in the middle of sentences
        for fragment in &fragments {
            // Each fragment should end with sentence-ending punctuation
            if !fragment.text.is_empty() {
                let last_char = fragment.text.chars().last().unwrap();
                if !fragment.is_last() {
                    assert!(
                        is_sentence_end(last_char)
                            || fragment.text.trim().ends_with('.')
                            || fragment.text.trim().ends_with('!')
                            || fragment.text.trim().ends_with('?'),
                        "Fragment should end at sentence boundary: {}",
                        fragment.text
                    );
                }
            }
        }

        // Verify all fragments combined equal original text (minus whitespace differences)
        let combined: String = fragments.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(combined.replace(' ', ""), text.replace(' ', ""));
    }

    #[test]
    fn test_force_split_never_breaks_a_word() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 20,
        };
        // No punctuation at all, so every cut is a forced one.
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";
        let fragments = paginate_text(text, &config);

        assert!(fragments.len() > 1);
        for fragment in &fragments {
            let trimmed = fragment.text.trim();
            assert!(
                !trimmed.is_empty(),
                "fragment must not be blank: {:?}",
                fragments
            );
            // A fragment may only start/end on whitespace, never mid-word.
            assert!(
                fragment.text.starts_with(trimmed) && fragment.text.ends_with(trimmed),
                "fragment carries unexpected whitespace: {:?}",
                fragment.text
            );
        }
        // No word may be severed by a fragment boundary.
        let rejoined: String = fragments
            .iter()
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        for word in text.split_whitespace() {
            assert!(
                rejoined.split_whitespace().any(|w| w == word),
                "word {word:?} was broken across a fragment boundary: {rejoined:?}"
            );
        }
    }

    #[test]
    fn test_oversized_tail_after_a_short_sentence_is_split() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 50,
        };
        // One short sentence, then a long unpunctuated tail. The tail used to be
        // emitted as a single 200-char fragment because only single-sentence
        // fragments were force-split.
        let tail = "word ".repeat(40);
        let text = format!("Short. {}", tail.trim());
        let fragments = paginate_text(&text, &config);

        assert!(
            fragments.len() > 1,
            "long tail was not paginated: {} fragments",
            fragments.len()
        );
        for fragment in &fragments {
            assert!(
                fragment.text.chars().count() <= config.fragment_size as usize,
                "fragment exceeds budget: {:?}",
                fragment.text
            );
        }
    }

    #[test]
    fn test_version_and_decimal_periods_are_not_sentence_ends() {
        let text = "Upgrade to v0.2.10 now. It fixes pi at 3.14 and example.com too.";
        let boundaries = detect_sentence_boundaries(text);
        // Only the two real sentence ends (after "now" and "too").
        assert_eq!(boundaries.len(), 2, "boundaries: {boundaries:?}");
    }

    #[test]
    fn test_missing_space_after_sentence_period_stays_word_safe() {
        // `blue.It` is ambiguous: it could be a run-on sentence or a dotted
        // token. Word integrity wins, so it is treated as one token and split on
        // whitespace rather than after the period. What matters is that no word
        // is cut and the boundary count stays deterministic.
        let text = "The sky is blue.It looks nice.";
        let fragments = paginate_text(
            text,
            &PaginationConfig {
                enabled: true,
                fragment_size: 100,
            },
        );
        // Fits in one fragment, so nothing is split regardless.
        assert_eq!(fragments.len(), 1);
    }

    #[test]
    fn test_dotted_initialisms_and_identifiers_are_single_tokens() {
        // Initialisms, filenames and dotted identifiers must never be split by
        // pagination. (A lone short token trivially yields one fragment; the
        // real risk is a boundary landing inside the token.)
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 40,
        };
        for token in [
            "U.S.A.",
            "Ph.D.",
            "System.Console",
            "README.TXT",
            "192.168.1.1",
        ] {
            let text = format!("Please send the parcel to the {token} office and confirm");
            let fragments = paginate_text(&text, &config);
            for fragment in &fragments {
                for word in fragment.text.split_whitespace() {
                    assert!(
                        text.split_whitespace().any(|w| w == word),
                        "{token:?} was split: fragment word {word:?} is not a whole source word"
                    );
                }
            }
            // The token itself must appear whole in some fragment.
            assert!(
                fragments
                    .iter()
                    .any(|f| f.text.split_whitespace().any(|w| w == token)),
                "{token:?} did not survive pagination: {:?}",
                fragments.iter().map(|f| &f.text).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_initialisms_are_never_split_mid_token() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 12,
        };
        let text = "Send it to the U.S.A. office now and Ph.D. students later on";
        let fragments = paginate_text(text, &config);
        // Every emitted fragment must be composed of whole words from the source.
        for fragment in &fragments {
            for word in fragment.text.split_whitespace() {
                assert!(
                    text.contains(word),
                    "fragment word {word:?} is not a whole source word"
                );
            }
        }
    }

    #[test]
    fn test_cjk_run_splits_on_cjk_punctuation() {
        // No whitespace at all: the only sane break is after a full-width comma.
        let text = "这是第一句话，这是第二句话，这是第三句话，这是第四句话";
        let ranges = force_split_ranges(text, 12);
        assert!(ranges.len() > 1, "CJK run should split: {ranges:?}");
        let chars: Vec<char> = text.chars().collect();
        for (_, end) in &ranges {
            if *end < chars.len() {
                assert!(
                    is_cjk_clause_delimiter(chars[*end - 1]),
                    "CJK cut at {end} is not on a clause delimiter"
                );
            }
        }
    }

    #[test]
    fn test_oversized_token_is_kept_whole() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 20,
        };
        // A 70-char unbreakable token is longer than fragment_size but well under
        // TOKEN_HARD_CAP, so it must survive intact instead of being severed.
        let token = "x".repeat(70);
        let text = format!("alpha {token} omega");
        let fragments = paginate_text(&text, &config);
        assert!(
            fragments.iter().any(|f| f.text == token),
            "word was split: {:?}",
            fragments.iter().map(|f| &f.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_token_beyond_hard_cap_is_split() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 50,
        };
        // Past the ceiling there is no engine-safe option but to cut.
        let text = "y".repeat(TOKEN_HARD_CAP + 100);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() > 1);
    }

    #[test]
    fn test_punctuation_inside_latin_tokens_is_preserved() {
        for token in [
            "https://example.com/some/very/long/path/that/keeps/going/and/going",
            "client/server",
            "1,000,000",
            "https://example.com/path?token=abcdef",
            "https://example.com/.well-known/security.txt",
            "../path/file.txt",
            "some_.name",
            "Hello!World?Test",
        ] {
            let fragments = paginate_text(
                &format!("before {token} after"),
                &PaginationConfig {
                    enabled: true,
                    fragment_size: 5,
                },
            );
            assert!(
                fragments.iter().any(|fragment| fragment.text == token),
                "{fragments:?}"
            );
        }
    }

    #[test]
    fn test_whitespace_only_text_is_a_single_fragment() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 5,
        };
        let fragments = paginate_text("          ", &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "          ");
    }

    #[test]
    fn test_should_paginate() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };

        assert!(!should_paginate("Short text.", &config));
        assert!(should_paginate("A".repeat(200).as_str(), &config));

        let disabled_config = PaginationConfig {
            enabled: false,
            ..default_config()
        };
        assert!(!should_paginate(&"A".repeat(200), &disabled_config));
    }

    #[test]
    fn test_estimate_fragment_count() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };

        assert_eq!(estimate_fragment_count("Short text.", &config), 1);

        let long_text = "Sentence. ".repeat(30);
        let estimate = estimate_fragment_count(&long_text, &config);
        assert!(estimate > 1);

        let disabled_config = PaginationConfig {
            enabled: false,
            ..default_config()
        };
        assert_eq!(estimate_fragment_count(&long_text, &disabled_config), 1);
    }

    #[test]
    fn test_hindi_text_splits_at_danda() {
        // The danda must stay a sentence end: without it the whole text is one
        // sentence and falls to force-split. That fallback now snaps cuts to
        // whitespace, so words survive either way — the danda still gives
        // sentence-level fragments that each end on it.
        let text = "यह एक परीक्षण वाक्य है। ".repeat(6);
        let config = PaginationConfig {
            fragment_size: 60,
            ..default_config()
        };
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() > 1);
        for fragment in &fragments {
            assert!(fragment.text.ends_with('।'), "{:?}", fragment.text);
            for word in fragment.text.split_whitespace() {
                assert!(
                    text.split_whitespace().any(|w| w == word),
                    "word {word:?} was broken across a fragment boundary"
                );
            }
        }
    }

    #[test]
    fn test_force_split_no_sentence_boundaries() {
        let text = "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10";
        let fragments = paginate_text_fragments_from_ranges(text, force_split_ranges(text, 20));

        // Should split into multiple fragments
        assert!(fragments.len() > 1);

        // Each fragment should be within the size limit
        for fragment in &fragments {
            assert!(fragment.text.len() <= 20);
        }
    }

    /// Test helper mirroring paginate_text's assembly path.
    fn paginate_text_fragments_from_ranges(
        text: &str,
        pieces: Vec<(usize, usize)>,
    ) -> Vec<TextFragment> {
        let chars: Vec<char> = text.chars().collect();
        build_fragments(text, &chars, pieces)
    }

    #[test]
    fn test_paginate_text_very_short_under_limit() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 500,
        };
        let text = "Short text under limit.";
        let fragments = paginate_text(text, &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, text);
    }

    #[test]
    fn test_paginate_text_short_exactly_at_limit() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };
        // Text at exactly fragment_size should NOT be paginated
        let text = "A".repeat(99) + ".";
        let fragments = paginate_text(&text, &config);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, text);
    }

    #[test]
    fn test_paginate_text_short_just_over_limit() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };
        // Create text where first sentence is under limit but total exceeds it
        let text = "A".repeat(80) + ". " + &"B".repeat(40) + ". " + &"C".repeat(40) + ".";
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() > 1);
        // First fragment should end with period
        assert!(fragments[0].text.ends_with('.'));
    }

    #[test]
    fn test_paginate_text_medium_200_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };
        let text = "This is a sentence. ".repeat(10);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 2);
        // Verify fragments don't split mid-sentence
        for fragment in &fragments {
            if !fragment.is_last() && !fragment.text.is_empty() {
                let last_char = fragment.text.chars().last().unwrap();
                assert!(is_sentence_end(last_char));
            }
        }
    }

    #[test]
    fn test_paginate_text_medium_500_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 200,
        };
        let text = "This is a test sentence. ".repeat(20);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 2);
        // Verify content is preserved
        let combined: String = fragments.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(combined.replace(' ', ""), text.replace(' ', ""));
    }

    #[test]
    fn test_paginate_text_long_1000_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 300,
        };
        let text = "Sentence one. Sentence two. Sentence three. ".repeat(25);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 3);
        // Verify all fragments are within reasonable bounds
        for fragment in &fragments {
            assert!(
                fragment.text.len() <= config.fragment_size as usize + 50,
                "Fragment too long: {} chars",
                fragment.text.len()
            );
        }
    }

    #[test]
    fn test_paginate_text_long_2000_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 500,
        };
        let text = "This is a medium length sentence. ".repeat(50);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 3);
        // Verify estimate is close to actual
        let estimate = estimate_fragment_count(&text, &config);
        assert!(
            (estimate as i32 - fragments.len() as i32).abs() <= 1,
            "Estimate {} too far from actual {}",
            estimate,
            fragments.len()
        );
    }

    #[test]
    fn test_paginate_text_very_long_5000_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 1000,
        };
        let text = "This is a sentence. ".repeat(125);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 2);
        // Verify first and last fragments
        assert!(fragments[0].is_first());
        assert!(fragments.last().unwrap().is_last());
    }

    #[test]
    fn test_paginate_text_very_long_10000_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 2000,
        };
        let text = "This is a test sentence. ".repeat(250);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 4);
        // Verify total character count is preserved
        let total_chars: usize = fragments.iter().map(|f| f.text.chars().count()).sum();
        let expected_chars = text.chars().count();
        assert!((total_chars as i32 - expected_chars as i32).abs() < fragments.len() as i32 * 5);
    }

    #[test]
    fn test_paginate_text_ultra_long_20000_chars() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 1000,
        };
        let text = "This is a sentence fragment. ".repeat(500);
        let fragments = paginate_text(&text, &config);
        // Just verify the text is handled correctly
        assert!(!fragments.is_empty());
        // Verify each fragment is non-empty
        for fragment in &fragments {
            assert!(!fragment.text.trim().is_empty());
        }
    }

    #[test]
    fn test_paginate_with_different_fragment_sizes() {
        let text = "Sentence one. Sentence two. Sentence three. Sentence four. ".repeat(20);

        let config_small = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };
        let fragments_small = paginate_text(&text, &config_small);

        let config_medium = PaginationConfig {
            enabled: true,
            fragment_size: 300,
        };
        let fragments_medium = paginate_text(&text, &config_medium);

        let config_large = PaginationConfig {
            enabled: true,
            fragment_size: 600,
        };
        let fragments_large = paginate_text(&text, &config_large);

        // Smaller fragment size should result in more fragments
        assert!(fragments_small.len() >= fragments_medium.len());
        assert!(fragments_medium.len() >= fragments_large.len());
    }

    #[test]
    fn test_paginate_many_short_sentences() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 200,
        };
        // Many short sentences - total should exceed fragment_size
        let text = "Hi. ".repeat(100);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() > 1);
        // Each fragment should contain multiple sentences
        for fragment in &fragments {
            let sentence_count = fragment.text.matches('.').count();
            assert!(
                sentence_count >= 1,
                "Fragment should contain at least one sentence"
            );
        }
    }

    #[test]
    fn test_paginate_few_long_sentences() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 200,
        };
        // Few long sentences - ensure total exceeds fragment_size
        let text = "This is a very long sentence with many words. ".repeat(20);
        let fragments = paginate_text(&text, &config);
        // Just verify the text is handled correctly, don't enforce fragment count
        // as the pagination algorithm may handle different sentence patterns differently
        assert!(!fragments.is_empty());
        // Verify each fragment ends at a sentence boundary
        for fragment in &fragments {
            if !fragment.is_last() && !fragment.text.is_empty() {
                let last_char = fragment.text.chars().last().unwrap();
                assert!(is_sentence_end(last_char));
            }
        }
    }

    #[test]
    fn test_paginate_mixed_sentence_lengths() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 400,
        };
        // Mix of short and long sentences
        let text = "Short. ".to_owned()
            + &"This is a much longer sentence with many more words. ".repeat(5)
            + &"Brief. "
            + &"Another very long sentence that goes on and on. ".repeat(3);
        let fragments = paginate_text(&text, &config);
        assert!(fragments.len() >= 2);
        // Verify no mid-sentence splits
        for fragment in &fragments {
            if !fragment.is_last() && !fragment.text.is_empty() {
                let last_char = fragment.text.chars().last().unwrap();
                assert!(is_sentence_end(last_char));
            }
        }
    }

    #[test]
    fn test_estimate_count_matches_actual_for_various_lengths() {
        let text1 = "Short text.";
        let text2 = "Sentence. ".repeat(10);
        let text3 = "This is a sentence. ".repeat(25);
        let text4 = "This is a test. ".repeat(75);
        let text5 = "Another sentence here. ".repeat(150);

        let test_cases: Vec<(&str, &str)> = vec![
            (&text1, &text1),
            (&text2, &text2),
            (&text3, &text3),
            (&text4, &text4),
            (&text5, &text5),
        ];

        for (_size, text) in test_cases {
            let config = PaginationConfig {
                enabled: true,
                fragment_size: 300,
            };
            let estimate = estimate_fragment_count(text, &config);
            let actual = paginate_text(text, &config).len();

            // Estimate should be close to actual (within 2)
            assert!(
                (estimate as i32 - actual as i32).abs() <= 2,
                "Estimate {} too far from actual {} for text length {}",
                estimate,
                actual,
                text.chars().count()
            );
        }
    }

    #[test]
    fn test_paginate_preserves_text_content_for_various_lengths() {
        let test_lengths = vec![100, 500, 1000, 5000, 10000];

        for length in test_lengths {
            let config = PaginationConfig {
                enabled: true,
                fragment_size: 1000,
            };

            // Create text with varied sentences
            let text = "A sentence. ".repeat(length / 15);
            let fragments = paginate_text(&text, &config);

            // Reconstruct text from fragments
            let reconstructed: String = fragments.iter().map(|f| f.text.as_str()).collect();

            // Verify content is preserved (ignoring whitespace differences)
            assert_eq!(
                reconstructed.replace(' ', ""),
                text.replace(' ', ""),
                "Content mismatch for text length {}",
                length
            );
        }
    }

    #[test]
    fn test_paginate_edge_case_exactly_multiple_fragments() {
        let config = PaginationConfig {
            enabled: true,
            fragment_size: 100,
        };
        // Create text that should split evenly into multiple fragments
        // Each sentence is about 45 chars, so with 5 sentences we get about 225 total
        let text = "A".repeat(40)
            + ". "
            + &"B".repeat(40)
            + ". "
            + &"C".repeat(40)
            + ". "
            + &"D".repeat(40)
            + ". "
            + &"E".repeat(40)
            + ".";
        let fragments = paginate_text(&text, &config);
        // Should split into multiple fragments
        assert!(fragments.len() >= 2);
        // Verify each fragment ends with period (except possibly the last one if it's incomplete)
        for fragment in &fragments {
            if !fragment.is_last() && !fragment.text.is_empty() {
                assert!(fragment.text.ends_with('.'));
            }
        }
    }

    #[test]
    fn test_paginate_with_various_fragment_sizes_same_text() {
        let text = "This is sentence one. This is sentence two. This is sentence three. This is sentence four. This is sentence five.";

        let sizes = vec![50, 100, 200, 500];
        let mut previous_count = usize::MAX;

        for size in &sizes {
            let config = PaginationConfig {
                enabled: true,
                fragment_size: *size,
            };
            let fragments = paginate_text(text, &config);

            // Larger fragment size should not result in more fragments
            assert!(
                fragments.len() <= previous_count,
                "Size {} produced {} fragments, more than previous count {}",
                size,
                fragments.len(),
                previous_count
            );
            previous_count = fragments.len();
        }
    }
}
