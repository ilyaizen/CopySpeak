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
    normalized_alignment: Option<CharacterAlignment>,
    alignment: Option<CharacterAlignment>,
}

pub(super) struct TimestampDecoder {
    pending: Vec<u8>,
    captions: CaptionAlignment,
}

impl TimestampDecoder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            captions: CaptionAlignment {
                text: String::new(),
                words: Vec::new(),
            },
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
            if let Some(alignment) = record.normalized_alignment.or(record.alignment) {
                if alignment.characters.len() != alignment.character_start_times_seconds.len()
                    || alignment.characters.len() != alignment.character_end_times_seconds.len()
                {
                    return Err("ElevenLabs alignment arrays have different lengths".into());
                }
                let mut offset = self.captions.text.encode_utf16().count();
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
                    self.captions.text.push_str(&character);
                    self.captions.words.push(WordTiming {
                        text_start: offset,
                        text_end: next,
                        start_ms: start * 1000.0,
                        end_ms: end * 1000.0,
                    });
                    offset = next;
                }
                self.captions.validate()?;
                items.push(ChunkItem::Captions(self.captions.clone()));
            }
            // Metadata is delivered before its audio can become audible.
            if !pcm.is_empty() {
                items.push(ChunkItem::Pcm(pcm));
            }
        }
        Ok(items)
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
    fn rejects_mismatched_arrays_and_malformed_audio() {
        for input in [
            r#"{"audio_base64":"!"}"#,
            r#"{"audio_base64":"", "normalized_alignment":{"characters":["a"],"character_start_times_seconds":[],"character_end_times_seconds":[]}}"#,
        ] {
            assert!(TimestampDecoder::new()
                .push(input.as_bytes(), true)
                .is_err());
        }
    }
}
