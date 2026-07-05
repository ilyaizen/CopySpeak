use super::{TtsBackend, TtsError};
use crate::config::CartesiaConfig;
use reqwest::Client;
use serde_json::json;

const CARTESIA_TTS_URL: &str = "https://api.cartesia.ai/tts/bytes";
const CARTESIA_VERSION: &str = "2024-06-10";

pub struct CartesiaTtsBackend {
    config: CartesiaConfig,
}

impl CartesiaTtsBackend {
    pub fn new(config: CartesiaConfig) -> Self {
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
}

impl TtsBackend for CartesiaTtsBackend {
    fn name(&self) -> &str {
        "Cartesia"
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
        let api_key = self.config.api_key.clone();

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
        if self.config.api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Cartesia API key is missing".into()));
        }
        Ok(())
    }

    fn file_extension(&self) -> &str {
        self.config.output_format.as_str()
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        self.config
            .voice_name
            .clone()
            .unwrap_or_else(|| match voice_id {
                "f786b574-daa5-4673-aa0c-cbe3e8534c02" => "Katie".to_string(),
                "a5136bf9-224c-4d76-b823-52bd5efcffcc" => "Jameson".to_string(),
                _ => "Voice".to_string(),
            })
    }
}
