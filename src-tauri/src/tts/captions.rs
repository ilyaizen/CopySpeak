//! Timings belong to the exact generated audio, in its native media clock.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordTiming {
    /// UTF-16 offsets into CaptionAlignment.text (the browser's string indices).
    pub text_start: usize,
    pub text_end: usize,
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionAlignment {
    pub text: String,
    pub words: Vec<WordTiming>,
}

impl CaptionAlignment {
    /// Remove `[...]` emote-tag text and its word entries. Engines interpret
    /// tags instead of speaking them, so tag characters must never be
    /// highlighted — including characters a native-timestamps API echoes
    /// back into its alignment stream. Word offsets are shifted left by the
    /// removed UTF-16 units and stay valid against the stripped text; words
    /// overlapping a tag are dropped with it.
    pub fn stripped_of_emote_tags(&self) -> CaptionAlignment {
        let spans = crate::emote::find_tag_spans(&self.text);
        self.stripped_of_spans(&spans)
    }

    /// Same, and additionally removes `<...>` style wraps: the xAI dialect's
    /// wrapping speech tags are interpreted too, and its API echoes them back
    /// into the alignment stream alongside the bracket tags.
    pub fn stripped_of_all_emote_tags(&self) -> CaptionAlignment {
        let mut spans = crate::emote::find_tag_spans(&self.text);
        spans.extend(crate::emote::find_angle_tag_spans(&self.text));
        spans.sort_by_key(|span| span.start_utf16);
        self.stripped_of_spans(&spans)
    }

    fn stripped_of_spans(&self, spans: &[crate::emote::TagSpan]) -> CaptionAlignment {
        if spans.is_empty() {
            return self.clone();
        }
        let removed_before = |offset: usize| -> usize {
            spans
                .iter()
                .filter(|span| span.end_utf16 <= offset)
                .map(|span| span.end_utf16 - span.start_utf16)
                .sum()
        };
        let inside_tag = |offset: usize| -> bool {
            spans
                .iter()
                .any(|span| span.start_utf16 <= offset && offset < span.end_utf16)
        };
        let mut text = String::with_capacity(self.text.len());
        let mut last_byte = 0usize;
        for span in spans {
            text.push_str(&self.text[last_byte..span.start_byte]);
            last_byte = span.end_byte;
        }
        text.push_str(&self.text[last_byte..]);
        let words = self
            .words
            .iter()
            .filter(|word| {
                !inside_tag(word.text_start) && word.text_end > 0 && !inside_tag(word.text_end - 1)
            })
            .map(|word| {
                let shift = removed_before(word.text_start);
                WordTiming {
                    text_start: word.text_start - shift,
                    text_end: word.text_end - shift,
                    start_ms: word.start_ms,
                    end_ms: word.end_ms,
                }
            })
            .collect();
        CaptionAlignment { text, words }
    }

    pub fn validate(&self) -> Result<(), String> {
        let len = self.text.encode_utf16().count();
        let boundaries: Vec<usize> = std::iter::once(0)
            .chain(self.text.chars().scan(0, |offset, ch| {
                *offset += ch.len_utf16();
                Some(*offset)
            }))
            .collect();
        let mut text_end = 0;
        let mut audio_end = 0.0;
        for word in &self.words {
            if word.text_start < text_end
                || word.text_end <= word.text_start
                || word.text_end > len
                || !word.start_ms.is_finite()
                || !word.end_ms.is_finite()
                || word.start_ms < audio_end
                || word.end_ms < word.start_ms
                || boundaries.binary_search(&word.text_start).is_err()
                || boundaries.binary_search(&word.text_end).is_err()
            {
                return Err("Invalid caption text offsets or audio boundaries".into());
            }
            text_end = word.text_end;
            audio_end = word.end_ms;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SpeechAudio {
    pub bytes: Vec<u8>,
    pub captions: Option<CaptionAlignment>,
}

impl From<Vec<u8>> for SpeechAudio {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            captions: None,
        }
    }
}

/// Decode fragments with the existing audio decoder so offsets include every
/// sample of silence (and Edge MP3 fragments can be joined without guessing).
pub fn concat_speech(fragments: Vec<(SpeechAudio, String)>) -> Result<SpeechAudio, String> {
    use rodio::Source;
    let mut pcm = Vec::new();
    let mut format: Option<super::stream::AudioFormatMeta> = None;
    let mut combined = CaptionAlignment {
        text: String::new(),
        words: Vec::new(),
    };
    for (speech, text) in fragments {
        // Providers ship streaming-style headers with placeholder sizes (xAI:
        // data = i32::MAX; Fish: 0xFFFFFF00) that rodio trusts and dies on
        // with "end of stream"; clamp them to the real byte count first.
        let mut bytes = speech.bytes;
        crate::audio::wav::clamp_chunk_sizes_to_file(&mut bytes);
        let source = rodio::Decoder::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let meta = super::stream::AudioFormatMeta {
            sample_rate: source.sample_rate(),
            channels: source.channels(),
            bits_per_sample: 16,
        };
        if let Some(previous) = &format {
            if previous.sample_rate != meta.sample_rate || previous.channels != meta.channels {
                return Err(
                    "Cannot concatenate speech with different sample rates or channel counts"
                        .into(),
                );
            }
        }
        let offset_ms =
            pcm.len() as f64 * 1000.0 / (meta.sample_rate as f64 * meta.channels as f64 * 2.0);
        let pcm_start = pcm.len();
        for sample in source.convert_samples::<i16>() {
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        let duration_ms = (pcm.len() - pcm_start) as f64 * 1000.0
            / (meta.sample_rate as f64 * meta.channels as f64 * 2.0);
        let captions = speech.captions.filter(|captions| {
            if let Err(error) = captions.validate() {
                log::warn!("Ignoring invalid fragment captions during concatenation: {error}");
                return false;
            }
            // Empty snapshots carry no usable timing. Keep the fragment's
            // source text so later captions retain their joined text offsets.
            if captions.words.is_empty() {
                return false;
            }
            if captions
                .words
                .last()
                .is_some_and(|word| word.end_ms > duration_ms)
            {
                log::warn!("Ignoring fragment captions extending beyond their audio");
                return false;
            }
            true
        });
        if !combined.text.is_empty() {
            combined.text.push(' ');
        }
        let text_offset = combined.text.encode_utf16().count();
        if let Some(mut captions) = captions {
            for word in &mut captions.words {
                word.text_start += text_offset;
                word.text_end += text_offset;
                word.start_ms += offset_ms;
                word.end_ms += offset_ms;
            }
            combined.text.push_str(&captions.text);
            combined.words.extend(captions.words);
        } else {
            combined.text.push_str(&text);
        }
        format = Some(meta);
    }
    let meta = format.ok_or("No speech fragments to concatenate")?;
    let captions = match combined.validate() {
        Ok(()) => (!combined.words.is_empty()).then_some(combined),
        Err(error) => {
            log::warn!("Ignoring invalid combined captions: {error}");
            None
        }
    };
    Ok(SpeechAudio {
        bytes: super::stream::pcm_to_wav(&pcm, &meta),
        captions,
    })
}

/// Estimated word timings for engines whose API returns none: place each word
/// proportionally inside the exactly measured duration of its audio (the
/// estimation recipe — per-request durations are exact, intra-request word
/// placement is approximate, and validation drops whatever does not fit).
/// Words are the same alphanumeric runs the browser bridge aligns with, so a
/// highlighted unit is identical on both sides. `[...]` emote tags are
/// excluded first: engines interpret them instead of speaking them, so a tag
/// word would take audio time no word occupies and shift every later
/// highlight.
pub fn estimate_alignment(text: &str, duration_ms: f64) -> CaptionAlignment {
    let stripped = crate::emote::strip_tags(text);
    let (spans, _) = crate::text_map::tokenize(&stripped);
    CaptionAlignment {
        text: stripped,
        words: proportional_words(&spans, duration_ms),
    }
}

/// Contiguous slices of `duration_ms`, each token weighted by its UTF-16
/// length. Empty for no tokens or a non-positive duration: still a valid,
/// untimed alignment.
fn proportional_words(spans: &[(usize, usize)], duration_ms: f64) -> Vec<WordTiming> {
    let total: usize = spans.iter().map(|(start, end)| end - start).sum();
    if total == 0 || !duration_ms.is_finite() || duration_ms <= 0.0 {
        return Vec::new();
    }
    let mut words = Vec::with_capacity(spans.len());
    let mut consumed = 0usize;
    for &(text_start, text_end) in spans {
        let start_ms = duration_ms * consumed as f64 / total as f64;
        consumed += text_end - text_start;
        let end_ms = duration_ms * consumed as f64 / total as f64;
        words.push(WordTiming {
            text_start,
            text_end,
            start_ms,
            end_ms,
        });
    }
    words
}

/// Sidecars travel with saved audio, including history and the CLI's one-shot output.
pub fn read_sidecar(path: &str) -> Option<CaptionAlignment> {
    let data = match std::fs::read(format!("{path}.captions.json")) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("Cannot read caption sidecar: {error}");
            return None;
        }
    };
    match serde_json::from_slice::<CaptionAlignment>(&data)
        .map_err(|e| e.to_string())
        .and_then(|captions| {
            captions.validate()?;
            Ok(captions)
        }) {
        Ok(captions) => Some(captions),
        Err(error) => {
            log::warn!("Ignoring invalid caption sidecar: {error}");
            None
        }
    }
}

pub fn write_sidecar(path: &str, captions: &CaptionAlignment) -> Result<(), String> {
    captions.validate()?;
    let data = serde_json::to_vec(captions).map_err(|e| e.to_string())?;
    std::fs::write(format!("{path}.captions.json"), data).map_err(|e| e.to_string())
}

pub fn remove_sidecar(path: &str) {
    if let Err(error) = std::fs::remove_file(format!("{path}.captions.json")) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Cannot remove caption sidecar: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_round_trip_preserves_native_gaps_and_rejects_invalid_metadata() {
        let path =
            std::env::temp_dir().join(format!("copyspeak-captions-test-{}", std::process::id()));
        let path = path.to_str().unwrap();
        let captions = CaptionAlignment {
            text: "😀 Hi there".into(),
            words: vec![
                WordTiming {
                    text_start: 3,
                    text_end: 5,
                    start_ms: 100.0,
                    end_ms: 250.0,
                },
                WordTiming {
                    text_start: 6,
                    text_end: 11,
                    start_ms: 700.0,
                    end_ms: 900.0,
                },
            ],
        };
        write_sidecar(path, &captions).unwrap();
        assert_eq!(read_sidecar(path), Some(captions.clone()));
        let mut invalid = captions;
        invalid.words[1].start_ms = 50.0;
        assert!(write_sidecar(path, &invalid).is_err());
        std::fs::write(format!("{path}.captions.json"), b"{}").unwrap();
        assert_eq!(read_sidecar(path), None);
        remove_sidecar(path);
        assert_eq!(read_sidecar(path), None);
    }

    #[test]
    fn rejected_fragment_captions_preserve_audio_and_neighbor_timings() {
        let meta = super::super::stream::AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        let make = |text: &str| SpeechAudio {
            bytes: super::super::stream::pcm_to_wav(&vec![0; 16000], &meta),
            captions: Some(CaptionAlignment {
                text: text.into(),
                words: vec![WordTiming {
                    text_start: 0,
                    text_end: text.encode_utf16().count(),
                    start_ms: 100.0,
                    end_ms: 300.0,
                }],
            }),
        };
        // Both invalid offsets and locally ordered timings beyond the audio
        // must degrade only this fragment, not invalidate the joined reading.
        for overrun in [false, true] {
            let mut middle = make("wrong");
            let word = &mut middle.captions.as_mut().unwrap().words[0];
            if overrun {
                word.end_ms = 2000.0;
            } else {
                word.text_end = 99;
            }
            let speech = concat_speech(vec![
                (make("A"), "A".into()),
                (middle, "😀 B".into()),
                (make("C"), "C".into()),
            ])
            .unwrap();
            let captions = speech.captions.unwrap();
            assert_eq!(captions.text, "A 😀 B C");
            assert_eq!(captions.words.len(), 2);
            assert_eq!(captions.words[0].start_ms, 100.0);
            assert_eq!(captions.words[0].end_ms, 300.0);
            assert_eq!(captions.words[1].text_start, 7);
            assert_eq!(captions.words[1].start_ms, 2100.0);
            assert_eq!(captions.words[1].end_ms, 2300.0);
            assert_eq!(
                speech.bytes,
                super::super::stream::pcm_to_wav(&vec![0; 48000], &meta)
            );
            assert!(captions.validate().is_ok());
        }
    }

    #[test]
    fn untimed_fragment_uses_source_text_between_timed_neighbors() {
        let meta = super::super::stream::AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        let timed = |text: &str| SpeechAudio {
            bytes: super::super::stream::pcm_to_wav(&vec![0; 16000], &meta),
            captions: Some(CaptionAlignment {
                text: text.into(),
                words: vec![WordTiming {
                    text_start: 0,
                    text_end: text.encode_utf16().count(),
                    start_ms: 100.0,
                    end_ms: 300.0,
                }],
            }),
        };
        for captions in [
            None,
            Some(CaptionAlignment {
                text: String::new(),
                words: Vec::new(),
            }),
        ] {
            let middle = SpeechAudio {
                bytes: super::super::stream::pcm_to_wav(&vec![0; 16000], &meta),
                captions,
            };
            let speech = concat_speech(vec![
                (timed("A"), "A".into()),
                (middle, "😀 B".into()),
                (timed("C"), "C".into()),
            ])
            .unwrap();
            let captions = speech.captions.unwrap();
            assert_eq!(captions.text, "A 😀 B C");
            assert_eq!(captions.words.len(), 2);
            assert_eq!(captions.words[1].text_start, 7);
            assert_eq!(captions.words[1].start_ms, 2100.0);
            assert_eq!(
                speech.bytes,
                super::super::stream::pcm_to_wav(&vec![0; 48000], &meta)
            );
        }
    }

    #[test]
    fn concatenation_still_rejects_undecodable_audio() {
        assert!(concat_speech(vec![(vec![1, 2, 3].into(), "Hi".into())]).is_err());
    }

    #[test]
    fn concat_clamps_streaming_placeholder_headers() {
        // xAI ships data = i32::MAX and a sign-bit-set RIFF size; rodio's
        // symphonia decoder trusts those fields and used to fail the whole
        // concatenation with "end of stream" (the 2026-10-26 Read-aloud
        // break). Placeholder headers must decode like honest ones.
        let meta = super::super::stream::AudioFormatMeta {
            sample_rate: 24000,
            channels: 1,
            bits_per_sample: 16,
        };
        let placeholder = |pcm: Vec<u8>| {
            let mut wav = super::super::stream::pcm_to_wav(&pcm, &meta);
            let riff: u32 = 0x8000_0000 + (wav.len() as u32 - 8);
            wav[4..8].copy_from_slice(&riff.to_le_bytes());
            wav[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
            wav
        };
        let speech = concat_speech(vec![
            (placeholder(vec![0; 16000]).into(), "A".into()),
            (placeholder(vec![0; 16000]).into(), "B".into()),
        ])
        .unwrap();
        let info = crate::audio::wav::parse_wav_header(&speech.bytes).unwrap();
        assert_eq!(info.data_size, 32000);
    }

    #[test]
    fn estimated_words_fill_the_measured_duration_contiguously() {
        let alignment = estimate_alignment("Hello, world!", 1000.0);
        assert_eq!(alignment.text, "Hello, world!");
        assert_eq!(alignment.words.len(), 2);
        assert_eq!(alignment.words[0].text_start, 0);
        assert_eq!(alignment.words[0].text_end, 5);
        assert_eq!(alignment.words[1].text_start, 7);
        assert_eq!(alignment.words[1].text_end, 12);
        assert_eq!(alignment.words[0].start_ms, 0.0);
        assert_eq!(alignment.words[0].end_ms, alignment.words[1].start_ms);
        assert_eq!(alignment.words[1].end_ms, 1000.0);
        alignment.validate().unwrap();
    }

    #[test]
    fn estimation_weights_words_by_length() {
        // UTF-16 token lengths 2, 5, 6 of 13 total over 1300 ms.
        let alignment = estimate_alignment("hi there friend", 1300.0);
        let ends: Vec<f64> = alignment.words.iter().map(|w| w.end_ms).collect();
        assert_eq!(ends, vec![200.0, 700.0, 1300.0]);
    }

    #[test]
    fn estimation_without_tokens_or_duration_yields_no_words() {
        for (text, duration) in [
            ("", 1000.0),
            ("... —!", 1000.0),
            ("words", 0.0),
            ("words", -5.0),
            ("words", f64::NAN),
        ] {
            let alignment = estimate_alignment(text, duration);
            assert!(alignment.words.is_empty(), "{text:?} {duration}");
            assert_eq!(alignment.text, text);
        }
    }

    #[test]
    fn joined_caption_offsets_include_trailing_silence_and_utf16_text() {
        let meta = super::super::stream::AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        let first = SpeechAudio {
            bytes: super::super::stream::pcm_to_wav(&vec![0; 20000], &meta),
            captions: Some(CaptionAlignment {
                text: "😀 Hi".into(),
                words: vec![WordTiming {
                    text_start: 3,
                    text_end: 5,
                    start_ms: 100.0,
                    end_ms: 300.0,
                }],
            }),
        };
        let second = SpeechAudio {
            bytes: super::super::stream::pcm_to_wav(&vec![0; 16000], &meta),
            captions: Some(CaptionAlignment {
                text: "there".into(),
                words: vec![WordTiming {
                    text_start: 0,
                    text_end: 5,
                    start_ms: 100.0,
                    end_ms: 500.0,
                }],
            }),
        };
        let combined =
            concat_speech(vec![(first, String::new()), (second, String::new())]).unwrap();
        let captions = combined.captions.unwrap();
        assert_eq!(captions.text, "😀 Hi there");
        assert_eq!(captions.words[1].text_start, 6);
        assert_eq!(captions.words[1].start_ms, 1350.0);
        assert_eq!(captions.words[1].end_ms, 1750.0);
        assert_eq!(
            crate::audio::wav::parse_wav_header(&combined.bytes)
                .unwrap()
                .data_size,
            36000
        );
    }
}

#[cfg(test)]
mod emote_tag_tests {
    use super::*;

    fn word_texts(captions: &CaptionAlignment) -> Vec<String> {
        let units: Vec<u16> = captions.text.encode_utf16().collect();
        captions
            .words
            .iter()
            .map(|word| String::from_utf16_lossy(&units[word.text_start..word.text_end]))
            .collect()
    }

    /// Estimated captions (Fish and every engine without native timings) must
    /// not give emote tags audio time: the engine never speaks them, so a tag
    /// word would shift every following highlight.
    #[test]
    fn estimated_captions_exclude_emote_tags() {
        let caps = estimate_alignment("[happy] Hello world [sighs] again", 1000.0);
        caps.validate().unwrap();
        assert_eq!(caps.text, "Hello world again", "{caps:?}");
        assert_eq!(word_texts(&caps), vec!["Hello", "world", "again"]);
    }

    #[test]
    fn plain_text_estimation_is_unchanged() {
        let caps = estimate_alignment("Hello world", 1000.0);
        caps.validate().unwrap();
        assert_eq!(caps.text, "Hello world");
        assert_eq!(caps.words.len(), 2);
        assert_eq!(word_texts(&caps), vec!["Hello", "world"]);
    }

    /// Tag-free captions pass through the strip untouched (the ElevenLabs
    /// emission path runs it on every snapshot).
    #[test]
    fn stripping_tag_free_captions_is_a_no_op() {
        let caps = CaptionAlignment {
            text: "Hello 😀 world".into(),
            words: vec![
                WordTiming {
                    text_start: 0,
                    text_end: 5,
                    start_ms: 0.0,
                    end_ms: 100.0,
                },
                WordTiming {
                    text_start: 10,
                    text_end: 15,
                    start_ms: 200.0,
                    end_ms: 300.0,
                },
            ],
        };
        let stripped = caps.stripped_of_emote_tags();
        assert_eq!(stripped, caps);
    }

    /// Native captions that echo tag characters lose them: tag text and its
    /// word entries are removed, later words shift left, offsets stay valid.
    #[test]
    fn stripping_removes_tag_words_and_shifts_offsets() {
        let caps = CaptionAlignment {
            text: "[sighs] Hello world".into(),
            words: vec![
                WordTiming {
                    text_start: 0,
                    text_end: 7,
                    start_ms: 0.0,
                    end_ms: 5.0,
                }, // "[sighs]"
                WordTiming {
                    text_start: 7,
                    text_end: 8,
                    start_ms: 5.0,
                    end_ms: 10.0,
                }, // " "
                WordTiming {
                    text_start: 8,
                    text_end: 13,
                    start_ms: 10.0,
                    end_ms: 300.0,
                }, // "Hello"
                WordTiming {
                    text_start: 13,
                    text_end: 14,
                    start_ms: 300.0,
                    end_ms: 310.0,
                }, // " "
                WordTiming {
                    text_start: 14,
                    text_end: 19,
                    start_ms: 310.0,
                    end_ms: 600.0,
                }, // "world"
            ],
        };
        let stripped = caps.stripped_of_emote_tags();
        stripped.validate().unwrap();
        assert_eq!(stripped.text, " Hello world");
        let words = word_texts(&stripped);
        assert_eq!(words, vec![" ", "Hello", " ", "world"]);
        assert_eq!(stripped.words[1].start_ms, 10.0);
        assert_eq!(stripped.words[1].text_start, 1);
        assert_eq!(stripped.words[3].text_start, 7);
        assert_eq!(stripped.words[3].text_end, 12);
    }
}
