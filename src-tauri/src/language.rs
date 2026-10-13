//! Language detection for per-utterance profile switching.
//!
//! Detection looks only at the utterance text. A result counts only when the
//! detector is reliable and the text is long enough to judge; anything else
//! returns `None` so the caller keeps the active profile.

use whatlang::Lang;

/// Shortest text, in characters, the detector is asked to judge. Shorter text
/// has too few trigrams for a reliable call.
pub const MIN_CHARS: usize = 10;

/// Detects the language of `text` as an ISO 639-1 code, or `None` when the
/// text is too short or the detector is not reliable.
pub fn detect_language(text: &str) -> Option<&'static str> {
    if text.chars().count() < MIN_CHARS {
        return None;
    }
    let info = whatlang::detect(text)?;
    if !info.is_reliable() {
        return None;
    }
    Some(iso_639_1(info.lang()))
}

/// ISO 639-1 code for a detector language. Exhaustive on purpose: a whatlang
/// upgrade that adds a language fails to compile until it gets a code here.
fn iso_639_1(lang: Lang) -> &'static str {
    match lang {
        Lang::Epo => "eo",
        Lang::Eng => "en",
        Lang::Rus => "ru",
        Lang::Cmn => "zh",
        Lang::Spa => "es",
        Lang::Por => "pt",
        Lang::Ita => "it",
        Lang::Ben => "bn",
        Lang::Fra => "fr",
        Lang::Deu => "de",
        Lang::Ukr => "uk",
        Lang::Kat => "ka",
        Lang::Ara => "ar",
        Lang::Hin => "hi",
        Lang::Jpn => "ja",
        Lang::Heb => "he",
        Lang::Yid => "yi",
        Lang::Pol => "pl",
        Lang::Amh => "am",
        Lang::Jav => "jv",
        Lang::Kor => "ko",
        Lang::Nob => "nb",
        Lang::Dan => "da",
        Lang::Swe => "sv",
        Lang::Fin => "fi",
        Lang::Tur => "tr",
        Lang::Nld => "nl",
        Lang::Hun => "hu",
        Lang::Ces => "cs",
        Lang::Ell => "el",
        Lang::Bul => "bg",
        Lang::Bel => "be",
        Lang::Mar => "mr",
        Lang::Kan => "kn",
        Lang::Ron => "ro",
        Lang::Slv => "sl",
        Lang::Hrv => "hr",
        Lang::Srp => "sr",
        Lang::Mkd => "mk",
        Lang::Lit => "lt",
        Lang::Lav => "lv",
        Lang::Est => "et",
        Lang::Tam => "ta",
        Lang::Vie => "vi",
        Lang::Urd => "ur",
        Lang::Tha => "th",
        Lang::Guj => "gu",
        Lang::Uzb => "uz",
        Lang::Pan => "pa",
        Lang::Aze => "az",
        Lang::Ind => "id",
        Lang::Tel => "te",
        Lang::Pes => "fa",
        Lang::Mal => "ml",
        Lang::Ori => "or",
        Lang::Mya => "my",
        Lang::Nep => "ne",
        Lang::Sin => "si",
        Lang::Khm => "km",
        Lang::Tuk => "tk",
        Lang::Aka => "ak",
        Lang::Zul => "zu",
        Lang::Sna => "sn",
        Lang::Afr => "af",
        Lang::Lat => "la",
        Lang::Slk => "sk",
        Lang::Cat => "ca",
        Lang::Tgl => "tl",
        Lang::Hye => "hy",
        Lang::Cym => "cy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// `iso_639_1<TAB>sentence` lines, two per language.
    const SAMPLES: &str = include_str!("../tests/fixtures/language_samples.tsv");

    #[test]
    fn short_text_is_never_judged() {
        // Nine characters: below the gate even though the detector would name English.
        assert_eq!(detect_language("thank you"), None);
        // Nine characters but 27 bytes: the gate counts characters, not bytes.
        assert_eq!(detect_language("日本語のテストです"), None);
    }

    #[test]
    fn common_languages_are_detected() {
        let cases = [
            // The pangram scores 0.74 (below the gate): a famous, over-quoted
            // sentence is refused, which is the intended outcome. Ordinary prose here.
            (
                "I think we should meet at the coffee shop around noon tomorrow to discuss the plan.",
                "en",
            ),
            (
                "Der schnelle braune Fuchs springt an einem warmen Sommermorgen über den faulen Hund am Flussufer.",
                "de",
            ),
            (
                "Le renard brun rapide saute par-dessus le chien paresseux au bord de la rivière par une chaude matinée d'été.",
                "fr",
            ),
            (
                "El rápido zorro marrón salta sobre el perro perezoso junto al río en una cálida mañana de verano.",
                "es",
            ),
            (
                "素早い茶色の狐が、暖かい夏の朝に川辺で怠け者の犬を飛び越えます。",
                "ja",
            ),
            (
                "Быстрая коричневая лиса прыгает через ленивую собаку у реки тёплым летним утром.",
                "ru",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(detect_language(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn fixture_samples_are_never_misdetected() {
        // Refusing is allowed; naming the wrong language is not.
        let mut judged = 0;
        let mut total = 0;
        for line in SAMPLES.lines().filter(|line| !line.trim().is_empty()) {
            let (expected, text) = line.split_once('\t').expect("tab-separated sample");
            total += 1;
            if let Some(code) = detect_language(text) {
                assert_eq!(code, expected, "misdetected sample: {text}");
                judged += 1;
            }
        }
        assert!(
            judged * 100 >= total * MIN_JUDGED_PERCENT,
            "only {judged}/{total} samples judged; floor is {MIN_JUDGED_PERCENT}%"
        );
    }

    /// Coverage floor for the fixture. Set just under what the fixture measures;
    /// raise it when the detector or the fixture improves.
    const MIN_JUDGED_PERCENT: usize = 60;

    #[test]
    fn every_detector_language_maps_to_a_distinct_two_letter_code() {
        let codes: HashSet<&str> = Lang::all().iter().map(|lang| iso_639_1(*lang)).collect();
        assert_eq!(codes.len(), Lang::all().len());
        assert!(codes.iter().all(|code| code.len() == 2));
    }
}
