// Pass 2: TTS text normalization — expands abbreviations, symbols, and units for TTS readability.

use regex::Regex;

use super::cleanup::cleanup_artifacts;

/// Normalize text for TTS readability.
/// Applies replacements in priority order: character normalization →
/// unpronounceable glyphs → URLs → citations → slashes → Latin abbreviations →
/// metric units → symbols → punctuation → leading-orphan strip.
/// Newlines are stripped at the end — they don't affect speech and produce
/// cleaner single-line text for history preview.
pub fn sanitize_tts(text: &str) -> String {
    let mut result = text.to_string();

    // Order matters — run in the specified priority sequence
    result = normalize_characters(&result);
    result = remove_unpronounceable(&result);
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
    // Last: drop orphan leading punctuation (", They didn't know…" →
    // "They didn't know…"). Runs after quote/dash normalization so opening
    // quotes survive while leading commas/bullets/dashes are already
    // canonical and stripped.
    result = strip_leading_orphans(&result);

    // Strip newlines — they have no effect on speech and produce cleaner
    // single-line output for history preview.
    result = result.replace('\r', "").replace('\n', " ");
    result.trim().to_string()
}

// ── 0. Unpronounceable Glyph Removal ────────────────────────────────────────

/// Strip glyphs neural TTS engines render as beep/noise artifacts (~1 s
/// bursts) instead of skipping: symbolic scripts beyond the classic emoji
/// ranges, plus control and invisible formatting characters.
fn remove_unpronounceable(text: &str) -> String {
    lazy_static::lazy_static! {
        // Symbol ranges engines confound into sound effects:
        // 00AD:        soft hyphen (invisible, mid-word)
        // 203C 2049:   ‼ ⁉ (emoji codepoints inside general punctuation)
        // 2139:        ℹ (emoji codepoint inside letterlike symbols)
        // 2190–2BFF:   arrows, math operators, technical symbols, enclosed
        //              alphanumerics, box/geometric shapes, stars, dingbats
        // E000–F8FF:   private use area
        // 1F000–1F2FF: mahjong, dominoes, playing cards, enclosed pictographs
        // 1F300–1F9FF: misc symbols, pictographs, emoticons, transport, etc.
        // 1FA00–1FAFF: newer emoji additions
        // 1F1E6–1F1FF: regional indicator symbols (flag pairs)
        // FE00–FE0F:   variation selectors (emoji vs text presentation)
        // E0100–E01EF: variation selector supplement
        // FFFD–FFFF:   replacement character and noncharacters
        static ref SYMBOL_REGEX: Regex = Regex::new(
            r"[\u{00AD}\u{203C}\u{2049}\u{2139}\u{2190}-\u{2BFF}\u{E000}-\u{F8FF}\u{1F000}-\u{1F2FF}\u{1F300}-\u{1F9FF}\u{1FA00}-\u{1FAFF}\u{1F1E6}-\u{1F1FF}\u{FE00}-\u{FE0F}\u{E0100}-\u{E01EF}\u{FFFD}-\u{FFFF}]+"
        ).unwrap();
        // Control and invisible formatting characters - never speech content:
        // 0000–0008 000E–001F: C0 controls except tab/newline/VT/FF
        // 007F–009F:   DEL and C1 controls
        // 200B 200D–200F: zero-width space/joiner, LRM/RLM (ZWNJ 200C is kept:
        //              Persian and Indic spelling need it)
        // 2028–202E:   line/paragraph separators, bidi overrides
        // 2060–2064:   word joiner and invisible operators
        // 20D0–20FF:   combining marks for symbols (keycap, etc.)
        // FEFF:        BOM / zero-width no-break space
        static ref INVISIBLE_REGEX: Regex = Regex::new(
            r"[\u{0000}-\u{0008}\u{000E}-\u{001F}\u{007F}-\u{009F}\u{200B}\u{200D}-\u{200F}\u{2028}-\u{202E}\u{2060}-\u{2064}\u{20D0}-\u{20FF}\u{FEFF}]+"
        ).unwrap();
    }
    // Tab/VT/FF separate words, so normalize them to spaces instead of
    // stripping (stripping would fuse adjacent words). Newlines stay for the
    // pipeline's end-of-pass handling.
    let spaced = text.replace(['\t', '\u{000B}', '\u{000C}'], " ");
    let without_invisible = INVISIBLE_REGEX.replace_all(&spaced, "");
    SYMBOL_REGEX
        .replace_all(&without_invisible, "")
        .into_owned()
}

// ── 0. Character Normalization (sanitext-style) ──────────────────────────────

/// True when two UTF-16 units are the same punctuation under normalization:
/// curly ↔ straight quotes, en/em dash ↔ hyphen. All pairs are 1 UTF-16 unit
/// on both sides, so substitution never shifts offsets. Single source of
/// truth, shared with `browser_bridge` (caption matching).
pub(crate) fn punct_equivalent(a: u16, b: u16) -> bool {
    const CURLY_OPEN: u16 = 0x201C; // “
    const CURLY_CLOSE: u16 = 0x201D; // ”
    const CURLY_SINGLE_OPEN: u16 = 0x2018; // ‘
    const CURLY_SINGLE_CLOSE: u16 = 0x2019; // ’
    const EN_DASH: u16 = 0x2013;
    const EM_DASH: u16 = 0x2014;
    const QUOTE: u16 = 0x0022; // "
    const APOSTROPHE: u16 = 0x0027; // '
    const HYPHEN: u16 = 0x002D; // -
    let class = |u: u16| match u {
        CURLY_OPEN | CURLY_CLOSE | QUOTE => 0,
        CURLY_SINGLE_OPEN | CURLY_SINGLE_CLOSE | APOSTROPHE => 1,
        EN_DASH | EM_DASH | HYPHEN => 2,
        _ => u, // anything else must match exactly
    };
    class(a) == class(b)
}

/// Same-length (1 UTF-16 unit → 1 unit) character substitutions: unicode
/// spaces, curly → straight quotes, en dash → hyphen, fullwidth → ASCII.
fn substitute_same_length(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref UNICODE_SPACES: Regex =
            Regex::new(r"[\u{00A0}\u{2000}-\u{200A}\u{202F}\u{205F}\u{3000}]").unwrap();
        static ref CURLY_DOUBLE: Regex = Regex::new(r"[\u{201C}\u{201D}]").unwrap();
        static ref CURLY_SINGLE: Regex = Regex::new(r"[\u{2018}\u{2019}\u{02BC}]").unwrap();
        static ref EN_DASH: Regex = Regex::new(r"\u{2013}").unwrap();
        static ref FULLWIDTH: Regex = Regex::new(r"[\u{FF01}-\u{FF5E}]").unwrap();
    }
    let result = UNICODE_SPACES.replace_all(text, " ").to_string();
    let result = CURLY_DOUBLE.replace_all(&result, "\"").to_string();
    let result = CURLY_SINGLE.replace_all(&result, "'").to_string();
    let result = EN_DASH.replace_all(&result, "-").to_string();
    // Fullwidth forms (！＂＃… from CJK contexts) fold to ASCII; the block is
    // a fixed 0xFEE0 offset for 0xFF01..=0xFF5E → 0x0021..=0x007E.
    FULLWIDTH
        .replace_all(&result, |caps: &regex::Captures| {
            let c = caps[0].chars().next().unwrap() as u32;
            char::from_u32(c - 0xFEE0).unwrap().to_string()
        })
        .to_string()
}

/// Homoglyph folding — Cyrillic confusables (a classic LLM-output artifact
/// that trips TTS pronunciation) with a visual Latin equivalent fold 1 unit
/// → 1 unit, but only inside a word that also holds a Latin letter. Real
/// Cyrillic words survive untouched. A word is a maximal alphanumeric run,
/// the same unit `text_map::tokenize` compares, so raw and spoken keys agree.
fn fold_homoglyphs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            fold_word(&mut out, &word);
            word.clear();
            out.push(c);
        }
    }
    fold_word(&mut out, &word);
    out
}

/// Appends `word`, folding its Cyrillic confusables when it mixes in Latin letters.
fn fold_word(out: &mut String, word: &str) {
    const MAP: &[(char, char)] = &[
        ('Ѐ', 'E'),
        ('Ё', 'E'),
        ('А', 'A'),
        ('В', 'B'),
        ('Е', 'E'),
        ('К', 'K'),
        ('М', 'M'),
        ('Н', 'H'),
        ('О', 'O'),
        ('Р', 'P'),
        ('С', 'C'),
        ('Т', 'T'),
        ('Х', 'X'),
        ('а', 'a'),
        ('е', 'e'),
        ('о', 'o'),
        ('р', 'p'),
        ('с', 'c'),
        ('у', 'y'),
        ('х', 'x'),
        ('ё', 'e'),
    ];
    if !word.chars().any(|c| c.is_ascii_alphabetic()) {
        out.push_str(word);
        return;
    }
    out.extend(
        word.chars()
            .map(|c| match MAP.iter().find(|(from, _)| *from == c) {
                Some((_, to)) => *to,
                None => c,
            }),
    );
}

/// Removes zero-width / invisible characters and BOM/soft-hyphen marks, but
/// keeps the zero-width non-joiner (U+200C), which Persian and Indic spelling need.
/// These are DELETIONS — they disqualify exact mapping by themselves.
fn strip_invisibles(text: &str) -> String {
    lazy_static::lazy_static! {
        static ref INVISIBLE: Regex = Regex::new(
            r"[\u{200B}\u{200D}-\u{200F}\u{2060}-\u{2064}\u{FEFF}\u{00AD}\u{180E}]"
        )
        .unwrap();
    }
    INVISIBLE.replace_all(text, "").to_string()
}

/// Compose the sanitization-side normalization passes (substitutions +
/// homoglyph folds + invisible deletions).
fn normalize_characters(text: &str) -> String {
    strip_invisibles(&fold_homoglyphs(&substitute_same_length(text)))
}

/// The same-length subset only (substitutions + homoglyph folds, NO
/// deletions). Used by `text_map` to compare raw and spoken words under the
/// folds the sanitizer applies, so a folded word still matches its source.
pub(crate) fn canonical_same_length(text: &str) -> String {
    fold_homoglyphs(&substitute_same_length(text))
}

// ── 0b. Leading Orphan Punctuation Strip ────────────────────────────────────

/// Strip orphan leading punctuation before the first word — clipboard
/// selections frequently pick up a comma/colon/bullet from the preceding
/// content (e.g. `, They didn't know…`). Opening quotes are NOT orphans
/// (real quotations start with them) and survive. Runs last, after quote
/// normalization, so a leading em dash has already become ", ".
fn strip_leading_orphans(text: &str) -> String {
    let is_orphan = |c: char| {
        matches!(
            c,
            ',' | ';' | ':' | '-' | '*' | '>' | '•' | '·' | '‣' | '∙' | '◦'
        )
    };
    let stripped_len = text
        .char_indices()
        .take_while(|&(_, c)| is_orphan(c) || c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    text[stripped_len..].trim_start().to_string()
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

lazy_static::lazy_static! {
    static ref PAREN_REGEX: Regex = Regex::new(r"\(([^)]+)\)").unwrap();
}

fn normalize_punctuation(text: &str) -> String {
    lazy_static::lazy_static! {
        // Normalize various ellipsis forms to standard "..."
        static ref ELLIPSIS_UNICODE_REGEX: Regex = Regex::new(r"\u{2026}").unwrap();
        static ref ELLIPSIS_SPACED_REGEX: Regex = Regex::new(r"\.\s*\.\s*\.").unwrap();
        // Em-dash → comma
        static ref EM_DASH_REGEX: Regex = Regex::new(r"\u{2014}").unwrap();
    }

    let result = ELLIPSIS_UNICODE_REGEX.replace_all(text, "...").to_string();
    let result = ELLIPSIS_SPACED_REGEX
        .replace_all(&result, "...")
        .to_string();
    let result = EM_DASH_REGEX.replace_all(&result, ", ").to_string();
    PAREN_REGEX.replace_all(&result, ", $1,").to_string()
}

#[cfg(test)]
#[path = "tts_normalize_tests.rs"]
mod tests;
