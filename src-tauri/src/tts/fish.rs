use super::block_on_async;
use super::{TtsBackend, TtsError, Voice};
use crate::config::FishConfig;
use reqwest::Client;
use serde_json::json;

const FISH_TTS_URL: &str = "https://api.fish.audio/v1/tts";
const FISH_MODEL_LIST_URL: &str = "https://api.fish.audio/model";

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

    fn request_body(&self, text: &str) -> serde_json::Value {
        let mut body = json!({
            "text": text,
            "format": self.config.format,
        });
        // Empty voice = Fish Audio's built-in default voice: `reference_id`
        // must be omitted, not sent blank.
        let voice = self.config.voice_id.trim();
        if !voice.is_empty() {
            body["reference_id"] = json!(voice);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
