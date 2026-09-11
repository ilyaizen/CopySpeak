//! NDJSON records may be split anywhere by HTTP, including inside UTF-8/base64.
use super::captions::{CaptionAlignment, WordTiming};
use super::stream::ChunkItem;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;

#[derive(Deserialize)]
struct CharacterAlignment {
    characters: Vec<String>,
    character_start_times_seconds: Vec<f64>,
    character_end_times_seconds: Vec<f64>,
}

#[derive(Deserialize)]
struct Record {
    audio_base64: String,
    // Deserialize optional metadata separately so its schema cannot reject audio.
    normalized_alignment: Option<serde_json::Value>,
    alignment: Option<serde_json::Value>,
}

pub(super) struct TimestampDecoder {
    pending: Vec<u8>,
    captions: CaptionAlignment,
    alignment_active: bool,
}

impl TimestampDecoder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            captions: CaptionAlignment {
                text: String::new(),
                words: Vec::new(),
            },
            alignment_active: true,
        }
    }

    pub fn push(&mut self, bytes: &[u8], final_chunk: bool) -> Result<Vec<ChunkItem>, String> {
        self.pending.extend_from_slice(bytes);
        // Prevent an invalid/malicious response from buffering without bound.
        if self.pending.len() > 16 * 1024 * 1024 {
            return Err("Timestamp record exceeds 16 MiB".into());
        }
        let mut items = Vec::new();
        while let Some(end) = self
            .pending
            .iter()
            .position(|&b| b == b'\n')
            .or_else(|| (final_chunk && !self.pending.is_empty()).then_some(self.pending.len()))
        {
            let line: Vec<u8> = self.pending.drain(..end).collect();
            if self.pending.first() == Some(&b'\n') {
                self.pending.remove(0);
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let record: Record = serde_json::from_slice(&line)
                .map_err(|e| format!("Invalid ElevenLabs timestamp record: {e}"))?;
            let pcm = STANDARD
                .decode(&record.audio_base64)
                .map_err(|e| format!("Invalid ElevenLabs audio: {e}"))?;
            if self.alignment_active {
                let result = match record.normalized_alignment.or(record.alignment) {
                    Some(value) => serde_json::from_value::<CharacterAlignment>(value)
                        .map_err(|e| format!("Invalid ElevenLabs alignment schema: {e}"))
                        .and_then(|alignment| {
                            if !pcm.is_empty() && alignment.characters.iter().all(String::is_empty)
                            {
                                return Err("ElevenLabs audio has empty alignment".into());
                            }
                            self.extend_captions(alignment)
                        })
                        .map(|()| true),
                    None if pcm.is_empty() => Ok(false),
                    None => Err("ElevenLabs audio has no alignment".into()),
                };
                match result {
                    Ok(true) => items.push(ChunkItem::Captions(self.captions.clone())),
                    Ok(false) => {}
                    Err(error) => {
                        log::warn!("Disabling ElevenLabs captions for this fragment: {error}");
                        // Later records are deltas. After a gap, their source
                        // offsets cannot be recovered by appending to the prefix.
                        self.alignment_active = false;
                        self.captions.text.clear();
                        self.captions.words.clear();
                        items.push(ChunkItem::ClearCaptions);
                    }
                }
            }
            // Metadata is delivered before its audio can become audible.
            if !pcm.is_empty() {
                items.push(ChunkItem::Pcm(pcm));
            }
        }
        Ok(items)
    }

    fn extend_captions(&mut self, alignment: CharacterAlignment) -> Result<(), String> {
        if alignment.characters.len() != alignment.character_start_times_seconds.len()
            || alignment.characters.len() != alignment.character_end_times_seconds.len()
        {
            return Err("ElevenLabs alignment arrays have different lengths".into());
        }
        // Commit only after validation; never mutate previously emitted intervals
        // with part of a malformed cumulative update.
        let mut captions = self.captions.clone();
        let mut offset = captions.text.encode_utf16().count();
        for ((character, start), end) in alignment
            .characters
            .into_iter()
            .zip(alignment.character_start_times_seconds)
            .zip(alignment.character_end_times_seconds)
        {
            let next = offset + character.encode_utf16().count();
            if next == offset {
                continue;
            }
            captions.text.push_str(&character);
            captions.words.push(WordTiming {
                text_start: offset,
                text_end: next,
                start_ms: start * 1000.0,
                end_ms: end * 1000.0,
            });
            offset = next;
        }
        captions.validate()?;
        self.captions = captions;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_transport_splits_preserve_native_gaps_and_unicode() {
        let record = serde_json::json!({"audio_base64": "AQIDBA==", "normalized_alignment": {
            "characters": ["😀", "H", "i"], "character_start_times_seconds": [0.0, 0.3, 0.6], "character_end_times_seconds": [0.0, 0.4, 0.8]
        }}).to_string();
        for split in 0..record.len() {
            let mut decoder = TimestampDecoder::new();
            assert!(decoder
                .push(&record.as_bytes()[..split], false)
                .unwrap()
                .is_empty());
            let items = decoder.push(&record.as_bytes()[split..], true).unwrap();
            let ChunkItem::Captions(captions) = &items[0] else {
                panic!("captions must precede PCM")
            };
            assert_eq!(captions.words[1].text_start, 2);
            assert_eq!(captions.words[2].start_ms, 600.0);
            assert_eq!(items[1], ChunkItem::Pcm(vec![1, 2, 3, 4]));
        }
    }
    #[test]
    fn multiple_records_keep_request_relative_times() {
        let mut decoder = TimestampDecoder::new();
        for (character, start) in [("A", 0.1), ("B", 1.2)] {
            let record = serde_json::json!({"audio_base64": "AAA=", "alignment": {
                "characters": [character], "character_start_times_seconds": [start],
                "character_end_times_seconds": [start + 0.1]
            }})
            .to_string()
                + "\n";
            let items = decoder.push(record.as_bytes(), false).unwrap();
            let ChunkItem::Captions(captions) = &items[0] else {
                panic!("missing captions")
            };
            assert_eq!(captions.words.last().unwrap().start_ms, start * 1000.0);
        }
        assert_eq!(decoder.captions.text, "AB");
        assert_eq!(decoder.captions.words[1].text_start, 1);
    }

    #[test]
    fn rejects_malformed_audio_and_record_framing() {
        for input in [
            r#"{"audio_base64":"!"}"#,
            r#"{"audio_base64":123, "alignment":{}}"#,
            r#"{"alignment":{}}"#,
            "not json",
        ] {
            assert!(TimestampDecoder::new()
                .push(input.as_bytes(), true)
                .is_err());
        }
    }

    fn timed_record(character: &str, start: f64) -> String {
        serde_json::json!({"audio_base64": "AQI=", "alignment": {
            "characters": [character],
            "character_start_times_seconds": [start],
            "character_end_times_seconds": [start + 0.1]
        }})
        .to_string()
            + "\n"
    }

    #[test]
    fn unusable_alignment_preserves_audio_without_resuming_shifted_captions() {
        let bad_alignments = [
            serde_json::Value::Null,
            serde_json::json!("wrong schema"),
            serde_json::json!({"characters": ["B"], "character_start_times_seconds": [], "character_end_times_seconds": []}),
            serde_json::json!({"characters": ["B"], "character_start_times_seconds": ["wrong type"], "character_end_times_seconds": [0.3]}),
            serde_json::json!({"characters": ["B"], "character_start_times_seconds": [0.3], "character_end_times_seconds": [0.2]}),
            serde_json::json!({"characters": ["B"], "character_start_times_seconds": [-1.0], "character_end_times_seconds": [0.3]}),
            serde_json::json!({"characters": ["B"], "character_start_times_seconds": [0.0], "character_end_times_seconds": [0.1]}),
            serde_json::json!({"characters": [], "character_start_times_seconds": [], "character_end_times_seconds": []}),
        ];
        for alignment in bad_alignments {
            let mut decoder = TimestampDecoder::new();
            let first = decoder
                .push(timed_record("A", 0.1).as_bytes(), false)
                .unwrap();
            let ChunkItem::Captions(prefix) = &first[0] else {
                panic!("expected initial valid captions")
            };
            let bad = serde_json::json!({"audio_base64": "AwQ=", "normalized_alignment": alignment})
                .to_string() + "\n";
            let rest = bad + &timed_record("C", 1.0);
            assert_eq!(
                decoder.push(rest.as_bytes(), true).unwrap(),
                vec![
                    ChunkItem::ClearCaptions,
                    ChunkItem::Pcm(vec![3, 4]),
                    ChunkItem::Pcm(vec![1, 2]),
                ]
            );
            // Previously returned metadata is unchanged, but no stale snapshot
            // remains in the decoder or gets extended across the missing B.
            assert_eq!(prefix.text, "A");
            assert!(decoder.captions.text.is_empty());
            assert!(decoder.captions.words.is_empty());
            let next = TimestampDecoder::new()
                .push(timed_record("C", 0.1).as_bytes(), true)
                .unwrap();
            assert!(matches!(&next[0], ChunkItem::Captions(c) if c.text == "C"));
        }
    }

    #[test]
    fn malformed_caption_schema_survives_arbitrary_transport_splits() {
        let input = "{\"audio_base64\":\"AQI=\",\"alignment\":{}}\n";
        for split in 0..input.len() {
            let mut decoder = TimestampDecoder::new();
            assert!(decoder
                .push(&input.as_bytes()[..split], false)
                .unwrap()
                .is_empty());
            assert_eq!(
                decoder.push(&input.as_bytes()[split..], true).unwrap(),
                vec![ChunkItem::ClearCaptions, ChunkItem::Pcm(vec![1, 2])]
            );
        }
    }

    #[test]
    fn rejected_provider_metadata_cannot_survive_stream_collection() {
        let wire = timed_record("A", 0.1)
            + "{\"audio_base64\":\"AwQ=\",\"alignment\":{}}\n"
            + &timed_record("C", 1.0);
        let mut decoder = TimestampDecoder::new();
        let (tx, rx) = std::sync::mpsc::channel();
        for item in decoder.push(wire.as_bytes(), true).unwrap() {
            tx.send(item).unwrap();
        }
        drop(tx);
        let meta = super::super::stream::AudioFormatMeta {
            sample_rate: 24000,
            channels: 1,
            bits_per_sample: 16,
        };
        let expected = super::super::stream::pcm_to_wav(&[1, 2, 3, 4, 1, 2], &meta);
        let speech =
            super::super::stream::collect_speech(super::super::stream::ChunkStream::new(meta, rx))
                .unwrap();
        assert_eq!(speech.bytes, expected);
        assert_eq!(speech.captions, None);
    }

    #[test]
    fn caption_degradation_does_not_disable_audio_validation() {
        let mut decoder = TimestampDecoder::new();
        assert_eq!(
            decoder
                .push(b"{\"audio_base64\":\"AQI=\",\"alignment\":{}}\n", false)
                .unwrap(),
            vec![ChunkItem::ClearCaptions, ChunkItem::Pcm(vec![1, 2])]
        );
        assert!(decoder
            .push(b"{\"audio_base64\":\"!\"}\n", true)
            .unwrap_err()
            .contains("Invalid ElevenLabs audio"));
    }

    #[test]
    fn absent_alignment_only_disables_captions_when_audio_is_present() {
        let mut decoder = TimestampDecoder::new();
        assert!(decoder
            .push(b"{\"audio_base64\":\"\"}\n", false)
            .unwrap()
            .is_empty());
        let first = decoder
            .push(timed_record("A", 0.1).as_bytes(), false)
            .unwrap();
        assert!(matches!(&first[0], ChunkItem::Captions(_)));
        assert_eq!(
            decoder
                .push(b"{\"audio_base64\":\"AQI=\"}\n", false)
                .unwrap(),
            vec![ChunkItem::ClearCaptions, ChunkItem::Pcm(vec![1, 2])]
        );
        assert_eq!(
            decoder
                .push(timed_record("C", 1.0).as_bytes(), true)
                .unwrap(),
            vec![ChunkItem::Pcm(vec![1, 2])]
        );
    }
}
