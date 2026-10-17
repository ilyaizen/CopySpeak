use super::block_on_async;
use super::captions::{estimate_alignment, CaptionAlignment, SpeechAudio};
use super::stream::{spawn_stream, AudioFormatMeta, ChunkItem, ChunkStream};
use super::{TtsBackend, TtsError, Voice};
use crate::config::FishConfig;
use reqwest::Client;
use serde_json::json;

const FISH_TTS_URL: &str = "https://api.fish.audio/v1/tts";
const FISH_MODEL_LIST_URL: &str = "https://api.fish.audio/model";

/// PCM rate of Fish's s2 models. Verified 44100 Hz / mono / 16-bit for
/// s2.1-pro-free via a WAV-header probe (2026-10-17); raw PCM carries no
/// header, so the paid models (s2-pro, drama-3) are assumed to match — a
/// mismatch would only surface as wrong-speed playback.
const FISH_PCM_SAMPLE_RATE: u32 = 44100;

pub struct FishTtsBackend {
    config: FishConfig,
}

impl FishTtsBackend {
    pub fn new(config: FishConfig) -> Self {
        Self { config }
    }

    /// Resolve the API key: config first, then the `FISH_API_KEY` env var.
    fn api_key(&self) -> String {
        crate::secrets::resolve(&self.config.api_key, &["FISH_API_KEY"])
    }

    fn request_body_with_format(&self, text: &str, format: &str) -> serde_json::Value {
        let mut body = json!({
            "text": text,
            "format": format,
        });
        // Empty voice = Fish Audio's built-in default voice: `reference_id`
        // must be omitted, not sent blank.
        let voice = self.config.voice_id.trim();
        if !voice.is_empty() {
            body["reference_id"] = json!(voice);
        }
        body
    }

    fn request_body(&self, text: &str) -> serde_json::Value {
        self.request_body_with_format(text, &self.config.format)
    }

    /// Streaming body: raw PCM (a container is only parseable once the whole
    /// body has arrived) at Fish's `balanced` latency, its lowest
    /// time-to-first-audio mode. The batch path keeps the profile's format,
    /// which is what file output and the cache want.
    fn stream_request_body(&self, text: &str) -> serde_json::Value {
        let mut body = self.request_body_with_format(text, "pcm");
        body["latency"] = json!("balanced");
        body
    }

    /// List voices from the Fish Audio model library (top-ranked community
    /// voice models). `reference_id` on the TTS request is one of these `_id`s
    /// (or an id of the caller's own cloned model).
    pub fn list_voices(&self) -> Result<Vec<Voice>, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "Fish Audio API key is missing".into(),
            ));
        }
        let (status, body) = block_on_async(async move {
            let client = Client::new();
            let response = client
                .get(FISH_MODEL_LIST_URL)
                .query(&[
                    ("page_size", "100"),
                    ("page_number", "1"),
                    ("sort_by", "score"),
                ])
                .bearer_auth(api_key)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| TtsError::Http(format!("Failed to fetch voices: {}", e)))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|e| TtsError::Http(format!("Failed to read voices response: {}", e)))?;
            Ok::<_, TtsError>((status, body))
        })?;

        if !status.is_success() {
            return Err(TtsError::Http(format!(
                "Fish Audio API error {}: {}",
                status,
                body.chars().take(300).collect::<String>()
            )));
        }

        #[derive(serde::Deserialize)]
        struct ModelPage {
            items: Vec<ModelEntity>,
        }
        #[derive(serde::Deserialize)]
        struct ModelEntity {
            #[serde(rename = "_id")]
            id: String,
            title: String,
            #[serde(default)]
            languages: Vec<String>,
        }

        let page: ModelPage = serde_json::from_str(&body)
            .map_err(|e| TtsError::Http(format!("Failed to parse voices response: {}", e)))?;
        Ok(page
            .items
            .into_iter()
            .map(|m| Voice {
                id: m.id,
                name: m.title,
                language: m.languages.into_iter().next(),
                default: None,
            })
            .collect())
    }
}

impl TtsBackend for FishTtsBackend {
    fn name(&self) -> &str {
        "Fish Audio"
    }

    fn supports_streaming(&self) -> bool {
        // Fish's /v1/tts streams PCM progressively for every model when asked
        // with format=pcm (probed 2026-10-17: first chunk at 0.61 s for a
        // 400-char read, continuous ~2x-realtime delivery after that).
        true
    }

    fn synthesize_streaming(&self, text: &str, _voice: &str) -> Result<ChunkStream, TtsError> {
        self.synthesize_streaming_from(FISH_TTS_URL, text)
    }

    fn synthesize(&self, text: &str, _voice: &str) -> Result<Vec<u8>, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "Fish Audio API key is missing".into(),
            ));
        }

        let body = self.request_body(text);

        log::info!(
            "Fish Audio TTS request - model: {}, voice: {}, format: {}, text length: {} chars",
            self.config.model,
            if self.config.voice_id.trim().is_empty() {
                "(default)"
            } else {
                &self.config.voice_id
            },
            self.config.format,
            text.len()
        );

        if crate::logging::is_debug_mode() {
            log::debug!(
                "Fish Audio TTS request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let start_time = std::time::Instant::now();

        let response_bytes = block_on_async(async {
            let client = Client::new();
            let response = client
                .post(FISH_TTS_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                // Fish selects the TTS model via a request header, not the body.
                .header("model", &self.config.model)
                .json(&body)
                .send()
                .await
                .map_err(|e| TtsError::Http(format!("Request failed: {e}")))?;

            let status = response.status();

            log::info!(
                "Fish Audio TTS response: {} {} (took {:?})",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                start_time.elapsed()
            );

            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                log::error!("Fish Audio API error {}: {}", status, error_text);
                return Err(TtsError::Http(format!(
                    "Fish Audio API error {}: {}",
                    status, error_text
                )));
            }

            // Read the body inside THIS block: the response's connection lives
            // on the runtime running this future, and `block_on_async` builds a
            // fresh runtime per call when none is ambient — a response carried
            // across calls loses its connection driver.
            let bytes = response.bytes().await.map_err(|e| {
                log::error!("Failed to read response bytes: {}", e);
                TtsError::Http(format!("Failed to read bytes: {}", e))
            })?;

            log::info!(
                "Fish Audio TTS synthesis complete: received {} bytes",
                bytes.len()
            );

            Ok(bytes.to_vec())
        })?;

        Ok(response_bytes)
    }

    /// No native timings: decode the finished audio for its exact duration
    /// and place the words proportionally inside it (estimation recipe).
    fn synthesize_with_captions(&self, text: &str, voice: &str) -> Result<SpeechAudio, TtsError> {
        let bytes = self.synthesize(text, voice)?;
        // An undecodable container still plays; it just carries no captions.
        let captions = captions_for_audio(&bytes, text);
        Ok(SpeechAudio { bytes, captions })
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("Fish Audio TTS health check - validating API key");

        if self.api_key().trim().is_empty() {
            log::error!("Fish Audio TTS health check failed - API key is missing");
            return Err(TtsError::Unavailable(
                "Fish Audio API key is missing".into(),
            ));
        }

        log::debug!("Fish Audio TTS health check passed");
        Ok(())
    }

    fn file_extension(&self) -> &str {
        match self.config.format.as_str() {
            "" => "wav",
            other => other,
        }
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}

/// Duration of finished audio in milliseconds, `None` when the container
/// cannot be measured. Parses the WAV header instead of decoding: rodio's
/// decoder trusts the data-chunk size field, and Fish writes 0xFFFFFF00
/// there while shipping the real bytes after it, so a decode iterates past
/// EOF forever (the busy-tray-with-no-audio hang). Header math is exact;
/// the declared size is clamped to the actual buffer so a lying or
/// truncated header cannot extend the measurement.
fn decoded_duration_ms(bytes: &[u8]) -> Option<f64> {
    let info = crate::audio::wav::parse_wav_header(bytes).ok()?;
    let bytes_per_second = f64::from(info.sample_rate)
        * f64::from(info.channels)
        * f64::from(info.bits_per_sample / 8);
    if bytes_per_second <= 0.0 {
        return None;
    }
    let available = bytes.len().saturating_sub(info.data_offset);
    let pcm_bytes = info.data_size.min(available);
    if info.data_size > available {
        log::debug!(
            "[TTS] WAV data chunk claims {} bytes but only {} follow the header; measuring the real bytes",
            info.data_size,
            available
        );
    }
    (pcm_bytes > 0).then(|| pcm_bytes as f64 * 1000.0 / bytes_per_second)
}

/// Estimated word timings for finished audio bytes, `None` without decodable
/// audio or with no words to time.
fn captions_for_audio(bytes: &[u8], text: &str) -> Option<CaptionAlignment> {
    let alignment = estimate_alignment(text, decoded_duration_ms(bytes)?);
    (!alignment.words.is_empty()).then_some(alignment)
}

impl FishTtsBackend {
    /// Open a progressive PCM stream. Split from
    /// [`TtsBackend::synthesize_streaming`] so tests can point it at a local
    /// mock server, mirroring the elevenlabs/cartesia shape.
    fn synthesize_streaming_from(
        &self,
        base_url: &str,
        text: &str,
    ) -> Result<ChunkStream, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "Fish Audio API key is missing".into(),
            ));
        }

        let body = self.stream_request_body(text);
        let model = self.config.model.clone();
        let url = base_url.to_string();
        let text = text.to_string();

        log::info!(
            "Fish Audio TTS stream - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            if self.config.voice_id.trim().is_empty() {
                "(default)"
            } else {
                &self.config.voice_id
            },
            text.len()
        );

        let meta = AudioFormatMeta {
            sample_rate: FISH_PCM_SAMPLE_RATE,
            channels: 1,
            bits_per_sample: 16,
        };
        Ok(spawn_stream(meta, move |sink| async move {
            let mut response = Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                // Fish selects the TTS model via a request header, not the body.
                .header("model", model)
                .json(&body)
                .send()
                .await
                .map_err(|e| TtsError::Http(format!("Fish Audio stream request failed: {e}")))?;
            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(TtsError::Http(format!(
                    "Fish Audio API error {status}: {error_text}"
                )));
            }
            let mut total_bytes = 0usize;
            while let Some(chunk) = response.chunk().await.map_err(|e| {
                TtsError::Http(format!(
                    "Fish Audio stream read failed after {total_bytes} bytes: {e}"
                ))
            })? {
                if chunk.is_empty() {
                    continue;
                }
                total_bytes += chunk.len();
                if !sink.send(ChunkItem::Pcm(chunk.to_vec())) {
                    // Consumer dropped (stop/abort): stop pulling the body.
                    return Ok(());
                }
            }
            // Fish returns no word timings: the measured PCM byte count IS the
            // exact duration, so place the words inside it once the body ends.
            // Sent after the final PCM — live consumers update mid-playback,
            // and the drained SpeechAudio carries captions for
            // history/cache/sidecar storage.
            if total_bytes > 0 {
                let duration_ms =
                    total_bytes as f64 * 1000.0 / (f64::from(FISH_PCM_SAMPLE_RATE) * 2.0);
                let alignment = estimate_alignment(&text, duration_ms);
                if !alignment.words.is_empty() {
                    let _ = sink.send(ChunkItem::Captions(alignment));
                }
            }
            Ok(())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

    fn backend(voice_id: &str) -> FishTtsBackend {
        FishTtsBackend::new(FishConfig {
            voice_id: voice_id.into(),
            ..Default::default()
        })
    }

    #[test]
    fn body_omits_reference_id_for_default_voice() {
        // Empty voice = Fish's built-in default voice; sending `reference_id`
        // blank would ask the API for a model with an empty id.
        let body = backend("").request_body("hello");
        assert!(body.get("reference_id").is_none());
        assert_eq!(body["text"], "hello");
        assert_eq!(body["format"], "wav");
    }

    #[test]
    fn body_carries_reference_id_when_voice_is_set() {
        let body = backend("933563129e564b19a115bedd57b7406a").request_body("hello");
        assert_eq!(body["reference_id"], "933563129e564b19a115bedd57b7406a");
    }

    #[test]
    fn file_extension_follows_format() {
        assert_eq!(backend("").file_extension(), "wav");
        let mp3 = FishTtsBackend::new(FishConfig {
            format: "mp3".into(),
            ..Default::default()
        });
        assert_eq!(mp3.file_extension(), "mp3");
    }

    #[test]
    fn streaming_body_pins_pcm_and_balanced_latency() {
        let body = backend("").stream_request_body("hello");
        assert_eq!(body["format"], "pcm");
        assert_eq!(body["latency"], "balanced");
        assert!(body.get("reference_id").is_none());
    }

    #[test]
    fn streaming_body_keeps_reference_id_when_voice_is_set() {
        let body = backend("933563129e564b19a115bedd57b7406a").stream_request_body("hello");
        assert_eq!(body["format"], "pcm");
        assert_eq!(body["latency"], "balanced");
        assert_eq!(body["reference_id"], "933563129e564b19a115bedd57b7406a");
    }

    #[test]
    fn supports_streaming_reports_true() {
        assert!(backend("").supports_streaming());
    }

    #[test]
    fn batch_captions_estimate_inside_the_decoded_duration() {
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        // 16000 bytes = 8000 samples = exactly one second at 8 kHz mono.
        let bytes = super::super::stream::pcm_to_wav(&vec![0; 16000], &meta);
        let captions = captions_for_audio(&bytes, "Hello, world!").expect("decodable wav");
        assert_eq!(captions.text, "Hello, world!");
        captions.validate().unwrap();
        let last = captions.words.last().unwrap();
        assert!(
            (last.end_ms - 1000.0).abs() < 1.0,
            "last word must end at the decoded duration: {}",
            last.end_ms
        );
        assert_eq!(captions.words[0].text_start, 0);
    }

    #[test]
    fn undecodable_batch_audio_yields_no_captions() {
        assert!(captions_for_audio(&[1, 2, 3, 4], "hello").is_none());
    }

    /// Fish writes 0xFFFFFF00 into the WAV data-chunk size field while
    /// shipping the real bytes after it. The duration must come from the real
    /// bytes — the previous rodio decode trusted the field and spun at EOF
    /// forever, hanging the whole synthesis queue (busy tray, no audio, no
    /// error).
    #[test]
    fn lying_data_chunk_size_yields_the_real_duration() {
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        // One second of PCM inside a canonical WAV...
        let mut bytes = super::super::stream::pcm_to_wav(&vec![0u8; 16000], &meta);
        // ...with the data-chunk size overwritten by Fish's lie.
        let data_size_field = 40; // riff(12) + fmt(8 + 16) → size field of data
        bytes[data_size_field..data_size_field + 4].copy_from_slice(&0xFFFFFF00u32.to_le_bytes());

        let captions = captions_for_audio(&bytes, "Hello, world!").expect("real bytes measured");
        captions.validate().unwrap();
        let last = captions.words.last().unwrap();
        assert!(
            (last.end_ms - 1000.0).abs() < 1.0,
            "last word must end at the real duration, not the claimed 4 GB: {}",
            last.end_ms
        );

        // A transfer cut mid-body must measure only what arrived (whole frames).
        bytes.truncate(bytes.len() - 10);
        let captions = captions_for_audio(&bytes, "Hello, world!").expect("truncated still parses");
        let last = captions.words.last().unwrap();
        assert!(
            (990.0..1000.0).contains(&last.end_ms),
            "truncated audio must measure its real frames: {}",
            last.end_ms
        );
    }

    /// Two-phase mock server: half the advertised body arrives, the response
    /// is held open, and the client must surface PCM before the body
    /// completes — the progressive behavior the live API exhibits.
    #[test]
    fn streams_pcm_while_response_is_open_with_pcm_body_and_model_header() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/tts", listener.local_addr().unwrap());
        let (first_sent_tx, first_sent_rx) = std::sync::mpsc::channel::<()>();
        let (second_go_tx, second_go_rx) = std::sync::mpsc::channel::<()>();
        let response_body: Vec<u8> = vec![7u8; 64];
        let server_body = response_body.clone();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request).unwrap().to_lowercase();
            assert!(headers.starts_with("post /v1/tts http/1.1"));
            assert!(headers.contains("authorization: bearer test-key\r\n"));
            assert!(headers.contains("model: s2.1-pro-free\r\n"));
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["text"], "hello");
            assert_eq!(body["format"], "pcm");
            assert_eq!(body["latency"], "balanced");
            assert!(body.get("reference_id").is_none());

            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                server_body.len()
            )
            .unwrap();
            socket.write_all(&server_body[..32]).unwrap();
            socket.flush().unwrap();
            first_sent_tx.send(()).unwrap();
            second_go_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            socket.write_all(&server_body[32..]).unwrap();
            socket.flush().unwrap();
        });

        let backend = FishTtsBackend::new(FishConfig {
            api_key: "test-key".into(),
            model: "s2.1-pro-free".into(),
            ..Default::default()
        });
        assert!(backend.supports_streaming());
        let stream = backend.synthesize_streaming_from(&url, "hello").unwrap();
        assert_eq!(stream.meta.sample_rate, 44100);
        assert_eq!(stream.meta.channels, 1);
        assert_eq!(stream.meta.bits_per_sample, 16);

        first_sent_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let Some(ChunkItem::Pcm(first)) = stream.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("first chunk must be PCM arriving while the response is open")
        };
        assert!(!first.is_empty() && first.iter().all(|&b| b == 7));
        assert!(
            first.len() <= 32,
            "client must not have buffered past the open response"
        );
        second_go_tx.send(()).unwrap();
        let mut total = first.len();
        let mut captions = None;
        let mut captions_before_pcm = false;
        while let Some(item) = stream.recv_timeout(Duration::from_secs(5)).unwrap() {
            match item {
                ChunkItem::Pcm(bytes) => {
                    if captions.is_some() {
                        captions_before_pcm = true;
                    }
                    total += bytes.len();
                }
                ChunkItem::Captions(value) => captions = Some(value),
                ChunkItem::Failed(reason) => panic!("stream failed: {reason}"),
                _ => {}
            }
        }
        assert_eq!(total, 64, "all advertised PCM must arrive");
        let captions = captions.expect("estimated captions must follow the final PCM");
        assert!(!captions_before_pcm, "captions must not precede the PCM");
        assert_eq!(captions.text, "hello");
        captions.validate().unwrap();
        assert_eq!(captions.words.len(), 1);
        assert_eq!(captions.words[0].text_start, 0);
        assert_eq!(captions.words[0].text_end, 5);
        assert!(captions.words[0].end_ms > 0.0);
        server.join().unwrap();
    }

    /// Live synthesis against api.fish.audio. Opt-in: needs `FISH_API_KEY` in
    /// the environment (e.g. exported from `target/debug/.env`); runs only via
    /// `cargo test fish_live -- --ignored`. Uses the free-tier model so it
    /// never consumes paid credits.
    #[test]
    #[ignore = "live API test; needs FISH_API_KEY in the environment"]
    fn fish_live_synthesizes_with_default_voice() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(api_key) = std::env::var("FISH_API_KEY") else {
            eprintln!("FISH_API_KEY not set; skipping live synthesis test");
            return Ok(());
        };
        let backend = FishTtsBackend::new(FishConfig {
            api_key,
            model: "s2.1-pro-free".into(),
            format: "wav".into(),
            ..Default::default()
        });
        let bytes = backend
            .synthesize("Hello from the CopySpeak live test.", "")
            .expect("live synthesis must succeed");
        assert!(
            bytes.len() > 10_000,
            "suspiciously small audio: {}",
            bytes.len()
        );
        assert_eq!(&bytes[..4], b"RIFF", "expected a WAV container");
        Ok(())
    }

    /// Live batch synthesis must also produce captions: the estimator decodes
    /// the container, and a real Fish WAV is the input no synthetic fixture
    /// matches. The decode runs under a watchdog because a hostile container
    /// could spin instead of erroring. Opt-in: needs `FISH_API_KEY`; runs only
    /// via `cargo test fish_live_batch_captions -- --ignored`.
    #[test]
    #[ignore = "live API test; needs FISH_API_KEY in the environment"]
    fn fish_live_batch_captions_decode_without_hanging() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(api_key) = std::env::var("FISH_API_KEY") else {
            eprintln!("FISH_API_KEY not set; skipping live batch captions test");
            return Ok(());
        };
        const TEXT: &str = "One two three, testing the batch caption decode.";
        let backend = FishTtsBackend::new(FishConfig {
            api_key,
            model: "s2.1-pro-free".into(),
            format: "wav".into(),
            ..Default::default()
        });
        let bytes = backend
            .synthesize(TEXT, "")
            .expect("live synthesis must succeed");
        let dump = std::env::temp_dir().join("fish-live-batch.wav");
        std::fs::write(&dump, &bytes)?;
        eprintln!("dumped {} bytes to {}", bytes.len(), dump.display());
        let (tx, rx) = std::sync::mpsc::channel();
        let for_decode = bytes.clone();
        let text = TEXT.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(captions_for_audio(&for_decode, &text));
        });
        let captions = rx
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| "caption decode hung on the live Fish WAV (>15s)")?;
        let captions = captions.expect("live wav must decode for estimation");
        captions.validate()?;
        assert!(!captions.words.is_empty());
        Ok(())
    }

    /// Live progressive streaming against api.fish.audio. Opt-in: needs
    /// `FISH_API_KEY` in the environment; runs only via
    /// `cargo test fish_live_streams -- --ignored`.
    #[test]
    #[ignore = "live API test; needs FISH_API_KEY in the environment"]
    fn fish_live_streams_pcm_with_default_voice() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(api_key) = std::env::var("FISH_API_KEY") else {
            eprintln!("FISH_API_KEY not set; skipping live streaming test");
            return Ok(());
        };
        let backend = FishTtsBackend::new(FishConfig {
            api_key,
            model: "s2.1-pro-free".into(),
            format: "wav".into(),
            ..Default::default()
        });
        let stream = backend
            .synthesize_streaming(
                "The streaming live test reads one full sentence aloud for verification.",
                "",
            )
            .expect("live stream must open");
        assert_eq!(stream.meta.sample_rate, FISH_PCM_SAMPLE_RATE);
        let mut pcm = Vec::new();
        let mut captions = None;
        while let Some(item) = stream.recv() {
            match item {
                ChunkItem::Pcm(bytes) => pcm.extend_from_slice(&bytes),
                ChunkItem::Captions(value) => captions = Some(value),
                ChunkItem::Failed(reason) => panic!("stream failed mid-way: {reason}"),
                _ => {}
            }
        }
        // One spoken sentence at 44.1 kHz / 16-bit / mono is ~88 KB/s; anything
        // under 10 KB is not a real synthesis.
        assert!(pcm.len() > 10_000, "suspiciously small pcm: {}", pcm.len());
        let captions = captions.expect("live stream must end with estimated captions");
        assert_eq!(
            captions.text,
            "The streaming live test reads one full sentence aloud for verification."
        );
        captions.validate().unwrap();
        Ok(())
    }
}
