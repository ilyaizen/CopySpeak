use super::captions::{CaptionAlignment, WordTiming};
use super::stream::{AudioFormatMeta, ChunkItem, ChunkStream};
use super::{TtsBackend, TtsError};
use crate::config::CartesiaConfig;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const CARTESIA_TTS_URL: &str = "https://api.cartesia.ai/tts/bytes";
const CARTESIA_TTS_SSE_URL: &str = "https://api.cartesia.ai/tts/sse";
const CARTESIA_VERSION: &str = "2024-06-10";

#[derive(Deserialize)]
struct CartesiaSseEvent {
    #[serde(rename = "type")]
    kind: String,
    data: Option<String>,
    word_timestamps: Option<CartesiaWordTimestamps>,
    title: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct CartesiaWordTimestamps {
    words: Vec<String>,
    start: Vec<f64>,
    end: Vec<f64>,
}

/// Cartesia sends each word timestamp after the PCM that contains it. Hold
/// that PCM until its timestamp arrives so metadata always reaches the player
/// before the corresponding audio can become audible.
struct CartesiaSseDecoder {
    pending: Vec<u8>,
    buffered_pcm: Vec<u8>,
    captions: CaptionAlignment,
    text_cursor: usize,
    alignment_active: bool,
}

impl CartesiaSseDecoder {
    fn new(text: String) -> Self {
        Self {
            pending: Vec::new(),
            buffered_pcm: Vec::new(),
            captions: CaptionAlignment {
                text,
                words: Vec::new(),
            },
            text_cursor: 0,
            alignment_active: true,
        }
    }

    fn push(&mut self, bytes: &[u8], final_chunk: bool) -> Result<Vec<ChunkItem>, String> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > 16 * 1024 * 1024 {
            return Err("Cartesia SSE frame exceeds 16 MiB".into());
        }

        let mut items = Vec::new();
        while let Some((end, delimiter_len)) = Self::frame_end(&self.pending) {
            let frame: Vec<u8> = self.pending.drain(..end).collect();
            self.pending.drain(..delimiter_len);
            self.decode_frame(&frame, &mut items)?;
        }
        if final_chunk && !self.pending.is_empty() {
            let frame = std::mem::take(&mut self.pending);
            self.decode_frame(&frame, &mut items)?;
        }
        if final_chunk {
            self.flush_audio(&mut items);
        }
        Ok(items)
    }

    fn frame_end(bytes: &[u8]) -> Option<(usize, usize)> {
        let lf = bytes
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|i| (i, 2));
        let crlf = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|i| (i, 4));
        [lf, crlf].into_iter().flatten().min_by_key(|(i, _)| *i)
    }

    fn decode_frame(&mut self, frame: &[u8], items: &mut Vec<ChunkItem>) -> Result<(), String> {
        let frame = std::str::from_utf8(frame)
            .map_err(|error| format!("Invalid Cartesia SSE text: {error}"))?;
        let payload = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if payload.is_empty() {
            return Ok(());
        }
        let event: CartesiaSseEvent = serde_json::from_str(&payload)
            .map_err(|error| format!("Invalid Cartesia SSE event: {error}"))?;
        match event.kind.as_str() {
            "chunk" => {
                let data = event.data.ok_or("Cartesia audio event has no data")?;
                self.buffered_pcm.extend(
                    STANDARD
                        .decode(data)
                        .map_err(|error| format!("Invalid Cartesia audio: {error}"))?,
                );
            }
            "timestamps" => {
                let Some(timestamps) = event.word_timestamps else {
                    return Err("Cartesia timestamp event has no word timestamps".into());
                };
                if !timestamps.words.is_empty() && self.extend_captions(timestamps) {
                    items.push(ChunkItem::Captions(self.captions.clone()));
                }
                self.flush_audio(items);
            }
            "done" => self.flush_audio(items),
            "error" => {
                return Err(event
                    .message
                    .or(event.title)
                    .unwrap_or_else(|| "Cartesia synthesis failed".into()));
            }
            _ => {}
        }
        Ok(())
    }

    fn extend_captions(&mut self, timestamps: CartesiaWordTimestamps) -> bool {
        if !self.alignment_active
            || timestamps.words.len() != timestamps.start.len()
            || timestamps.words.len() != timestamps.end.len()
        {
            self.alignment_active = false;
            return false;
        }

        let mut captions = self.captions.clone();
        let mut cursor = self.text_cursor;
        for ((word, start), end) in timestamps
            .words
            .into_iter()
            .zip(timestamps.start)
            .zip(timestamps.end)
        {
            let Some(relative_start) = captions.text[cursor..].find(&word) else {
                self.alignment_active = false;
                return false;
            };
            let byte_start = cursor + relative_start;
            if word.is_empty()
                || captions.text[cursor..byte_start]
                    .chars()
                    .any(char::is_alphanumeric)
            {
                self.alignment_active = false;
                return false;
            }
            let byte_end = byte_start + word.len();
            captions.words.push(WordTiming {
                text_start: captions.text[..byte_start].encode_utf16().count(),
                text_end: captions.text[..byte_end].encode_utf16().count(),
                start_ms: start * 1000.0,
                end_ms: end * 1000.0,
            });
            cursor = byte_end;
        }
        if captions.validate().is_err() {
            self.alignment_active = false;
            return false;
        }
        self.captions = captions;
        self.text_cursor = cursor;
        true
    }

    fn flush_audio(&mut self, items: &mut Vec<ChunkItem>) {
        if !self.buffered_pcm.is_empty() {
            items.push(ChunkItem::Pcm(std::mem::take(&mut self.buffered_pcm)));
        }
    }
}

/// Voice metadata from the Cartesia `/voices` API.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CartesiaVoice {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
}

impl Default for CartesiaVoice {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            description: None,
            language: None,
        }
    }
}

pub struct CartesiaTtsBackend {
    config: CartesiaConfig,
}

impl CartesiaTtsBackend {
    pub fn new(config: CartesiaConfig) -> Self {
        Self { config }
    }

    fn synthesize_streaming_from(
        &self,
        url: &str,
        text: &str,
        voice: &str,
    ) -> Result<ChunkStream, TtsError> {
        let api_key = crate::secrets::resolve(&self.config.api_key, &["CARTESIA_API_KEY"]);
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Cartesia API key is missing".into()));
        }
        // The shared player and history WAV writer consume signed integer PCM.
        let body = json!({
            "model_id": self.config.model_id,
            "transcript": text,
            "voice": { "mode": "id", "id": voice },
            "output_format": {
                "container": "raw",
                "encoding": "pcm_s16le",
                "sample_rate": 44100,
            },
            "add_timestamps": true,
            "use_normalized_timestamps": false,
        });
        let url = url.to_owned();
        let text = text.to_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            Self::block_on_async(async move {
                let result: Result<(), TtsError> = async {
                    let mut response = Client::new()
                        .post(url)
                        .header("X-API-Key", api_key)
                        .header("Cartesia-Version", CARTESIA_VERSION)
                        .json(&body)
                        .send()
                        .await
                        .map_err(|e| {
                            TtsError::Http(format!("Cartesia stream request failed: {e}"))
                        })?;
                    let status = response.status();
                    if !status.is_success() {
                        let error_text = response.text().await.unwrap_or_default();
                        return Err(TtsError::Http(format!(
                            "Cartesia API error {status}: {error_text}"
                        )));
                    }
                    let mut total_bytes = 0usize;
                    let mut decoder = CartesiaSseDecoder::new(text);
                    while let Some(chunk) = response.chunk().await.map_err(|e| {
                        TtsError::Http(format!(
                            "Cartesia stream read failed after {total_bytes} bytes: {e}"
                        ))
                    })? {
                        if chunk.is_empty() {
                            continue;
                        }
                        total_bytes += chunk.len();
                        for item in decoder.push(&chunk, false).map_err(TtsError::Http)? {
                            if tx.send(item).is_err() {
                                return Ok(());
                            }
                        }
                    }
                    for item in decoder.push(&[], true).map_err(TtsError::Http)? {
                        if tx.send(item).is_err() {
                            return Ok(());
                        }
                    }
                    Ok(())
                }
                .await;
                if let Err(error) = result {
                    log::error!("[TTS] {error}");
                    let _ = tx.send(ChunkItem::Failed(error.to_string()));
                }
            });
        });
        Ok(ChunkStream::new(
            AudioFormatMeta {
                sample_rate: 44100,
                channels: 1,
                bits_per_sample: 16,
            },
            rx,
        ))
    }

    fn block_on_async<F, T>(f: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(f),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(f)
            }
        }
    }

    /// Fetch available voices from the Cartesia API.
    /// GET https://api.cartesia.ai/voices — returns the account's voice library.
    /// On failure the caller falls back to the static catalog list.
    pub fn list_voices(&self) -> Result<Vec<CartesiaVoice>, TtsError> {
        let api_key = crate::secrets::resolve(&self.config.api_key, &["CARTESIA_API_KEY"]);
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Cartesia API key is missing".into()));
        }
        let (status, body) = Self::block_on_async(async move {
            let client = Client::new();
            let response = client
                .get("https://api.cartesia.ai/voices")
                .header("X-API-Key", api_key)
                .header("Cartesia-Version", CARTESIA_VERSION)
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
                "Cartesia API error {}: {}",
                status,
                body.chars().take(300).collect::<String>()
            )));
        }

        serde_json::from_str::<Vec<CartesiaVoice>>(&body)
            .map_err(|e| TtsError::Http(format!("Failed to parse voices response: {}", e)))
    }
}

impl TtsBackend for CartesiaTtsBackend {
    fn name(&self) -> &str {
        "Cartesia"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn synthesize_streaming(&self, text: &str, voice: &str) -> Result<ChunkStream, TtsError> {
        self.synthesize_streaming_from(CARTESIA_TTS_SSE_URL, text, voice)
    }

    fn synthesize_with_captions(
        &self,
        text: &str,
        voice: &str,
    ) -> Result<super::captions::SpeechAudio, TtsError> {
        super::stream::collect_speech(self.synthesize_streaming_from(
            CARTESIA_TTS_SSE_URL,
            text,
            voice,
        )?)
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        let body = json!({
            "model_id": self.config.model_id,
            "transcript": text,
            "voice": {
                "mode": "id",
                "id": voice,
            },
            "output_format": {
                "container": self.config.output_format,
                "encoding": "pcm_f32le",
                "sample_rate": 44100,
            },
        });

        log::info!(
            "Cartesia TTS request - model: {}, voice: {}, text length: {} chars",
            self.config.model_id,
            voice,
            text.len()
        );

        let start_time = std::time::Instant::now();
        let api_key = crate::secrets::resolve(&self.config.api_key, &["CARTESIA_API_KEY"]);

        let response = Self::block_on_async(async {
            let client = Client::new();
            client
                .post(CARTESIA_TTS_URL)
                .header("X-API-Key", api_key)
                .header("Cartesia-Version", CARTESIA_VERSION)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
        })
        .map_err(|e| {
            log::error!(
                "Cartesia TTS request failed after {:?}: {}",
                start_time.elapsed(),
                e
            );
            TtsError::Http(format!("Request failed: {}", e))
        })?;

        let status = response.status();
        log::info!(
            "Cartesia TTS response: {} {} (took {:?})",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            start_time.elapsed()
        );

        if !status.is_success() {
            let error_text =
                Self::block_on_async(async { response.text().await.unwrap_or_default() });
            log::error!("Cartesia API error {}: {}", status, error_text);
            return Err(TtsError::Http(format!(
                "Cartesia API error {}: {}",
                status, error_text
            )));
        }

        let bytes = Self::block_on_async(async { response.bytes().await }).map_err(|e| {
            log::error!("Failed to read Cartesia response bytes: {}", e);
            TtsError::Http(format!("Failed to read bytes: {}", e))
        })?;

        log::info!(
            "Cartesia TTS synthesis complete: received {} bytes",
            bytes.len()
        );
        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        if crate::secrets::resolve(&self.config.api_key, &["CARTESIA_API_KEY"])
            .trim()
            .is_empty()
        {
            return Err(TtsError::Unavailable("Cartesia API key is missing".into()));
        }
        Ok(())
    }

    fn file_extension(&self) -> &str {
        self.config.output_format.as_str()
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        self.config.voice_name.clone().unwrap_or_else(|| {
            // Voice ids are opaque UUIDs — resolve the label from the catalog
            // rather than keeping a second copy of the id/name mapping here.
            crate::tts::catalog::list_static_voices(&crate::config::TtsEngine::Cartesia)
                .into_iter()
                .find(|v| v.id == voice_id)
                .map(|v| v.label)
                .unwrap_or_else(|| "Voice".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

    #[test]
    fn streams_before_response_finishes_and_reports_truncated_body() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/tts/sse", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
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
            assert!(headers.starts_with("post /tts/sse http/1.1"));
            assert!(headers.contains("x-api-key: test-key\r\n"));
            assert!(headers.contains(&format!("cartesia-version: {CARTESIA_VERSION}\r\n")));
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["transcript"], "hello");
            assert_eq!(body["model_id"], "test-model");
            assert_eq!(body["voice"]["id"], "requested-voice");
            assert_eq!(
                body["output_format"],
                json!({
                    "container": "raw", "encoding": "pcm_s16le", "sample_rate": 44100,
                })
            );
            assert_eq!(body["add_timestamps"], true);
            assert_eq!(body["use_normalized_timestamps"], false);
            let response_body = format!(
                "data: {}\n\ndata: {}\n\n",
                json!({ "type": "chunk", "data": "AQI=" }),
                json!({
                    "type": "timestamps",
                    "word_timestamps": { "words": ["hello"], "start": [0.1], "end": [0.4] }
                })
            );
            // Advertise more bytes than we send, to exercise mid-body failure.
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len() + 8,
                response_body
            )
            .unwrap();
            socket.flush().unwrap();
            // Captions and their buffered audio must arrive while the response is open.
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let backend = CartesiaTtsBackend::new(CartesiaConfig {
            api_key: "test-key".into(),
            model_id: "test-model".into(),
            ..Default::default()
        });
        assert!(backend.supports_streaming());
        let stream = backend
            .synthesize_streaming_from(&url, "hello", "requested-voice")
            .unwrap();
        assert_eq!(stream.meta.sample_rate, 44100);
        assert_eq!(stream.meta.channels, 1);
        assert_eq!(stream.meta.bits_per_sample, 16);
        let Some(ChunkItem::Captions(captions)) =
            stream.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("captions must precede their PCM")
        };
        assert_eq!(captions.words[0].text_start, 0);
        assert_eq!(captions.words[0].start_ms, 100.0);
        assert_eq!(
            stream.recv_timeout(Duration::from_secs(5)).unwrap(),
            Some(ChunkItem::Pcm(vec![1, 2]))
        );
        release.send(()).unwrap();
        assert!(
            matches!(stream.recv_timeout(Duration::from_secs(5)).unwrap(),
            Some(ChunkItem::Failed(reason)) if reason.contains("after"))
        );
        assert_eq!(stream.recv_timeout(Duration::from_secs(5)).unwrap(), None);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn streams_complete_body_and_surfaces_api_errors() {
        for status in [200, 401] {
            let server = wiremock::MockServer::start().await;
            let body = if status == 200 {
                format!(
                    "data: {}\n\ndata: {}\n\ndata: {}\n\n",
                    json!({ "type": "chunk", "data": "AQIDBA==" }),
                    json!({
                        "type": "timestamps",
                        "word_timestamps": { "words": ["hello"], "start": [0.0], "end": [0.2] }
                    }),
                    json!({ "type": "done" })
                )
            } else {
                "denied".into()
            };
            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(status).set_body_string(body))
                .mount(&server)
                .await;
            let backend = CartesiaTtsBackend::new(CartesiaConfig {
                api_key: "test-key".into(),
                ..Default::default()
            });
            let stream = backend
                .synthesize_streaming_from(&server.uri(), "hello", "voice")
                .unwrap();
            tokio::task::spawn_blocking(move || {
                let mut pcm = Vec::new();
                let mut captions = None;
                let mut failed = false;
                while let Some(item) = stream.recv_timeout(Duration::from_secs(5)).unwrap() {
                    match item {
                        ChunkItem::Pcm(bytes) => pcm.extend(bytes),
                        ChunkItem::Captions(value) => captions = Some(value),
                        ChunkItem::Failed(reason) => {
                            assert!(reason.contains("401"));
                            failed = true;
                        }
                    }
                }
                if status == 200 {
                    assert!(!failed);
                    assert_eq!(pcm, vec![1, 2, 3, 4]);
                    assert_eq!(captions.unwrap().words[0].end_ms, 200.0);
                } else {
                    assert!(failed);
                    assert!(pcm.is_empty());
                }
            })
            .await
            .unwrap();
        }
    }

    #[test]
    fn decoder_handles_transport_splits_and_maps_original_utf16_text() {
        let wire = format!(
            "data: {}\r\n\r\ndata: {}\r\n\r\ndata: {}\r\n\r\ndata: {}\r\n\r\ndata: {}",
            json!({ "type": "chunk", "data": "AQI=" }),
            json!({
                "type": "timestamps",
                "word_timestamps": { "words": ["Hello"], "start": [0.1], "end": [0.4] }
            }),
            json!({ "type": "chunk", "data": "AwQ=" }),
            json!({
                "type": "timestamps",
                "word_timestamps": { "words": ["world"], "start": [0.5], "end": [0.9] }
            }),
            json!({ "type": "done" })
        );
        for split in 0..wire.len() {
            let mut decoder = CartesiaSseDecoder::new("😀 Hello world".into());
            let mut items = decoder.push(&wire.as_bytes()[..split], false).unwrap();
            items.extend(decoder.push(&wire.as_bytes()[split..], true).unwrap());
            let ChunkItem::Captions(first) = &items[0] else {
                panic!("captions must precede PCM")
            };
            assert_eq!((first.words[0].text_start, first.words[0].text_end), (3, 8));
            assert_eq!(items[1], ChunkItem::Pcm(vec![1, 2]));
            let ChunkItem::Captions(second) = &items[2] else {
                panic!("second snapshot missing")
            };
            assert_eq!(
                (second.words[1].text_start, second.words[1].text_end),
                (9, 14)
            );
            assert_eq!(items[3], ChunkItem::Pcm(vec![3, 4]));
        }
    }

    #[test]
    fn unmappable_native_words_leave_audio_playable_without_fabricated_timing() {
        let wire = format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\n",
            json!({ "type": "chunk", "data": "AQI=" }),
            json!({
                "type": "timestamps",
                "word_timestamps": { "words": ["thirteen"], "start": [0.0], "end": [0.5] }
            }),
            json!({ "type": "done" })
        );
        assert_eq!(
            CartesiaSseDecoder::new("13".into())
                .push(wire.as_bytes(), true)
                .unwrap(),
            vec![ChunkItem::Pcm(vec![1, 2])]
        );
    }
}
