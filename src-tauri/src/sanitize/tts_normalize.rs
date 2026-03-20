// Pass 2: TTS text normalization — expands abbreviations, symbols, and units for TTS readability.

use regex::Regex;

use super::cleanup::cleanup_artifacts;

/// Normalize text for TTS readability.
/// Applies replacements in priority order: emojis → URLs → citations → slashes →
/// Latin abbreviations → metric units → symbols → punctuation.
/// Newlines are stripped at the end — they don't affect speech and produce
/// cleaner single-line text for history preview.
pub fn sanitize_tts(text: &str) -> String {
    let mut result = text.to_string();

    // Order matters — run in the specified priority sequence
    result = remove_emojis(&result);
    result = remove_urls(&result);
    result = remove_citations(&result);
    result = expand_slash_lookups(&result);
    result = expand_slash_options(&result);
    result = expand_slash_ratios(&result);
    result = expand_latin_abbreviations(&result);
    result = expand_title_abbreviations(&result);
    result = expand_number_suffixes(&result); // Run before metric units (5m = 5 million, not 5 meters)
    result = expand_metric_units(&result);
    result = expand_symbols(&result);
    result = normalize_punctuation(&result);
    result = cleanup_artifacts(&result);

    // Strip newlines — they have no effect on speech and produce cleaner
    // single-line output for history preview.
    result = result.replace('\r', "").replace('\n', " ");
    result.trim().to_string()
}

// ── 0. Emoji Removal ─────────────────────────────────────────────────────────

fn remove_emojis(text: &str) -> String {
    lazy_static::lazy_static! {
        // Matches common emoji Unicode ranges:
        // 1F300–1F9FF: misc symbols, pictographs, emoticons, transport, etc.
        // 1FA00–1FAFF: newer emoji additions
        // 2600–27BF:   miscellaneous symbols and dingbats
        // FE00–FE0F:   variation selectors (emoji vs text presentation)
        // 200D:        zero-width joiner (used in multi-part emoji sequences)
        // 20E3:        combining enclosing keycap (e.g. 1️⃣)
        // 1F1E0–1F1FF: regional indicator symbols (flag pairs)
        static ref EMOJI_REGEX: Regex = Regex::new(
            r"[\u{1F300}-\u{1F9FF}\u{1FA00}-\u{1FAFF}\u{2600}-\u{27BF}\u{FE00}-\u{FE0F}\u{200D}\u{20E3}\u{1F1E0}-\u{1F1FF}]+"
        ).unwrap();
    }
    EMOJI_REGEX.replace_all(text, "").to_string()
}

// ── 1. Web Artifacts ────────────────────────────────────────────────────────

fn remove_urls(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref URL_REGEX: Regex = Regex::new(r"https?://[^\s)\]]+").unwrap();
    }
    URL_REGEX.replace_all(text, "").to_string()
}

// ── 2. Citations ────────────────────────────────────────────────────────────

fn remove_citations(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref CITATION_REGEX: Regex = Regex::new(r"\[[a-zA-Z0-9]+\]").unwrap();
    }
    CITATION_REGEX.replace_all(text, "").to_string()
}

// ── 3. Slash Lookups (Priority 1 — specific abbreviations) ──────────────────

fn expand_slash_lookups(text: &str) -> String {
    lazy_static::lazy_static! {
        // Order matters: match longer patterns first
        static ref WO_REGEX: Regex = Regex::new(r"(?i)\bw/o\b").unwrap();
        static ref NA_REGEX: Regex = Regex::new(r"(?i)\bn/a\b").unwrap();
        static ref AND_OR_REGEX: Regex = Regex::new(r"(?i)\band/or\b").unwrap();
        // w/ must come last — after w/o is already handled
        static ref W_REGEX: Regex = Regex::new(r"(?i)\bw/\s*").unwrap();
    }

    let result = WO_REGEX.replace_all(text, "without").to_string();
    let result = NA_REGEX.replace_all(&result, "not applicable").to_string();
    let result = AND_OR_REGEX.replace_all(&result, "and or").to_string();
    W_REGEX.replace_all(&result, "with ").to_string()
}

// ── 4. Slash Options (Priority 2 — wordA/wordB → wordA or wordB) ───────────

fn expand_slash_options(text: &str) -> String {
    lazy_static::lazy_static! {
        // Match word/word patterns (alphabetic words separated by /)
        static ref SLASH_OPTION_REGEX: Regex = Regex::new(r"\b([a-zA-Z]+)/([a-zA-Z]+)\b").unwrap();
    }
    SLASH_OPTION_REGEX.replace_all(text, "$1 or $2").to_string()
}

// ── 5. Slash Ratios (Priority 3 — unit/unit → unit per unit) ────────────────

fn expand_slash_ratios(text: &str) -> String {
    lazy_static::lazy_static! {
        // Match patterns like km/h, m/s, miles/hour — unit/unit after a digit
        static ref SLASH_RATIO_REGEX: Regex = Regex::new(r"(\d\s*)([a-zA-Z]+)/([a-zA-Z]+)").unwrap();
    }
    SLASH_RATIO_REGEX
        .replace_all(text, "$1$2 per $3")
        .to_string()
}

// ── 6. Latin Abbreviations ──────────────────────────────────────────────────

fn expand_latin_abbreviations(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref EG_REGEX: Regex = Regex::new(r"(?i)\be\.g\.\s*").unwrap();
        static ref IE_REGEX: Regex = Regex::new(r"(?i)\bi\.e\.\s*").unwrap();
        static ref ETC_REGEX: Regex = Regex::new(r"(?i)\betc\.\s*").unwrap();
        static ref VS_REGEX: Regex = Regex::new(r"(?i)\bvs\.\s*").unwrap();
    }

    let result = EG_REGEX.replace_all(text, "for example ").to_string();
    let result = IE_REGEX.replace_all(&result, "that is ").to_string();
    let result = ETC_REGEX.replace_all(&result, "et cetera ").to_string();
    VS_REGEX.replace_all(&result, "versus ").to_string()
}

// ── 7. Title Abbreviations ────────────────────────────────────────────────────

fn expand_title_abbreviations(text: &str) -> String {
    lazy_static::lazy_static! {
        // Honorific and academic titles
        static ref DR_REGEX: Regex = Regex::new(r"(?i)\bDr\.\s*").unwrap();
        static ref MR_REGEX: Regex = Regex::new(r"(?i)\bMr\.\s*").unwrap();
        static ref MRS_REGEX: Regex = Regex::new(r"(?i)\bMrs\.\s*").unwrap();
        static ref MS_REGEX: Regex = Regex::new(r"(?i)\bMs\.\s*").unwrap();
        static ref PROF_REGEX: Regex = Regex::new(r"(?i)\bProf\.\s*").unwrap();
        static ref REV_REGEX: Regex = Regex::new(r"(?i)\bRev\.\s*").unwrap();
        static ref SR_REGEX: Regex = Regex::new(r"(?i)\bSr\.\s*").unwrap();
        static ref JR_REGEX: Regex = Regex::new(r"(?i)\bJr\.\s*").unwrap();
        static ref HON_REGEX: Regex = Regex::new(r"(?i)\bHon\.\s*").unwrap();
        static ref SIR_REGEX: Regex = Regex::new(r"(?i)\bSir\s+").unwrap();
        static ref MADM_REGEX: Regex = Regex::new(r"(?i)\bMadam\s+").unwrap();
    }

    let result = DR_REGEX.replace_all(text, "Doctor ").to_string();
    let result = MR_REGEX.replace_all(&result, "Mister ").to_string();
    let result = MRS_REGEX.replace_all(&result, "Misses ").to_string();
    let result = MS_REGEX.replace_all(&result, "Miss ").to_string();
    let result = PROF_REGEX.replace_all(&result, "Professor ").to_string();
    let result = REV_REGEX.replace_all(&result, "Reverend ").to_string();
    let result = SR_REGEX.replace_all(&result, "Senior ").to_string();
    let result = JR_REGEX.replace_all(&result, "Junior ").to_string();
    let result = HON_REGEX.replace_all(&result, "Honorable ").to_string();
    let result = SIR_REGEX.replace_all(&result, "Sir ").to_string();
    MADM_REGEX.replace_all(&result, "Madam ").to_string()
}

// ── 8. Number Suffixes (Magnitude) ──────────────────────────────────────────

fn expand_number_suffixes(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref BN_REGEX: Regex = Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*bn\b").unwrap();
        static ref BILLION_B_REGEX: Regex = Regex::new(r"(\d+(?:\.\d+)?)\s*B\b").unwrap();
        static ref MILLION_REGEX: Regex = Regex::new(r"(\d+(?:\.\d+)?)\s*m\b").unwrap();
        static ref MILLION_M_REGEX: Regex = Regex::new(r"(\d+(?:\.\d+)?)\s*M\b").unwrap();
        static ref K_REGEX: Regex = Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*k\b").unwrap();
        static ref TR_REGEX: Regex = Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*tr\b").unwrap();
    }

    // Order matters: longer patterns first
    let result = BN_REGEX.replace_all(text, "$1 billion").to_string();
    let result = BILLION_B_REGEX
        .replace_all(&result, "$1 billion")
        .to_string();
    let result = TR_REGEX.replace_all(&result, "$1 trillion").to_string();
    let result = MILLION_REGEX.replace_all(&result, "$1 million").to_string();
    let result = MILLION_M_REGEX
        .replace_all(&result, "$1 million")
        .to_string();
    K_REGEX.replace_all(&result, "$1 thousand").to_string()
}

// ── 9. Metric Units ────────────────────────────────────────────────────────

fn expand_metric_units(text: &str) -> String {
    lazy_static::lazy_static! {
        // Match digit(s) followed by unit abbreviation, with optional space
        static ref MM_REGEX: Regex = Regex::new(r"(\d)\s*mm\b").unwrap();
        static ref CM_REGEX: Regex = Regex::new(r"(\d)\s*cm\b").unwrap();
        static ref KM_REGEX: Regex = Regex::new(r"(\d)\s*km\b").unwrap();
        static ref KG_REGEX: Regex = Regex::new(r"(\d)\s*kg\b").unwrap();
        static ref G_REGEX: Regex = Regex::new(r"(\d)\s*g\b").unwrap();
        // Note: 'm' for meters is now handled more carefully since expand_number_suffixes
        // runs first and handles 'm' for million. We only expand to meters if the text
        // still contains a standalone 'm' after number suffix expansion (rare case).
        // Using word boundary and requiring the number to be directly adjacent.
        static ref M_REGEX: Regex = Regex::new(r"(\d)\s*m\b").unwrap();
    }

    // Order: longer unit abbreviations first to avoid partial matches
    let result = MM_REGEX.replace_all(text, "$1 millimeters").to_string();
    let result = CM_REGEX.replace_all(&result, "$1 centimeters").to_string();
    let result = KM_REGEX.replace_all(&result, "$1 kilometers").to_string();
    let result = KG_REGEX.replace_all(&result, "$1 kilograms").to_string();
    let result = G_REGEX.replace_all(&result, "$1 grams").to_string();
    M_REGEX.replace_all(&result, "$1 meters").to_string()
}

// ── 10. Symbols ──────────────────────────────────────────────────────────────

fn expand_symbols(text: &str) -> String {
    lazy_static::lazy_static! {
        // @ only inside email addresses or @handles
        static ref AT_EMAIL_REGEX: Regex = Regex::new(r"([a-zA-Z0-9._%+-])@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})").unwrap();
        static ref AT_HANDLE_REGEX: Regex = Regex::new(r"@([a-zA-Z0-9_]+)").unwrap();
        // Currency symbols: $50 → 50 dollars, €20 → 20 euros, £15 → 15 pounds, ¥1000 → 1000 yen
        static ref DOLLAR_REGEX: Regex = Regex::new(r"\$(\d+(?:\.\d+)?)\b").unwrap();
        static ref EURO_REGEX: Regex = Regex::new(r"€(\d+(?:\.\d+)?)\b").unwrap();
        static ref POUND_REGEX: Regex = Regex::new(r"£(\d+(?:\.\d+)?)\b").unwrap();
        static ref YEN_REGEX: Regex = Regex::new(r"¥(\d+(?:\.\d+)?)\b").unwrap();
    }

    let mut result = text.to_string();

    // Handle @ in emails and handles first (before generic symbol replacement)
    result = AT_EMAIL_REGEX.replace_all(&result, "$1 at $2").to_string();
    result = AT_HANDLE_REGEX.replace_all(&result, "at $1").to_string();

    // Handle currency symbols before other symbol replacements
    result = DOLLAR_REGEX.replace_all(&result, "$1 dollars").to_string();
    result = EURO_REGEX.replace_all(&result, "$1 euros").to_string();
    result = POUND_REGEX.replace_all(&result, "$1 pounds").to_string();
    result = YEN_REGEX.replace_all(&result, "$1 yen").to_string();

    // Simple character replacements
    result = result.replace('&', " and ");
    result = result.replace('%', " percent");
    result = result.replace('~', "approximately ");
    result = result.replace('+', " plus ");
    result = result.replace('=', " equals ");
    result = result.replace('°', " degrees");

    result
}

// ── 11. Punctuation Normalization ───────────────────────────────────────────

fn normalize_punctuation(text: &str) -> String {
    lazy_static::lazy_static! {
        // Normalize various ellipsis forms to standard "..."
        static ref ELLIPSIS_UNICODE_REGEX: Regex = Regex::new(r"\u{2026}").unwrap();
        static ref ELLIPSIS_SPACED_REGEX: Regex = Regex::new(r"\.\s*\.\s*\.").unwrap();
        // Em-dash → comma
        static ref EM_DASH_REGEX: Regex = Regex::new(r"\u{2014}").unwrap();
        // Parenthesized text → comma-delimited
        static ref PAREN_REGEX: Regex = Regex::new(r"\(([^)]+)\)").unwrap();
    }

    let result = ELLIPSIS_UNICODE_REGEX.replace_all(text, "...").to_string();
    let result = ELLIPSIS_SPACED_REGEX
        .replace_all(&result, "...")
        .to_string();
    let result = EM_DASH_REGEX.replace_all(&result, ", ").to_string();
    PAREN_REGEX.replace_all(&result, ", $1,").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_emojis() {
        assert_eq!(remove_emojis("Hello 😀 world"), "Hello  world");
        assert_eq!(remove_emojis("🎉🎊 Party time"), " Party time");
        assert_eq!(remove_emojis("No emojis here"), "No emojis here");
        assert_eq!(remove_emojis("🌍 Earth 🌏"), " Earth ");
        assert_eq!(remove_emojis("Text ✅ check"), "Text  check");
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
}
