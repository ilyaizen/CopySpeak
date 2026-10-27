use super::block_on_async;
use super::{TtsBackend, TtsError, Voice};
use crate::config::MistralConfig;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const MISTRAL_SPEECH_URL: &str = "https://api.mistral.ai/v1/audio/speech";
const MISTRAL_VOICES_URL: &str = "https://api.mistral.ai/v2/audio/voices";

/// The current offline (batch) Voxtral TTS model, verified against
/// docs.mistral.ai (2026-10-25). The realtime variant is a different model;
/// v1 is batch-only and carries no model knob. No Hebrew among its 20+
/// languages.
pub const MISTRAL_TTS_MODEL: &str = "voxtral-mini-tts-3-0";

pub struct MistralTtsBackend {
    config: MistralConfig,
}

impl MistralTtsBackend {
    pub fn new(config: MistralConfig) -> Self {
        Self { config }
    }

    /// Resolve the API key: config first, then the `MISTRAL_API_KEY` env var.
    fn api_key(&self) -> String {
        crate::secrets::resolve(&self.config.api_key, &["MISTRAL_API_KEY"])
    }

    /// The batch request body. `voice_id` is the only selection knob (cloned
    /// voices are covered by manual ID entry; v1 wires no ref_audio cloning)
    /// and is omitted when empty — the service then picks its own default.
    /// Output container is pinned to WAV (v1 decision). No timing endpoint
    /// exists, so the request carries no captions hooks.
    fn request_body(&self, text: &str) -> serde_json::Value {
        let mut body = json!({
            "model": MISTRAL_TTS_MODEL,
            "input": text,
            "response_format": "wav",
        });
        let voice = self.config.voice_id.trim();
        if !voice.is_empty() {
            body["voice_id"] = json!(voice);
        }
        body
    }

    fn synthesize_full(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Mistral API key is missing".into()));
        }

        let body = self.request_body(text);

        log::info!(
            "Mistral TTS request - model: {}, voice: {}, text length: {} chars",
            MISTRAL_TTS_MODEL,
            if self.config.voice_id.trim().is_empty() {
                "(service default)"
            } else {
                &self.config.voice_id
            },
            text.len()
        );

        if crate::logging::is_debug_mode() {
            log::debug!(
                "Mistral TTS request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let start_time = std::time::Instant::now();

        // Request AND body read inside one block_on_async call: with no
        // ambient runtime each call builds a fresh runtime, and a Response
        // carried across calls loses its connection driver.
        let bytes = block_on_async(async move {
            let client = Client::new();
            let response = client
                .post(MISTRAL_SPEECH_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    TtsError::Http(format!(
                        "Request failed: {}",
                        super::http::flatten_error(&e)
                    ))
                })?;

            let status = response.status();
            log::info!(
                "Mistral TTS response: {} {} (took {:?})",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                start_time.elapsed()
            );

            let body_text = response.text().await.map_err(|e| {
                TtsError::Http(format!(
                    "Failed to read response body: {}",
                    super::http::flatten_error(&e)
                ))
            })?;

            if !status.is_success() {
                log::error!("Mistral API error {}: {}", status, body_text);
                return Err(TtsError::Http(format!(
                    "Mistral API error {}: {}",
                    status, body_text
                )));
            }

            // Raw JSON field per the API reference: {"audio_data": "<base64>"}.
            #[derive(Deserialize)]
            struct SpeechEnvelope {
                audio_data: String,
            }
            let envelope: SpeechEnvelope = serde_json::from_str(&body_text).map_err(|e| {
                TtsError::Http(format!("Failed to parse Mistral speech response: {e}"))
            })?;
            let bytes = STANDARD
                .decode(envelope.audio_data.as_bytes())
                .map_err(|e| TtsError::Http(format!("Invalid Mistral audio payload: {e}")))?;
            log::info!(
                "Mistral TTS synthesis complete: received {} bytes",
                bytes.len()
            );
            Ok::<_, TtsError>(bytes)
        })?;

        Ok(bytes)
    }

    /// List voices (presets + the workspace's saved ones) from the v2
    /// cursor-paginated endpoint. Cloned/saved voices work by manual ID entry
    /// even when not listed under this account.
    pub fn list_voices(&self) -> Result<Vec<Voice>, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Mistral API key is missing".into()));
        }

        let mut voices = Vec::new();
        let mut next_token: Option<String> = None;
        // Cursor pagination: the docs deprecate offset pagination. Bound the
        // walk defensively — a hostile token loop must not hang the UI.
        for _page in 0..20 {
            let page_token = next_token.clone();
            let page_key = api_key.clone();
            let (status, body) = block_on_async(async move {
                let client = Client::new();
                let mut request = client
                    .get(MISTRAL_VOICES_URL)
                    .bearer_auth(page_key)
                    .query(&[("page_size", "100")]);
                if let Some(token) = page_token {
                    request = request.query(&[("next_page_token", token)]);
                }
                let response = request
                    .header("Accept", "application/json")
                    .send()
                    .await
                    .map_err(|e| {
                        TtsError::Http(format!(
                            "Failed to fetch voices: {}",
                            super::http::flatten_error(&e)
                        ))
                    })?;
                let status = response.status();
                let body = response.text().await.map_err(|e| {
                    TtsError::Http(format!(
                        "Failed to read voices response: {}",
                        super::http::flatten_error(&e)
                    ))
                })?;
                Ok::<_, TtsError>((status, body))
            })?;

            if !status.is_success() {
                return Err(TtsError::Http(format!(
                    "Mistral API error {}: {}",
                    status,
                    body.chars().take(300).collect::<String>()
                )));
            }

            #[derive(Deserialize)]
            struct VoicePage {
                #[serde(default)]
                data: Vec<VoiceEntry>,
                #[serde(default)]
                next_page_token: Option<String>,
            }
            #[derive(Deserialize)]
            struct VoiceEntry {
                id: String,
                /// Preset voices carry a slug (`voice_478`, `gb_jane_neutral`);
                /// that slug is the `voice_id` speech requests accept. Saved
                /// voices have none: their `id` is the request key.
                #[serde(default)]
                slug: Option<String>,
                #[serde(default)]
                name: Option<String>,
                #[serde(default)]
                languages: Vec<String>,
            }

            let page: VoicePage = serde_json::from_str(&body)
                .map_err(|e| TtsError::Http(format!("Failed to parse voices response: {}", e)))?;
            for entry in page.data {
                let id = entry
                    .slug
                    .filter(|slug| !slug.trim().is_empty())
                    .unwrap_or(entry.id.clone());
                voices.push(Voice {
                    id,
                    name: entry.name.unwrap_or(entry.id),
                    language: entry.languages.into_iter().next(),
                    default: None,
                });
            }
            next_token = page.next_page_token.filter(|t| !t.trim().is_empty());
            if next_token.is_none() {
                break;
            }
        }
        Ok(voices)
    }
}

impl TtsBackend for MistralTtsBackend {
    fn name(&self) -> &str {
        "Mistral"
    }

    fn synthesize(&self, text: &str, _voice: &str) -> Result<Vec<u8>, TtsError> {
        self.synthesize_full(text)
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("Mistral TTS health check - validating API key");

        if self.api_key().trim().is_empty() {
            log::error!("Mistral TTS health check failed - API key is missing");
            return Err(TtsError::Unavailable("Mistral API key is missing".into()));
        }

        log::debug!("Mistral TTS health check passed");
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "wav"
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(voice_id: &str) -> MistralTtsBackend {
        MistralTtsBackend::new(MistralConfig {
            voice_id: voice_id.into(),
            ..Default::default()
        })
    }

    #[test]
    fn body_pins_model_input_and_wav() {
        let body = backend("voice_478").request_body("hello");
        assert_eq!(body["model"], MISTRAL_TTS_MODEL);
        assert_eq!(body["input"], "hello");
        assert_eq!(body["response_format"], "wav");
        assert_eq!(body["voice_id"], "voice_478");
    }

    #[test]
    fn body_omits_voice_id_for_the_service_default() {
        let body = backend("").request_body("hello");
        assert!(body.get("voice_id").is_none());
    }

    /// Live synthesis against api.mistral.ai on a preset voice. Opt-in: needs
    /// `MISTRAL_API_KEY` in the environment (e.g. exported from
    /// `target/debug/.env`); runs only via `cargo test mistral_live -- --ignored`.
    /// ~300 chars (~$0.005 at $16/1M chars).
    #[test]
    #[ignore = "live API test; needs MISTRAL_API_KEY in the environment"]
    fn mistral_live_synthesizes_a_preset_voice() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(api_key) = std::env::var("MISTRAL_API_KEY") else {
            eprintln!("MISTRAL_API_KEY not set; skipping live synthesis test");
            return Ok(());
        };
        const TEXT: &str = "Mistral live check. CopySpeak reads this short paragraph to verify \
            that the Voxtral batch endpoint returns a decodable WAV container with real \
            speech inside it. The quick brown fox jumps over the lazy dog while the \
            evening sun settles behind the hills and the town lights begin to glow one \
            by one across the valley floor, just as they did in quieter years.";
        debug_assert!(TEXT.chars().count() < 400);
        let backend = MistralTtsBackend::new(MistralConfig {
            api_key,
            voice_id: "voice_478".into(),
            ..Default::default()
        });
        let bytes = backend.synthesize(TEXT, "voice_478")?;
        assert!(
            bytes.len() > 10_000,
            "suspiciously small audio: {}",
            bytes.len()
        );
        assert_eq!(&bytes[..4], b"RIFF", "expected a WAV container");
        Ok(())
    }
}
