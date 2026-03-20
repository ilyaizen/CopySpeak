use super::{TtsBackend, TtsError};
use crate::config::OpenAIConfig;
use reqwest::Client;
use serde_json::json;

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
                let rt = tokio::runtime::Runtime::new()
                    .expect("Failed to create Tokio runtime");
                rt.block_on(f)
            }
        }
    }
}

impl TtsBackend for OpenAiTtsBackend {
    fn name(&self) -> &str {
        "OpenAI"
    }

    fn synthesize(&self, text: &str, _voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        let url = "https://api.openai.com/v1/audio/speech";

        let body = json!({
            "model": self.config.model,
            "input": text,
            "voice": self.config.voice,
            "speed": speed,
            "response_format": "wav",
        });

        let api_key = self.config.api_key.clone();

        // Log request details
        log::info!("OpenAI TTS request - model: {}, voice: {}, speed: {}, text length: {} chars",
            self.config.model, self.config.voice, speed, text.len());

        if crate::logging::is_debug_mode() {
            log::debug!("OpenAI TTS request body: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
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
        }).map_err(|e| {
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
            let error_text = Self::block_on_async(async {
                response.text().await.unwrap_or_default()
            });
            log::error!(
                "OpenAI API error {}: {}",
                status, error_text
            );
            return Err(TtsError::Http(format!(
                "OpenAI API error {}: {}",
                status, error_text
            )));
        }

        let bytes = Self::block_on_async(async {
            response.bytes().await
        }).map_err(|e| {
            log::error!("Failed to read response bytes: {}", e);
            TtsError::Http(format!("Failed to read bytes: {}", e))
        })?;

        log::info!("OpenAI TTS synthesis complete: received {} bytes", bytes.len());

        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("OpenAI TTS health check - validating API key");

        if self.config.api_key.trim().is_empty() {
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
