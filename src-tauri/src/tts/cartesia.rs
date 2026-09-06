use super::stream::{AudioFormatMeta, ChunkItem, ChunkStream};
use super::{TtsBackend, TtsError};
use crate::config::CartesiaConfig;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const CARTESIA_TTS_URL: &str = "https://api.cartesia.ai/tts/bytes";
const CARTESIA_VERSION: &str = "2024-06-10";

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
        });
        let url = url.to_owned();
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
                        .map_err(|e| TtsError::Http(format!("Cartesia stream request failed: {e}")))?;
                    let status = response.status();
                    if !status.is_success() {
                        let error_text = response.text().await.unwrap_or_default();
                        return Err(TtsError::Http(format!("Cartesia API error {status}: {error_text}")));
                    }
                    let mut total_bytes = 0usize;
                    while let Some(chunk) = response.chunk().await.map_err(|e| {
                        TtsError::Http(format!("Cartesia stream read failed after {total_bytes} bytes: {e}"))
                    })? {
                        if chunk.is_empty() {
                            continue;
                        }
                        total_bytes += chunk.len();
                        if tx.send(ChunkItem::Pcm(chunk.to_vec())).is_err() {
                            return Ok(());
                        }
                    }
                    Ok(())
                }.await;
                if let Err(error) = result {
                    log::error!("[TTS] {error}");
                    let _ = tx.send(ChunkItem::Failed(error.to_string()));
                }
            });
        });
        Ok(ChunkStream::new(AudioFormatMeta {
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
        }, rx))
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
        self.synthesize_streaming_from(CARTESIA_TTS_URL, text, voice)
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
        let url = format!("http://{}/tts/bytes", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request).unwrap().to_lowercase();
            assert!(headers.starts_with("post /tts/bytes http/1.1"));
            assert!(headers.contains("x-api-key: test-key\r\n"));
            assert!(headers.contains(&format!("cartesia-version: {CARTESIA_VERSION}\r\n")));
            let length: usize = headers.lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap().parse().unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["transcript"], "hello");
            assert_eq!(body["model_id"], "test-model");
            assert_eq!(body["voice"]["id"], "requested-voice");
            assert_eq!(body["output_format"], json!({
                "container": "raw", "encoding": "pcm_s16le", "sample_rate": 44100,
            }));
            // Advertise more bytes than we send, to exercise mid-body failure.
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n\x01\x02").unwrap();
            socket.flush().unwrap();
            // The client must receive audio while this response is still open.
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let backend = CartesiaTtsBackend::new(CartesiaConfig {
            api_key: "test-key".into(),
            model_id: "test-model".into(),
            ..Default::default()
        });
        assert!(backend.supports_streaming());
        let stream = backend.synthesize_streaming_from(&url, "hello", "requested-voice").unwrap();
        assert_eq!(stream.meta.sample_rate, 44100);
        assert_eq!(stream.meta.channels, 1);
        assert_eq!(stream.meta.bits_per_sample, 16);
        let mut pcm = Vec::new();
        while pcm.len() < 2 {
            match stream.recv_timeout(Duration::from_secs(5)).unwrap() {
                Some(ChunkItem::Pcm(bytes)) => pcm.extend(bytes),
                other => panic!("expected early PCM, got {other:?}"),
            }
        }
        assert_eq!(pcm, vec![1, 2]);
        release.send(()).unwrap();
        assert!(matches!(stream.recv_timeout(Duration::from_secs(5)).unwrap(),
            Some(ChunkItem::Failed(reason)) if reason.contains("after 2 bytes")));
        assert_eq!(stream.recv_timeout(Duration::from_secs(5)).unwrap(), None);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn streams_complete_body_and_surfaces_api_errors() {
        for status in [200, 401] {
            let server = wiremock::MockServer::start().await;
            wiremock::Mock::given(wiremock::matchers::method("POST"))
                .respond_with(wiremock::ResponseTemplate::new(status).set_body_bytes(vec![1, 2, 3, 4]))
                .mount(&server).await;
            let backend = CartesiaTtsBackend::new(CartesiaConfig {
                api_key: "test-key".into(),
                ..Default::default()
            });
            let stream = backend.synthesize_streaming_from(&server.uri(), "hello", "voice").unwrap();
            tokio::task::spawn_blocking(move || {
                let mut pcm = Vec::new();
                let mut failed = false;
                while let Some(item) = stream.recv_timeout(Duration::from_secs(5)).unwrap() {
                    match item {
                        ChunkItem::Pcm(bytes) => pcm.extend(bytes),
                        ChunkItem::Failed(reason) => {
                            assert!(reason.contains("401"));
                            failed = true;
                        }
                    }
                }
                if status == 200 {
                    assert!(!failed);
                    assert_eq!(pcm, vec![1, 2, 3, 4]);
                } else {
                    assert!(failed);
                    assert!(pcm.is_empty());
                }
            }).await.unwrap();
        }
    }
}
