// Microsoft MAI-Voice-2 backend (native Rust HTTP).
//
// MAI-Voice-2 is served from Azure AI Foundry / microsoft.ai with deployment-
// specific endpoints, so the endpoint is fully user-configurable. The request
// shape follows the common OpenAI-compatible audio/speech contract
// ({ model, input, voice }); the response is auto-detected as either raw audio
// bytes or a JSON envelope carrying base64 audio.
//
// NOTE: exact auth header / body / response for MAI-Voice-2 should be confirmed
// against current Foundry docs. The HTTP backend (engine "http") is the escape
// hatch when a deployment diverges from this default contract.

use super::{TtsBackend, TtsError};
use crate::config::MicrosoftTtsConfig;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;

pub struct MicrosoftTtsBackend {
    config: MicrosoftTtsConfig,
}

impl MicrosoftTtsBackend {
    pub fn new(config: MicrosoftTtsConfig) -> Self {
        Self { config }
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

    /// Pull base64 audio out of a JSON envelope, trying common field names.
    fn audio_from_json(value: &serde_json::Value) -> Option<Vec<u8>> {
        for ptr in ["/audio", "/data", "/audioContent", "/result/audio"] {
            if let Some(s) = value.pointer(ptr).and_then(|v| v.as_str()) {
                if let Ok(bytes) = general_purpose::STANDARD.decode(s) {
                    return Some(bytes);
                }
            }
        }
        None
    }
}

impl TtsBackend for MicrosoftTtsBackend {
    fn name(&self) -> &str {
        "Microsoft MAI-Voice-2"
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        if self.config.endpoint.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "Microsoft endpoint is not configured".into(),
            ));
        }

        let body = json!({
            "model": self.config.model,
            "input": text,
            "voice": voice,
            "response_format": self.config.output_format,
        });

        log::info!(
            "Microsoft TTS request - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        let start_time = std::time::Instant::now();
        let api_key = self.config.api_key.clone();
        let endpoint = self.config.endpoint.clone();

        let response = Self::block_on_async(async {
            let client = Client::new();
            client
                .post(&endpoint)
                .header("api-key", &api_key)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
        })
        .map_err(|e| {
            log::error!(
                "Microsoft TTS request failed after {:?}: {}",
                start_time.elapsed(),
                e
            );
            TtsError::Http(format!("Request failed: {}", e))
        })?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        log::info!(
            "Microsoft TTS response: {} ({}), took {:?}",
            status.as_u16(),
            content_type,
            start_time.elapsed()
        );

        let bytes = Self::block_on_async(async { response.bytes().await })
            .map_err(|e| TtsError::Http(format!("Failed to read bytes: {}", e)))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&bytes);
            log::error!("Microsoft API error {}: {}", status, error_text);
            return Err(TtsError::Http(format!(
                "Microsoft API error {}: {}",
                status, error_text
            )));
        }

        // JSON envelope → decode base64; otherwise treat as raw audio bytes.
        if content_type.contains("application/json") {
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| TtsError::Http(format!("Invalid JSON response: {}", e)))?;
            return Self::audio_from_json(&value)
                .ok_or_else(|| TtsError::Http("No audio field in JSON response".into()));
        }

        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        if self.config.api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Microsoft API key is missing".into()));
        }
        if self.config.endpoint.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "Microsoft endpoint is missing".into(),
            ));
        }
        Ok(())
    }

    fn file_extension(&self) -> &str {
        match self.config.output_format.as_str() {
            "" => "wav",
            other => other,
        }
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}
