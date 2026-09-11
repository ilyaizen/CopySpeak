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
        let source =
            rodio::Decoder::new(std::io::Cursor::new(speech.bytes)).map_err(|e| e.to_string())?;
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
