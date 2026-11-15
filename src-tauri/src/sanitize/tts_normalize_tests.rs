use super::*;

#[test]
fn test_normalize_curly_quotes_and_dashes() {
    assert_eq!(
        normalize_characters("\u{201C}quoted\u{201D} and \u{2018}single\u{2019}"),
        "\"quoted\" and 'single'"
    );
    assert_eq!(normalize_characters("co\u{2013}op"), "co-op");
    assert_eq!(normalize_characters("a\u{00A0}b\u{202F}c"), "a b c");
}

#[test]
fn test_normalize_fullwidth_folds_to_ascii() {
    assert_eq!(
        normalize_characters("Ｈｅｌｌｏ　Ｗｏｒｌｄ"),
        "Hello World"
    );
}

#[test]
fn test_normalize_homoglyphs_fold_latin_lookalikes() {
    // Cyrillic с, а fold to Latin inside a word that holds Latin letters;
    // real Cyrillic words survive untouched, lookalike letters included.
    assert_eq!(normalize_characters("саt"), "cat");
    assert_eq!(normalize_characters("жизнь"), "жизнь");
    assert_eq!(normalize_characters("привет саt"), "привет cat");
    assert_eq!(normalize_characters("Россия"), "Россия");
}

#[test]
fn test_normalize_strips_invisibles() {
    // ZWSP, soft hyphen, BOM are deletions.
    assert_eq!(
        normalize_characters("in\u{200B}vis\u{00AD}ible"),
        "invisible"
    );
    assert_eq!(normalize_characters("\u{FEFF}leading"), "leading");
}

#[test]
fn test_sanitize_keeps_zero_width_non_joiner() {
    // Persian "می‌خواهم" is spelled with a ZWNJ; dropping it fuses the word.
    assert_eq!(sanitize_tts("می\u{200C}خواهم"), "می\u{200C}خواهم");
}

#[test]
fn test_strip_leading_orphans_acceptance_example() {
    // The acceptance example from the browser-companion handoff.
    assert_eq!(
        strip_leading_orphans(", They didn't know that they were looking for a better theory."),
        "They didn't know that they were looking for a better theory."
    );
    // Multiple orphans + leading whitespace.
    assert_eq!(strip_leading_orphans(" ••  Point one"), "Point one");
    // Opening quote is NOT an orphan and survives.
    assert_eq!(
        strip_leading_orphans("\"Quoted start\""),
        "\"Quoted start\""
    );
}

#[test]
fn test_sanitize_tts_acceptance_example() {
    // End-to-end: leading comma + curly apostrophe (mojibake source in
    // TTS) → clean straight-quote output.
    assert_eq!(
        sanitize_tts(", They didn\u{2019}t know that they were looking for a better theory."),
        "They didn't know that they were looking for a better theory."
    );
}

#[test]
fn test_remove_unpronounceable() {
    assert_eq!(remove_unpronounceable("Hello 😀 world"), "Hello  world");
    assert_eq!(remove_unpronounceable("🎉🎊 Party time"), " Party time");
    assert_eq!(remove_unpronounceable("No emojis here"), "No emojis here");
    assert_eq!(remove_unpronounceable("🌍 Earth 🌏"), " Earth ");
    assert_eq!(remove_unpronounceable("Text ✅ check"), "Text  check");
}

/// Glyphs outside the classic emoji ranges reached the engines before and
/// are rendered as beep/noise artifacts instead of being skipped.
#[test]
fn test_remove_unpronounceable_strips_symbol_scripts() {
    assert_eq!(remove_unpronounceable("A⭐B★C→D∑E①F■G"), "ABCDEFG");
    assert_eq!(remove_unpronounceable("⌚ watch ⏩ now"), " watch  now");
    assert_eq!(remove_unpronounceable("🀄🂠🄰 tiles"), " tiles");
    assert_eq!(
        remove_unpronounceable("wow‼ really⁉ infoℹ"),
        "wow really info"
    );
    assert_eq!(remove_unpronounceable("🇺🇸 US flag"), " US flag");
}

#[test]
fn test_remove_unpronounceable_strips_control_and_invisible() {
    assert_eq!(remove_unpronounceable("x\u{E000}y"), "xy");
    assert_eq!(remove_unpronounceable("bad \u{FFFD} data"), "bad  data");
    assert_eq!(remove_unpronounceable("hy\u{00AD}phen"), "hyphen");
    assert_eq!(remove_unpronounceable("a\u{200B}b"), "ab");
    assert_eq!(remove_unpronounceable("\u{FEFF}text"), "text");
    assert_eq!(remove_unpronounceable("a\u{0007}\u{009F}b"), "ab");
    // Keycap sequence keeps its digit, drops the presentation marks.
    assert_eq!(remove_unpronounceable("1\u{FE0F}\u{20E3}"), "1");
}

#[test]
fn test_remove_unpronounceable_normalizes_whitespace_controls_to_spaces() {
    assert_eq!(remove_unpronounceable("foo\tbar"), "foo bar");
    assert_eq!(remove_unpronounceable("a\u{000B}b\u{000C}c"), "a b c");
}

#[test]
fn test_remove_unpronounceable_keeps_speech_content() {
    assert_eq!(
        remove_unpronounceable("it's — ok… \"quoted\""),
        "it's — ok… \"quoted\""
    );
    assert_eq!(remove_unpronounceable("a、b。c「d」"), "a、b。c「d」");
    assert_eq!(remove_unpronounceable("50% + $5"), "50% + $5");
}

#[test]
fn test_sanitize_tts_strips_newlines() {
    assert_eq!(sanitize_tts("line one\nline two"), "line one line two");
    assert_eq!(sanitize_tts("a\nb\nc"), "a b c");
    assert_eq!(sanitize_tts("a\r\nb"), "a b");
    assert_eq!(sanitize_tts("no newlines"), "no newlines");
}

#[test]
fn test_sanitize_tts_urls() {
    assert_eq!(
        remove_urls("Visit https://example.com for info"),
        "Visit  for info"
    );
    assert_eq!(remove_urls("See http://foo.bar/path?q=1 here"), "See  here");
    assert_eq!(remove_urls("No URLs here"), "No URLs here");
}

#[test]
fn test_sanitize_tts_citations() {
    assert_eq!(
        remove_citations("According to [1] and [12]"),
        "According to  and "
    );
    assert_eq!(remove_citations("No citations"), "No citations");
    assert_eq!(
        remove_citations("See [a] and [b] also [xyz]"),
        "See  and  also "
    );
    assert_eq!(remove_citations("Mixed [1] [a] [12] [abc]"), "Mixed    ");
}

#[test]
fn test_sanitize_tts_slash_lookup() {
    assert_eq!(expand_slash_lookups("w/o any issue"), "without any issue");
    assert_eq!(expand_slash_lookups("w/ sugar"), "with sugar");
    assert_eq!(expand_slash_lookups("n/a"), "not applicable");
    assert_eq!(expand_slash_lookups("and/or both"), "and or both");
}

#[test]
fn test_sanitize_tts_slash_option() {
    assert_eq!(
        expand_slash_options("true/false value"),
        "true or false value"
    );
    assert_eq!(
        expand_slash_options("yes/no question"),
        "yes or no question"
    );
}

#[test]
fn test_sanitize_tts_slash_ratio() {
    assert_eq!(expand_slash_ratios("100 km/h speed"), "100 km per h speed");
    assert_eq!(
        expand_slash_ratios("9.8 m/s acceleration"),
        "9.8 m per s acceleration"
    );
}

#[test]
fn test_sanitize_tts_latin_abbreviations() {
    assert_eq!(
        expand_latin_abbreviations("e.g. this one"),
        "for example this one"
    );
    assert_eq!(
        expand_latin_abbreviations("i.e. that is"),
        "that is that is"
    );
    assert_eq!(
        expand_latin_abbreviations("and etc. more"),
        "and et cetera more"
    );
    assert_eq!(expand_latin_abbreviations("A vs. B"), "A versus B");
}

#[test]
fn test_sanitize_tts_title_abbreviations() {
    assert_eq!(expand_title_abbreviations("Dr. Smith"), "Doctor Smith");
    assert_eq!(expand_title_abbreviations("Mr. Johnson"), "Mister Johnson");
    assert_eq!(expand_title_abbreviations("Mrs. Davis"), "Misses Davis");
    assert_eq!(expand_title_abbreviations("Ms. Williams"), "Miss Williams");
    assert_eq!(expand_title_abbreviations("Prof. Brown"), "Professor Brown");
    assert_eq!(expand_title_abbreviations("Rev. White"), "Reverend White");
    assert_eq!(
        expand_title_abbreviations("Sr. Martinez"),
        "Senior Martinez"
    );
    assert_eq!(
        expand_title_abbreviations("Jr. King Jr."),
        "Junior King Junior "
    );
    assert_eq!(expand_title_abbreviations("Hon. Lee"), "Honorable Lee");
    assert_eq!(expand_title_abbreviations("Sir John"), "Sir John");
    assert_eq!(
        expand_title_abbreviations("Madam President"),
        "Madam President"
    );
}

#[test]
fn test_sanitize_tts_metric_units() {
    assert_eq!(expand_metric_units("10mm gap"), "10 millimeters gap");
    assert_eq!(expand_metric_units("5 cm wide"), "5 centimeters wide");
    assert_eq!(expand_metric_units("3km away"), "3 kilometers away");
    assert_eq!(expand_metric_units("2kg of rice"), "2 kilograms of rice");
    assert_eq!(expand_metric_units("500g pack"), "500 grams pack");
    assert_eq!(expand_metric_units("10 m tall"), "10 meters tall");
    // Should NOT replace 'm' in regular words
    assert_eq!(expand_metric_units("maximum"), "maximum");
}

#[test]
fn test_sanitize_tts_number_suffixes() {
    assert_eq!(expand_number_suffixes("2bn revenue"), "2 billion revenue");
    assert_eq!(expand_number_suffixes("1.5bn users"), "1.5 billion users");
    assert_eq!(expand_number_suffixes("$2bn deal"), "$2 billion deal");
    assert_eq!(
        expand_number_suffixes("3B valuation"),
        "3 billion valuation"
    );
    assert_eq!(expand_number_suffixes("5m users"), "5 million users");
    assert_eq!(
        expand_number_suffixes("2.5m downloads"),
        "2.5 million downloads"
    );
    assert_eq!(
        expand_number_suffixes("10M followers"),
        "10 million followers"
    );
    assert_eq!(
        expand_number_suffixes("3k subscribers"),
        "3 thousand subscribers"
    );
    assert_eq!(expand_number_suffixes("50K views"), "50 thousand views");
    assert_eq!(expand_number_suffixes("2.5k stars"), "2.5 thousand stars");
    assert_eq!(
        expand_number_suffixes("1tr market cap"),
        "1 trillion market cap"
    );
    assert_eq!(expand_number_suffixes("2TR debt"), "2 trillion debt");
    assert_eq!(expand_number_suffixes("maximum"), "maximum");
    assert_eq!(expand_number_suffixes("bank"), "bank");
    assert_eq!(expand_number_suffixes("milk"), "milk");
}

#[test]
fn test_sanitize_tts_number_suffixes_edge_cases() {
    assert_eq!(expand_number_suffixes("5 m users"), "5 million users");
    assert_eq!(
        expand_number_suffixes("2 k followers"),
        "2 thousand followers"
    );
    assert_eq!(
        expand_number_suffixes("from 5m to 10m users"),
        "from 5 million to 10 million users"
    );
    assert_eq!(
        expand_number_suffixes("Revenue grew from $2bn to $5bn"),
        "Revenue grew from $2 billion to $5 billion"
    );
}

#[test]
fn test_number_suffixes_before_metric_units() {
    let result = sanitize_tts("The company has 5m users");
    assert!(
        result.contains("million"),
        "Expected 'million' in: {}",
        result
    );

    let result = sanitize_tts("The distance is 5km");
    assert!(
        result.contains("kilometers"),
        "Expected 'kilometers' in: {}",
        result
    );
}

#[test]
fn test_sanitize_tts_symbols() {
    assert!(expand_symbols("A & B").contains("and"));
    assert!(expand_symbols("50%").contains("percent"));
    assert!(expand_symbols("~100").contains("approximately"));
    assert!(expand_symbols("2 + 3").contains("plus"));
    assert!(expand_symbols("x = 5").contains("equals"));
    assert!(expand_symbols("90°").contains("degrees"));
}

#[test]
fn test_sanitize_tts_currency_symbols() {
    assert_eq!(expand_symbols("$50"), "50 dollars");
    assert_eq!(expand_symbols("€20"), "20 euros");
    assert_eq!(expand_symbols("£15"), "15 pounds");
    assert_eq!(expand_symbols("¥1000"), "1000 yen");
}

#[test]
fn test_sanitize_tts_currency_symbols_decimals() {
    assert_eq!(expand_symbols("$19.99"), "19.99 dollars");
    assert_eq!(expand_symbols("€12.50"), "12.50 euros");
    assert_eq!(expand_symbols("£3.75"), "3.75 pounds");
}

#[test]
fn test_sanitize_tts_currency_symbols_in_text() {
    assert_eq!(
        expand_symbols("The price is $50"),
        "The price is 50 dollars"
    );
    assert_eq!(
        expand_symbols("It costs €20 total"),
        "It costs 20 euros total"
    );
    assert_eq!(
        expand_symbols("You owe £15 to me"),
        "You owe 15 pounds to me"
    );
}

#[test]
fn test_sanitize_tts_currency_symbols_multiple() {
    assert_eq!(expand_symbols("$50 and $100"), "50 dollars and 100 dollars");
    assert_eq!(expand_symbols("€20 to €30"), "20 euros to 30 euros");
}

#[test]
fn test_sanitize_tts_currency_symbols_with_commas() {
    assert_eq!(expand_symbols("$1,000"), "1 dollars,000");
    assert_eq!(expand_symbols("€10,000"), "10 euros,000");
    assert_eq!(expand_symbols("£1,234,567"), "1 pounds,234,567");
}

#[test]
fn test_sanitize_tts_currency_symbols_at_end() {
    assert_eq!(
        expand_symbols("The total is $50"),
        "The total is 50 dollars"
    );
    assert_eq!(expand_symbols("It costs €20"), "It costs 20 euros");
    assert_eq!(expand_symbols("Price: £15"), "Price: 15 pounds");
}

#[test]
fn test_sanitize_tts_currency_symbols_mixed() {
    assert_eq!(
        expand_symbols("$50 and €20 and £15"),
        "50 dollars and 20 euros and 15 pounds"
    );
    assert_eq!(
        expand_symbols("Convert $100 to €85"),
        "Convert 100 dollars to 85 euros"
    );
}

#[test]
fn test_sanitize_tts_currency_symbols_edge_cases() {
    assert_eq!(expand_symbols("$0"), "0 dollars");
    assert_eq!(expand_symbols("€.50"), "€.50");
    assert_eq!(expand_symbols("$"), "$");
    assert_eq!(expand_symbols("$$$"), "$$$");
    assert_eq!(expand_symbols("50$"), "50$");
}

#[test]
fn test_sanitize_tts_currency_symbols_no_space_after() {
    assert_eq!(expand_symbols("Price:$50"), "Price:50 dollars");
    assert_eq!(expand_symbols("Cost(€20)"), "Cost(20 euros)");
}

#[test]
fn test_sanitize_tts_number_suffixes_large_values() {
    assert_eq!(expand_number_suffixes("100k"), "100 thousand");
    assert_eq!(expand_number_suffixes("1000K"), "1000 thousand");
    assert_eq!(expand_number_suffixes("999M"), "999 million");
    assert_eq!(expand_number_suffixes("100B"), "100 billion");
    assert_eq!(expand_number_suffixes("999tr"), "999 trillion");
}

#[test]
fn test_sanitize_tts_number_suffixes_decimals() {
    assert_eq!(expand_number_suffixes("1.5k"), "1.5 thousand");
    assert_eq!(expand_number_suffixes("2.3M"), "2.3 million");
    assert_eq!(expand_number_suffixes("3.75bn"), "3.75 billion");
    assert_eq!(expand_number_suffixes("4.5tr"), "4.5 trillion");
}

#[test]
fn test_sanitize_tts_number_suffixes_without_digits() {
    assert_eq!(expand_number_suffixes("k users"), "k users");
    assert_eq!(expand_number_suffixes("M downloads"), "M downloads");
    assert_eq!(expand_number_suffixes("bn revenue"), "bn revenue");
    assert_eq!(expand_number_suffixes("B valuation"), "B valuation");
    assert_eq!(expand_number_suffixes("tr market cap"), "tr market cap");
}

#[test]
fn test_sanitize_tts_number_suffixes_zero() {
    assert_eq!(expand_number_suffixes("0k"), "0 thousand");
    assert_eq!(expand_number_suffixes("0M"), "0 million");
    assert_eq!(expand_number_suffixes("0bn"), "0 billion");
    assert_eq!(expand_number_suffixes("0B"), "0 billion");
    assert_eq!(expand_number_suffixes("0tr"), "0 trillion");
}

#[test]
fn test_sanitize_tts_number_suffixes_case_sensitivity() {
    assert_eq!(expand_number_suffixes("5K"), "5 thousand");
    assert_eq!(expand_number_suffixes("5k"), "5 thousand");
    assert_eq!(expand_number_suffixes("5M"), "5 million");
    assert_eq!(expand_number_suffixes("5m"), "5 million");
    assert_eq!(expand_number_suffixes("5bn"), "5 billion");
    assert_eq!(expand_number_suffixes("5B"), "5 billion");
    assert_eq!(expand_number_suffixes("5TR"), "5 trillion");
    assert_eq!(expand_number_suffixes("5tr"), "5 trillion");
}

#[test]
fn test_sanitize_tts_number_suffixes_in_sentences() {
    assert_eq!(
        expand_number_suffixes("We have 10k users now"),
        "We have 10 thousand users now"
    );
    assert_eq!(
        expand_number_suffixes("Revenue hit $5bn this quarter"),
        "Revenue hit $5 billion this quarter"
    );
    assert_eq!(
        expand_number_suffixes("The company is worth 100B"),
        "The company is worth 100 billion"
    );
}

#[test]
fn test_sanitize_tts_number_suffixes_with_currency() {
    assert_eq!(expand_number_suffixes("$10k profit"), "$10 thousand profit");
    assert_eq!(expand_number_suffixes("€5M funding"), "€5 million funding");
    assert_eq!(expand_number_suffixes("£2bn deal"), "£2 billion deal");
}

#[test]
fn test_sanitize_tts_symbols_at() {
    assert_eq!(
        expand_symbols("email user@example.com here"),
        "email user at example.com here"
    );
    assert_eq!(expand_symbols("@username"), "at username");
}

#[test]
fn test_sanitize_tts_punctuation_ellipsis() {
    assert_eq!(normalize_punctuation("wait\u{2026}"), "wait...");
    assert_eq!(normalize_punctuation("wait. . ."), "wait...");
    assert_eq!(normalize_punctuation("wait..."), "wait...");
}

#[test]
fn test_sanitize_tts_punctuation_em_dash() {
    assert_eq!(
        normalize_punctuation("word\u{2014}another"),
        "word, another"
    );
}

#[test]
fn test_sanitize_tts_punctuation_parentheses() {
    assert_eq!(
        normalize_punctuation("text (aside) more"),
        "text , aside, more"
    );
}

#[test]
fn test_sanitize_tts_combined() {
    let input = "According to [1], the speed is ~100 km/h (e.g. on highways) & drivers should check https://traffic.info for updates etc.";
    let result = sanitize_tts(input);
    assert!(!result.contains("https://"));
    assert!(!result.contains("[1]"));
    assert!(result.contains("and"));
    assert!(result.contains("approximately"));
    assert!(result.contains("for example"));
    assert!(result.contains("et cetera"));
}
