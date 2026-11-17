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
    assert_eq!(
        highlight(source, spoken, "Second", 0).as_deref(),
        Some("Second")
    );
    assert_eq!(
        highlight(source, spoken, "First", 0).as_deref(),
        Some("First")
    );
}

/// The live 2026-09-10 failure: one `%` in a 1,944-character selection
/// expanded to " percent", and the whole reading lost word highlighting.
#[test]
fn percent_expansion_costs_only_the_inserted_word() {
    let source = "Around 10-20% of measles cases result in hospitalization.";
    let spoken =
        crate::sanitize::sanitize_text(source, &crate::config::SanitizationConfig::default());
    assert!(spoken.contains("percent"), "{spoken:?}");
    for word in ["Around", "10", "20", "of", "measles", "hospitalization"] {
        assert_eq!(
            highlight(source, &spoken, word, 0).as_deref(),
            Some(word),
            "{word:?} lost its origin"
        );
    }
    // The invented word points at the punctuation it was expanded from.
    assert_eq!(
        highlight(source, &spoken, "percent", 0).as_deref(),
        Some("%")
    );
}

#[test]
fn deletions_do_not_drift_the_words_after_them() {
    let source = "See https://example.com/x now, really now.";
    let spoken =
        crate::sanitize::sanitize_text(source, &crate::config::SanitizationConfig::default());
    assert_eq!(highlight(source, &spoken, "See", 0).as_deref(), Some("See"));
    assert_eq!(
        highlight(source, &spoken, "really", 0).as_deref(),
        Some("really")
    );
}

#[test]
fn same_length_folds_still_map_to_the_raw_form() {
    // Curly quotes and a Cyrillic homoglyph both fold 1 unit → 1 unit.
    assert_eq!(
        highlight(
            "The \u{201C}platform\u{201D} shipped",
            "The \"platform\" shipped",
            "platform",
            0
        )
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
    assert_eq!(
        alignment.source_span_utf16(3, 4),
        Some((7, 8)),
        "b, not the emoji"
    );
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
        let spoken =
            crate::sanitize::sanitize_text(source, &crate::config::SanitizationConfig::default());
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
    assert_eq!(
        alignment.source_span_utf16(0, 5),
        Some((0, 5)),
        "alpha still exact"
    );
    let (start, end) = alignment.source_span_utf16(6, 10).unwrap();
    assert_eq!(utf16_slice(source, (start, end)), "bravo charlie delta");
}

/// Widening gives each projected word the non-speech characters attached to
/// it: gaps between words ride with the word before them, the text's opening
/// characters ride with the first word, and whitespace separates the spans.
#[test]
fn widen_word_span_covers_attached_punctuation_without_stealing_words() {
    let raw_str = "Hello, world! Really.";
    let raw: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&raw, span));
    assert_eq!(wide((0, 5)), "Hello,");
    assert_eq!(wide((7, 12)), "world!");
    assert_eq!(wide((14, 20)), "Really.");
    // An opening quote before the first word belongs to it.
    let raw_str = "\"read this,\" he said";
    let quoted: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&quoted, span));
    assert_eq!(wide((1, 5)), "\"read");
    // The comma and the closing quote both sit flush after "this".
    assert_eq!(wide((6, 10)), "this,\"");
    assert_eq!(wide((13, 15)), "he");
}

#[test]
fn widen_word_span_stops_at_whitespace_and_keeps_surrogate_pairs_whole() {
    // A whitespace-flanked gap belongs to the word before it, so an em dash
    // between words rides with "one" and "two" starts clean.
    let raw_str = "one — two";
    let raw: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&raw, span));
    assert_eq!(wide((0, 3)), "one —");
    assert_eq!(wide((6, 9)), "two");
    // An emoji after a word is attached punctuation; neither half of the
    // surrogate pair may split off, and the span stays a char boundary.
    let raw_str = "nice \u{1F44D} work";
    let raw: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&raw, span));
    assert_eq!(wide((0, 4)), "nice \u{1F44D}");
    assert_eq!(wide((8, 12)), "work");
}

#[test]
fn widen_word_span_covers_cjk_and_trailing_symbols() {
    // Fullwidth punctuation is not alphanumeric, so the gap rides with the
    // word before it; the final word keeps its closing mark.
    let raw_str = "你好，世界！";
    let raw: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&raw, span));
    assert_eq!(wide((0, 2)), "你好，");
    assert_eq!(wide((3, 5)), "世界！");
    // Symbols after the last word, with no later word to stop at.
    let raw_str = "end...";
    let raw: Vec<u16> = raw_str.encode_utf16().collect();
    let wide = |span: Span| utf16_slice(raw_str, widen_word_span(&raw, span));
    assert_eq!(wide((0, 3)), "end...");
}
