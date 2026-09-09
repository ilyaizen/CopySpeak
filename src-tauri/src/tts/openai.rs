use super::stream::{AudioFormatMeta, ChunkItem, ChunkStream};
use super::{TtsBackend, TtsError};
use crate::config::OpenAIConfig;
use reqwest::Client;
use serde_json::json;

const OPENAI_TTS_URL: &str = "https://api.openai.com/v1/audio/speech";

/// `response_format: "pcm"` is documented as 24 kHz, 16-bit signed
/// little-endian, mono — exactly what the player and the history WAV writer
/// consume, so the streaming path needs no decode step at all.
const OPENAI_PCM_SAMPLE_RATE: u32 = 24000;

pub struct OpenAiTtsBackend {
    config: OpenAIConfig,
}

impl OpenAiTtsBackend {
    pub fn new(config: OpenAIConfig) -> Self {
        Self { config }
    }

    /// Execute an async block using the current Tokio runtime if available,
    /// or create a new one if called from outside a runtime context.
    fn block_on_async<F, T>(f: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        // Try to get the current runtime handle
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // We're in a Tokio runtime context, use block_on
                handle.block_on(f)
            }
            Err(_) => {
                // No runtime context, create a new one
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(f)
            }
        }
    }
}

impl TtsBackend for OpenAiTtsBackend {
    fn name(&self) -> &str {
        "OpenAI"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn synthesize_streaming(&self, text: &str, voice: &str) -> Result<ChunkStream, TtsError> {
        let api_key = crate::secrets::resolve(&self.config.api_key, &["OPENAI_API_KEY"]);
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("OpenAI API key is missing".into()));
        }
        // Raw PCM rather than the configured container: /v1/audio/speech is
        // chunk-transfer encoded, and a container would only be parseable once
        // the whole body had arrived. The batch path below keeps the user's
        // format, which is what file output and the cache want.
        let mut body = json!({
            "model": self.config.model,
            "input": text,
            "voice": voice,
            "response_format": "pcm",
        });
        if let Some(instructions) = self
            .config
            .instructions
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            body["instructions"] = json!(instructions);
        }

        log::info!(
            "OpenAI TTS stream - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            Self::block_on_async(async move {
                let result: Result<(), TtsError> = async {
                    let mut response = Client::new()
                        .post(OPENAI_TTS_URL)
                        .header("Authorization", format!("Bearer {api_key}"))
                        .json(&body)
                        .send()
                        .await
                        .map_err(|e| {
                            TtsError::Http(format!("OpenAI stream request failed: {e}"))
                        })?;
                    let status = response.status();
                    if !status.is_success() {
                        let error_text = response.text().await.unwrap_or_default();
                        return Err(TtsError::Http(format!(
                            "OpenAI API error {status}: {error_text}"
                        )));
                    }
                    let mut total_bytes = 0usize;
                    while let Some(chunk) = response.chunk().await.map_err(|e| {
                        TtsError::Http(format!(
                            "OpenAI stream read failed after {total_bytes} bytes: {e}"
                        ))
                    })? {
                        if chunk.is_empty() {
                            continue;
                        }
                        total_bytes += chunk.len();
                        if tx.send(ChunkItem::Pcm(chunk.to_vec())).is_err() {
                            // Consumer dropped (stop/abort): stop pulling the body.
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
                sample_rate: OPENAI_PCM_SAMPLE_RATE,
                channels: 1,
                bits_per_sample: 16,
            },
            rx,
        ))
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        let url = OPENAI_TTS_URL;

        let mut body = json!({
            "model": self.config.model,
            "input": text,
            "voice": voice,
            "response_format": self.config.response_format,
        });
        if let Some(instructions) = self
            .config
            .instructions
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            body["instructions"] = json!(instructions);
        }

        let api_key = crate::secrets::resolve(&self.config.api_key, &["OPENAI_API_KEY"]);

        // Log request details
        log::info!(
            "OpenAI TTS request - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        if crate::logging::is_debug_mode() {
            log::debug!(
                "OpenAI TTS request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let start_time = std::time::Instant::now();

        let response = Self::block_on_async(async {
            let client = Client::new();
            client
                .post(url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
        })
        .map_err(|e| {
            let elapsed = start_time.elapsed();
            log::error!("OpenAI TTS request failed after {:?}: {}", elapsed, e);
            TtsError::Http(format!("Request failed: {}", e))
        })?;

        let elapsed = start_time.elapsed();
        let status = response.status();

        // Log response status and timing
        log::info!(
            "OpenAI TTS response: {} {} (took {:?})",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            elapsed
        );

        if !response.status().is_success() {
            let error_text =
                Self::block_on_async(async { response.text().await.unwrap_or_default() });
            log::error!("OpenAI API error {}: {}", status, error_text);
            return Err(TtsError::Http(format!(
                "OpenAI API error {}: {}",
                status, error_text
            )));
        }

        let bytes = Self::block_on_async(async { response.bytes().await }).map_err(|e| {
            log::error!("Failed to read response bytes: {}", e);
            TtsError::Http(format!("Failed to read bytes: {}", e))
        })?;

        log::info!(
            "OpenAI TTS synthesis complete: received {} bytes",
            bytes.len()
        );

        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("OpenAI TTS health check - validating API key");

        if crate::secrets::resolve(&self.config.api_key, &["OPENAI_API_KEY"])
            .trim()
            .is_empty()
        {
            log::error!("OpenAI TTS health check failed - API key is missing");
            return Err(TtsError::Unavailable("OpenAI API key is missing".into()));
        }

        log::debug!("OpenAI TTS health check passed");
        Ok(())
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}
