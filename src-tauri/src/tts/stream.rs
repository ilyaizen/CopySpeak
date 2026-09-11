// Streaming contract shared by all TTS backends.
//
// A backend that supports intra-request streaming produces PCM chunks through
// a bounded channel; the command layer forwards them to the frontend without
// knowing which engine produced them. Backends without native streaming get a
// default implementation that wraps their existing batch `synthesize` output
// as a single-chunk stream, so consumers never special-case engines.

use serde::Serialize;
use std::sync::mpsc;
use std::time::Duration;

use super::TtsError;
use crate::audio::wav::{parse_wav_header, WavInfo};

/// Format metadata describing the PCM carried by a [`ChunkStream`].
/// Serialized verbatim in audio-stream-chunk events.
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct AudioFormatMeta {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

impl From<&WavInfo> for AudioFormatMeta {
    fn from(info: &WavInfo) -> Self {
        Self {
            sample_rate: info.sample_rate,
            channels: info.channels,
            bits_per_sample: info.bits_per_sample,
        }
    }
}

/// One item delivered by a [`ChunkStream`].
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkItem {
    /// Raw PCM payload in the format described by the stream's meta.
    Pcm(Vec<u8>),
    /// Complete snapshot so far; offsets and times are relative to this request.
    Captions(super::captions::CaptionAlignment),
    /// Rejected optional metadata invalidates the retained snapshot, not audio.
    ClearCaptions,
    /// The synthesis failed mid-stream; carries a human-readable reason.
    /// Constructed by native-streaming backends (later slice tasks).
    #[allow(dead_code)]
    Failed(String),
}

/// A sequence of PCM chunks plus the format needed to play them.
///
/// The receiver side of an unbounded mpsc channel. Channel close signals
/// end-of-stream; a mid-stream failure arrives as a [`ChunkItem::Failed`].
pub struct ChunkStream {
    /// Consumed by the command layer when emitting audio-stream-chunk events
    /// (later slice tasks); tests assert it here.
    #[allow(dead_code)]
    pub meta: AudioFormatMeta,
    rx: mpsc::Receiver<ChunkItem>,
}

impl ChunkStream {
    /// Wrap a receiver into a stream. Used by producers (backends) and tests.
    #[allow(dead_code)]
    pub fn new(meta: AudioFormatMeta, rx: mpsc::Receiver<ChunkItem>) -> Self {
        Self { meta, rx }
    }

    /// Block until the next chunk. Returns `None` once the producer closed
    /// the channel (end of stream).
    #[allow(dead_code)]
    pub fn recv(&self) -> Option<ChunkItem> {
        self.rx.recv().ok()
    }

    /// Block up to `timeout` for the next chunk so the consumer can enforce
    /// an idle timeout between chunks.
    ///
    /// - `Ok(Some(item))` — a chunk arrived in time.
    /// - `Ok(None)` — producer closed the channel: end of stream.
    /// - `Err(RecvTimeoutError::Timeout)` — no chunk within `timeout`.
    /// - `Err(RecvTimeoutError::Disconnected)` — producer closed while waiting.
    #[allow(dead_code)]
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<ChunkItem>, mpsc::RecvTimeoutError> {
        match self.rx.recv_timeout(timeout) {
            Ok(item) => Ok(Some(item)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(mpsc::RecvTimeoutError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(self.rx.try_recv().ok()),
        }
    }
}

/// Build a single-chunk [`ChunkStream`] from a complete WAV byte buffer.
///
/// Parses the container with the existing WAV helpers, extracts format
/// metadata and the data-section payload, and delivers the payload as exactly
/// one `Pcm` chunk. This backs the default `synthesize_streaming`
/// implementation for batch-only backends.
pub fn chunk_stream_from_wav(wav: Vec<u8>) -> Result<ChunkStream, TtsError> {
    chunk_stream_from_speech(wav.into())
}

pub fn chunk_stream_from_speech(
    speech: super::captions::SpeechAudio,
) -> Result<ChunkStream, TtsError> {
    let wav = speech.bytes;
    let info = parse_wav_header(&wav).map_err(TtsError::Http)?;

    let data_start = info.data_offset;
    let data_end = data_start + info.data_size;
    if data_end > wav.len() || info.data_size == 0 {
        log::error!(
            "[TTS] WAV data chunk out of bounds (offset {}, size {}, buffer {} bytes)",
            data_start,
            info.data_size,
            wav.len()
        );
        return Err(TtsError::Http(
            "WAV data chunk extends beyond file".to_string(),
        ));
    }

    let pcm = wav[data_start..data_end].to_vec();
    log::debug!(
        "[TTS] Batch-wrapped stream: {} PCM bytes, {} Hz / {} ch / {} bit",
        pcm.len(),
        info.sample_rate,
        info.channels,
        info.bits_per_sample
    );

    let (tx, rx) = mpsc::channel();
    if let Some(captions) = speech.captions {
        match captions.validate() {
            Ok(()) => {
                let _ = tx.send(ChunkItem::Captions(captions));
            }
            Err(error) => log::warn!("Ignoring invalid batch captions: {error}"),
        }
    }
    let _ = tx.send(ChunkItem::Pcm(pcm));
    drop(tx); // closing the sender signals end-of-stream after the single chunk

    Ok(ChunkStream {
        meta: AudioFormatMeta::from(&info),
        rx,
    })
}

/// Collect the same native stream when playback is configured for batch mode.
pub fn collect_speech(stream: ChunkStream) -> Result<super::captions::SpeechAudio, TtsError> {
    let mut pcm = Vec::new();
    let mut captions = None;
    while let Some(item) = stream.recv() {
        match item {
            ChunkItem::Pcm(bytes) => pcm.extend_from_slice(&bytes),
            ChunkItem::Captions(value) => {
                captions = match value.validate() {
                    Ok(()) => Some(value),
                    Err(error) => {
                        log::warn!("Ignoring invalid stream captions: {error}");
                        None
                    }
                };
            }
            ChunkItem::ClearCaptions => captions = None,
            ChunkItem::Failed(reason) => return Err(TtsError::Http(reason)),
        }
    }
    if pcm.is_empty() {
        return Err(TtsError::Http("Stream produced no audio".into()));
    }
    Ok(super::captions::SpeechAudio {
        bytes: pcm_to_wav(&pcm, &stream.meta),
        captions,
    })
}

/// Wrap raw PCM bytes into a minimal canonical WAV container (44-byte header).
///
/// Used by the command layer to package accumulated streaming PCM into a
/// single WAV for history, cache, and HUD envelope computation.
pub fn pcm_to_wav(pcm: &[u8], meta: &AudioFormatMeta) -> Vec<u8> {
    let byte_rate =
        meta.sample_rate * u32::from(meta.channels) * u32::from(meta.bits_per_sample) / 8;
    let block_align = meta.channels * meta.bits_per_sample / 8;

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&((36 + pcm.len()) as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format tag
    wav.extend_from_slice(&meta.channels.to_le_bytes());
    wav.extend_from_slice(&meta.sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&meta.bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn batch_stream_round_trip_preserves_native_captions() {
        let captions = super::super::captions::CaptionAlignment {
            text: "Hi".into(),
            words: vec![super::super::captions::WordTiming {
                text_start: 0,
                text_end: 2,
                start_ms: 1.0,
                end_ms: 2.0,
            }],
        };
        let bytes = build_wav(8000, 1, 16, &[0; 48]);
        let speech = collect_speech(
            chunk_stream_from_speech(super::super::captions::SpeechAudio {
                bytes: bytes.clone(),
                captions: Some(captions.clone()),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(speech.bytes, bytes);
        assert_eq!(speech.captions, Some(captions));
    }

    #[test]
    fn invalid_batch_captions_do_not_reject_valid_wav() {
        let bytes = build_wav(8000, 1, 16, &[1, 2, 3, 4]);
        let captions = super::super::captions::CaptionAlignment {
            text: "Hi".into(),
            words: vec![super::super::captions::WordTiming {
                text_start: 0,
                text_end: 3,
                start_ms: 0.0,
                end_ms: 1.0,
            }],
        };
        assert!(captions.validate().is_err());
        let speech = collect_speech(
            chunk_stream_from_speech(super::super::captions::SpeechAudio {
                bytes: bytes.clone(),
                captions: Some(captions),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(speech.bytes, bytes);
        assert_eq!(speech.captions, None);
    }

    #[test]
    fn invalid_snapshot_clears_collected_captions_without_losing_pcm() {
        let valid = super::super::captions::CaptionAlignment {
            text: "Hi".into(),
            words: vec![super::super::captions::WordTiming {
                text_start: 0,
                text_end: 2,
                start_ms: 0.0,
                end_ms: 0.1,
            }],
        };
        let mut invalid = valid.clone();
        invalid.words[0].start_ms = -1.0;
        let (tx, rx) = mpsc::channel();
        for item in [
            ChunkItem::Captions(valid.clone()),
            ChunkItem::Pcm(vec![1, 2]),
            ChunkItem::Captions(invalid),
            ChunkItem::Pcm(vec![3, 4]),
        ] {
            tx.send(item).unwrap();
        }
        drop(tx);
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        let speech = collect_speech(ChunkStream::new(meta, rx)).unwrap();
        assert_eq!(speech.bytes, build_wav(8000, 1, 16, &[1, 2, 3, 4]));
        assert_eq!(speech.captions, None);

        let next = collect_speech(
            chunk_stream_from_speech(super::super::captions::SpeechAudio {
                bytes: build_wav(8000, 1, 16, &[5, 6]),
                captions: Some(valid.clone()),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(next.captions, Some(valid));
    }

    #[test]
    fn provider_caption_clear_removes_the_collected_snapshot() {
        let captions = super::super::captions::CaptionAlignment {
            text: "Hi".into(),
            words: vec![super::super::captions::WordTiming {
                text_start: 0,
                text_end: 2,
                start_ms: 0.0,
                end_ms: 0.1,
            }],
        };
        let (tx, rx) = mpsc::channel();
        for item in [
            ChunkItem::Captions(captions),
            ChunkItem::Pcm(vec![1, 2]),
            ChunkItem::ClearCaptions,
            ChunkItem::Pcm(vec![3, 4]),
        ] {
            tx.send(item).unwrap();
        }
        drop(tx);
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        let speech = collect_speech(ChunkStream::new(meta, rx)).unwrap();
        assert_eq!(speech.captions, None);
        assert_eq!(speech.bytes, build_wav(8000, 1, 16, &[1, 2, 3, 4]));
    }

    #[test]
    fn collection_still_rejects_empty_audio_and_explicit_failure() {
        for items in [
            vec![],
            vec![
                ChunkItem::Pcm(vec![1, 2]),
                ChunkItem::Failed("engine failed".into()),
            ],
        ] {
            let (tx, rx) = mpsc::channel();
            for item in items {
                tx.send(item).unwrap();
            }
            drop(tx);
            let meta = AudioFormatMeta {
                sample_rate: 8000,
                channels: 1,
                bits_per_sample: 16,
            };
            assert!(collect_speech(ChunkStream::new(meta, rx)).is_err());
        }
    }

    /// Build a minimal valid WAV: RIFF header + fmt chunk + data chunk.
    fn build_wav(sample_rate: u32, channels: u16, bits_per_sample: u16, pcm: &[u8]) -> Vec<u8> {
        let fmt = vec![
            &1u16.to_le_bytes()[..],        // PCM format tag
            &channels.to_le_bytes()[..],    // channel count
            &sample_rate.to_le_bytes()[..], // sample rate
            &(sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8).to_le_bytes()[..], // byte rate
            &(channels * bits_per_sample / 8).to_le_bytes()[..], // block align
            &bits_per_sample.to_le_bytes()[..],                  // bits per sample
        ]
        .concat();

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        let riff_size = (4 + 8 + fmt.len() + 8 + pcm.len()) as u32;
        wav.extend_from_slice(&riff_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        wav.extend_from_slice(&fmt);
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(pcm);
        wav
    }

    #[test]
    fn wav_buffer_becomes_single_pcm_chunk_with_parsed_meta() {
        let pcm = vec![0u8, 1u8, 2u8, 3u8, 254u8, 255u8];
        let wav = build_wav(44100, 1, 16, &pcm);

        let stream = chunk_stream_from_wav(wav).expect("valid WAV must parse");

        assert_eq!(stream.meta.sample_rate, 44100);
        assert_eq!(stream.meta.channels, 1);
        assert_eq!(stream.meta.bits_per_sample, 16);

        // Exactly one chunk equal to the data section...
        assert!(matches!(stream.recv(), Some(ChunkItem::Pcm(bytes)) if bytes == pcm));
        // ...then channel-close signals end of stream.
        assert!(stream.recv().is_none());
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(chunk_stream_from_wav(Vec::new()).is_err());

        // Valid header shape, but not a WAV (missing WAVE marker).
        let mut junk = b"RIFF\x24\x00\x00\x00JUNK".to_vec();
        junk.resize(64, 0);
        assert!(chunk_stream_from_wav(junk).is_err());
    }

    #[test]
    fn failed_item_passes_through_recv() {
        let meta = AudioFormatMeta {
            sample_rate: 22050,
            channels: 2,
            bits_per_sample: 16,
        };
        let (tx, rx) = mpsc::channel();
        tx.send(ChunkItem::Failed("engine died mid-stream".into()))
            .unwrap();
        drop(tx);

        let stream = ChunkStream::new(meta, rx);
        assert!(
            matches!(stream.recv(), Some(ChunkItem::Failed(reason)) if reason.contains("mid-stream"))
        );
        assert!(stream.recv().is_none());
    }

    #[test]
    fn channel_close_signals_end_of_stream_via_recv_and_recv_timeout() {
        let meta = AudioFormatMeta {
            sample_rate: 16000,
            channels: 1,
            bits_per_sample: 16,
        };
        let (_, rx) = mpsc::channel::<ChunkItem>();
        let stream = ChunkStream::new(meta, rx);

        assert!(stream.recv().is_none());
        assert_eq!(
            stream.recv_timeout(Duration::from_millis(50)),
            Ok(None),
            "closed channel resolves recv_timeout immediately with end-of-stream"
        );
    }

    #[test]
    fn pcm_to_wav_round_trips_through_chunk_stream_from_wav() {
        let meta = AudioFormatMeta {
            sample_rate: 24000,
            channels: 2,
            bits_per_sample: 16,
        };
        let pcm = vec![7u8; 40];
        let wav = pcm_to_wav(&pcm, &meta);

        let stream = chunk_stream_from_wav(wav).expect("generated WAV must parse");
        assert_eq!(stream.meta.sample_rate, 24000);
        assert_eq!(stream.meta.channels, 2);
        assert_eq!(stream.meta.bits_per_sample, 16);
        assert!(matches!(stream.recv(), Some(ChunkItem::Pcm(bytes)) if bytes == pcm));
    }

    #[test]
    fn recv_timeout_times_out_when_producer_silent_then_delivers_late_chunk() {
        let meta = AudioFormatMeta {
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(120));
            let _ = tx.send(ChunkItem::Pcm(vec![9, 9]));
        });

        let stream = ChunkStream::new(meta, rx);
        assert_eq!(
            stream.recv_timeout(Duration::from_millis(20)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "silent producer must surface Timeout, not block"
        );
        assert!(matches!(
            stream.recv_timeout(Duration::from_secs(2)),
            Ok(Some(ChunkItem::Pcm(_)))
        ));
    }
}
