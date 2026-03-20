use super::{TtsBackend, TtsError, Voice};
use crate::config::ElevenLabsConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Available output formats for ElevenLabs API
/// https://elevenlabs.io/docs/api-reference/text-to-speech
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[allow(non_camel_case_types)]
pub enum ElevenLabsOutputFormat {
    /// MP3 format, 44.1kHz, 128kbps (default, good quality)
    #[serde(rename = "mp3_44100_128")]
    Mp3_44100_128,
    /// MP3 format, 44.1kHz, 192kbps (higher quality)
    #[serde(rename = "mp3_44100_192")]
    Mp3_44100_192,
    /// MP3 format, 44.1kHz, 32kbps (smaller file)
    #[serde(rename = "mp3_44100_32")]
    Mp3_44100_32,
    /// MP3 format, 22.05kHz, 32kbps (lower quality)
    #[serde(rename = "mp3_22050_32")]
    Mp3_22050_32,
    /// PCM format, 16-bit, 44.1kHz (uncompressed)
    #[serde(rename = "pcm_44100")]
    Pcm_44100,
    /// PCM format, 16-bit, 22.05kHz
    #[serde(rename = "pcm_22050")]
    Pcm_22050,
    /// PCM format, 16-bit, 16kHz
    #[serde(rename = "pcm_16000")]
    Pcm_16000,
    /// OGG format, Opus codec
    #[serde(rename = "ogg_vorbis_44100")]
    OggVorbis_44100,
    /// OGG format, Opus codec (lower quality)
    #[serde(rename = "ogg_vorbis_22050")]
    OggVorbis_22050,
    /// FLAC format, lossless compression
    #[serde(rename = "flac_44100")]
    Flac_44100,
    /// MULAW format, 8-bit, 8kHz (for telephony)
    #[serde(rename = "mulaw_8000")]
    Mulaw_8000,
}

impl Default for ElevenLabsOutputFormat {
    fn default() -> Self {
        ElevenLabsOutputFormat::Mp3_44100_128
    }
}

impl ElevenLabsOutputFormat {
    /// Get the format identifier string for API requests
    pub fn as_str(&self) -> &'static str {
        match self {
            ElevenLabsOutputFormat::Mp3_44100_128 => "mp3_44100_128",
            ElevenLabsOutputFormat::Mp3_44100_192 => "mp3_44100_192",
            ElevenLabsOutputFormat::Mp3_44100_32 => "mp3_44100_32",
            ElevenLabsOutputFormat::Mp3_22050_32 => "mp3_22050_32",
            ElevenLabsOutputFormat::Pcm_44100 => "pcm_44100",
            ElevenLabsOutputFormat::Pcm_22050 => "pcm_22050",
            ElevenLabsOutputFormat::Pcm_16000 => "pcm_16000",
            ElevenLabsOutputFormat::OggVorbis_44100 => "ogg_vorbis_44100",
            ElevenLabsOutputFormat::OggVorbis_22050 => "ogg_vorbis_22050",
            ElevenLabsOutputFormat::Flac_44100 => "flac_44100",
            ElevenLabsOutputFormat::Mulaw_8000 => "mulaw_8000",
        }
    }

    /// Get MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            ElevenLabsOutputFormat::Mp3_44100_128
            | ElevenLabsOutputFormat::Mp3_44100_192
            | ElevenLabsOutputFormat::Mp3_44100_32
            | ElevenLabsOutputFormat::Mp3_22050_32 => "audio/mpeg",
            ElevenLabsOutputFormat::Pcm_44100
            | ElevenLabsOutputFormat::Pcm_22050
            | ElevenLabsOutputFormat::Pcm_16000 => "audio/pcm",
            ElevenLabsOutputFormat::OggVorbis_44100 | ElevenLabsOutputFormat::OggVorbis_22050 => {
                "audio/ogg"
            }
            ElevenLabsOutputFormat::Flac_44100 => "audio/flac",
            ElevenLabsOutputFormat::Mulaw_8000 => "audio/mulaw",
        }
    }

    /// Check if this format can be decoded by rodio
    #[allow(dead_code)]
    pub fn is_playable_by_rodio(&self) -> bool {
        match self {
            // MP3, WAV (PCM), FLAC, and OGG Vorbis are supported by rodio
            ElevenLabsOutputFormat::Mp3_44100_128
            | ElevenLabsOutputFormat::Mp3_44100_192
            | ElevenLabsOutputFormat::Mp3_44100_32
            | ElevenLabsOutputFormat::Mp3_22050_32
            | ElevenLabsOutputFormat::Pcm_44100
            | ElevenLabsOutputFormat::Pcm_22050
            | ElevenLabsOutputFormat::Pcm_16000
            | ElevenLabsOutputFormat::OggVorbis_44100
            | ElevenLabsOutputFormat::OggVorbis_22050
            | ElevenLabsOutputFormat::Flac_44100 => true,
            // MULAW is not directly supported by rodio
            ElevenLabsOutputFormat::Mulaw_8000 => false,
        }
    }

    /// Get all available formats
    pub fn all() -> &'static [ElevenLabsOutputFormat] {
        &[
            ElevenLabsOutputFormat::Mp3_44100_128,
            ElevenLabsOutputFormat::Mp3_44100_192,
            ElevenLabsOutputFormat::Mp3_44100_32,
            ElevenLabsOutputFormat::Mp3_22050_32,
            ElevenLabsOutputFormat::Pcm_44100,
            ElevenLabsOutputFormat::Pcm_22050,
            ElevenLabsOutputFormat::Pcm_16000,
            ElevenLabsOutputFormat::OggVorbis_44100,
            ElevenLabsOutputFormat::OggVorbis_22050,
            ElevenLabsOutputFormat::Flac_44100,
            ElevenLabsOutputFormat::Mulaw_8000,
        ]
    }

    /// Get human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            ElevenLabsOutputFormat::Mp3_44100_128 => "MP3 44.1kHz 128kbps (Recommended)",
            ElevenLabsOutputFormat::Mp3_44100_192 => "MP3 44.1kHz 192kbps (High Quality)",
            ElevenLabsOutputFormat::Mp3_44100_32 => "MP3 44.1kHz 32kbps (Small Size)",
            ElevenLabsOutputFormat::Mp3_22050_32 => "MP3 22.05kHz 32kbps (Low Quality)",
            ElevenLabsOutputFormat::Pcm_44100 => "PCM 44.1kHz 16-bit (Uncompressed)",
            ElevenLabsOutputFormat::Pcm_22050 => "PCM 22.05kHz 16-bit",
            ElevenLabsOutputFormat::Pcm_16000 => "PCM 16kHz 16-bit",
            ElevenLabsOutputFormat::OggVorbis_44100 => "OGG Vorbis 44.1kHz",
            ElevenLabsOutputFormat::OggVorbis_22050 => "OGG Vorbis 22.05kHz",
            ElevenLabsOutputFormat::Flac_44100 => "FLAC 44.1kHz (Lossless)",
            ElevenLabsOutputFormat::Mulaw_8000 => "MULAW 8kHz (Telephony)",
        }
    }
}

/// Voice information from ElevenLabs API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ElevenLabsVoice {
    pub voice_id: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub labels: Option<serde_json::Value>,
    pub description: Option<String>,
    pub preview_url: Option<String>,
}

impl Default for ElevenLabsVoice {
    fn default() -> Self {
        Self {
            voice_id: String::new(),
            name: None,
            category: None,
            labels: None,
            description: None,
            preview_url: None,
        }
    }
}

/// Voice settings for ElevenLabs TTS
#[derive(Debug, Clone, Serialize)]
pub struct VoiceSettings {
    pub stability: f32,
    pub similarity_boost: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_speaker_boost: Option<bool>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            stability: 0.5,
            similarity_boost: 0.75,
            style: None,
            use_speaker_boost: None,
        }
    }
}

pub struct ElevenLabsTtsBackend {
    config: ElevenLabsConfig,
}

impl ElevenLabsTtsBackend {
    pub fn new(config: ElevenLabsConfig) -> Self {
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

    /// Fetch available voices from ElevenLabs API
    /// https://elevenlabs.io/docs/api-reference/get-voices
    pub fn list_voices(&self) -> Result<Vec<ElevenLabsVoice>, TtsError> {
        log::debug!("ElevenLabs - fetching available voices");

        if self.config.api_key.trim().is_empty() {
            log::error!("ElevenLabs - API key is missing");
            return Err(TtsError::Unavailable(
                "ElevenLabs API key is missing".into(),
            ));
        }

        // Try fetching from API first, if it fails return default voices
        match self.list_voices_internal() {
            Ok(voices) => Ok(voices),
            Err(e) => {
                log::warn!("ElevenLabs - failed to fetch voices from API, using defaults: {}", e);
                Ok(Self::default_voices())
            }
        }
    }

    /// Internal method to fetch voices from API
    fn list_voices_internal(&self) -> Result<Vec<ElevenLabsVoice>, TtsError> {
        let url = "https://api.elevenlabs.io/v1/voices";
        let api_key = self.config.api_key.clone();

        let start_time = std::time::Instant::now();

        // Perform the full request-response cycle in one block to avoid body truncation
        // that occurs when .send() and .bytes() are split across separate block_on_async calls.
        let fetch_result: Result<(reqwest::StatusCode, Vec<u8>), TtsError> =
            Self::block_on_async(async move {
                let client = Client::new();
                let response = client
                    .get(url)
                    .header("xi-api-key", api_key)
                    .header("Accept", "application/json")
                    .send()
                    .await
                    .map_err(|e| TtsError::Http(format!("Failed to fetch voices: {}", e)))?;

                let status = response.status();
                let body = response
                    .bytes()
                    .await
                    .map_err(|e| TtsError::Http(format!("Failed to read voices response: {}", e)))?;

                Ok((status, body.to_vec()))
            });

        let (status, response_bytes) = fetch_result.map_err(|e| {
            let elapsed = start_time.elapsed();
            log::error!("ElevenLabs - failed to fetch voices after {:?}: {}", elapsed, e);
            e
        })?;

        let elapsed = start_time.elapsed();
        log::info!(
            "ElevenLabs - voices list response: {} {} (took {:?})",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            elapsed
        );

        let response_text = String::from_utf8_lossy(&response_bytes).to_string();

        if !status.is_success() {
            log::error!("ElevenLabs API error {}: {}", status, response_text);
            return Err(TtsError::Http(format!("ElevenLabs API error {}: {}", status, response_text)));
        }

        log::debug!(
            "ElevenLabs - raw voices response ({} bytes): {}",
            response_bytes.len(),
            &response_text[..response_text.len().min(500)]
        );

        #[derive(Deserialize)]
        struct VoicesResponse {
            voices: Vec<ElevenLabsVoice>,
        }

        let voices_response: VoicesResponse = serde_json::from_str(&response_text).map_err(|e| {
            log::error!("ElevenLabs - failed to parse voices response: {}", e);
            log::error!("ElevenLabs - response text was: {}", response_text);
            TtsError::Http(format!("Failed to parse voices response: {}", e))
        })?;

        log::info!("ElevenLabs - fetched {} voices", voices_response.voices.len());

        Ok(voices_response.voices)
    }

    /// Default ElevenLabs voices (fallback when API fails)
    fn default_voices() -> Vec<ElevenLabsVoice> {
        vec![
            ElevenLabsVoice {
                voice_id: "21m00Tcm4TlvDq8ikWAM".to_string(),
                name: Some("Rachel".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "female"})),
                description: Some("Young female voice, confident and clear".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/21m00Tcm4TlvDq8ikWAM".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "AZnzlk1XvdvUeBnXmlld".to_string(),
                name: Some("Domi".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "female"})),
                description: Some("Female voice, clear and professional".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/AZnzlk1XvdvUeBnXmlld".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "EXAVITQu4vr4xnSDxMaL".to_string(),
                name: Some("Sarah".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "female"})),
                description: Some("Female voice, energetic and friendly".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/EXAVITQu4vr4xnSDxMaL".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "JBFqnCBsd6RMkjVDRZzb".to_string(),
                name: Some("George".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "male"})),
                description: Some("Male voice, deep and professional".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/JBFqnCBsd6RMkjVDRZzb".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "N2lVS1w4xneBdscFXVwa".to_string(),
                name: Some("Arnold".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "male"})),
                description: Some("Male voice, strong and confident".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/N2lVS1w4xneBdscFXVwa".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "pNInz6obpgDQGcFmaJgB".to_string(),
                name: Some("Adam".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "male"})),
                description: Some("Male voice, expressive and natural".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/pNInz6obpgDQGcFmaJgB".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "ThT5KcBeYPX3keUQqHPh".to_string(),
                name: Some("Dorothy".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "female"})),
                description: Some("Female voice, soft and calming".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/ThT5KcBeYPX3keUQqHPh".to_string()),
            },
            ElevenLabsVoice {
                voice_id: "CwhRBWXzGAHq8TQ4Fs17".to_string(),
                name: Some("Roger".to_string()),
                category: Some("premade".to_string()),
                labels: Some(serde_json::json!({"language": "en", "gender": "male"})),
                description: Some("Male voice, casual and laid-back".to_string()),
                preview_url: Some("https://storage.googleapis.com/eleven-public-prod/premade/voices/CwhRBWXzGAHq8TQ4Fs17".to_string()),
            },
        ]
    }

    /// Resolve voice ID to human-readable name without API call.
    /// Returns capitalized voice name for default voices, or sanitized capitalized voice ID as fallback.
    pub fn resolve_voice_name_static(voice_id: &str) -> String {
        let name = Self::default_voices()
            .iter()
            .find(|v| v.voice_id == voice_id)
            .and_then(|v| v.name.clone())
            .unwrap_or_else(|| {
                voice_id
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>()
            });

        // Capitalize first letter
        let mut chars = name.chars();
        match chars.next() {
            None => name,
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Fetch a single voice by ID from ElevenLabs API
    /// https://elevenlabs.io/docs/api-reference/get-voices#get-voice-by-id
    pub fn get_voice_by_id(&self, voice_id: &str) -> Result<ElevenLabsVoice, TtsError> {
        log::debug!("ElevenLabs - fetching voice by ID: {}", voice_id);

        // First check if it's a known default voice
        if let Some(voice) = Self::default_voices().iter().find(|v| v.voice_id == voice_id) {
            log::info!("ElevenLabs - using default voice: {} ({})", voice.name.as_deref().unwrap_or("Unknown"), voice.voice_id);
            return Ok(voice.clone());
        }

        if self.config.api_key.trim().is_empty() {
            log::error!("ElevenLabs - API key is missing");
            return Err(TtsError::Unavailable(
                "ElevenLabs API key is missing".into(),
            ));
        }

        // Try fetching from API
        match self.get_voice_by_id_internal(voice_id) {
            Ok(voice) => Ok(voice),
            Err(e) => {
                log::warn!("ElevenLabs - failed to fetch voice {} from API: {}", voice_id, e);
                // Return not found error since we don't have it in defaults
                Err(TtsError::Http(format!("Voice {} not found", voice_id)))
            }
        }
    }

    /// Internal method to fetch a single voice by ID from API
    fn get_voice_by_id_internal(&self, voice_id: &str) -> Result<ElevenLabsVoice, TtsError> {
        let url = format!("https://api.elevenlabs.io/v1/voices/{}", voice_id);
        let api_key = self.config.api_key.clone();
        let voice_id_owned = voice_id.to_string();

        let start_time = std::time::Instant::now();

        // Perform the full request-response cycle in one block to avoid body truncation
        // that occurs when .send() and .bytes() are split across separate block_on_async calls.
        let fetch_result: Result<(reqwest::StatusCode, String), TtsError> =
            Self::block_on_async(async move {
                let client = Client::new();
                let response = client
                    .get(&url)
                    .header("xi-api-key", api_key)
                    .header("Accept", "application/json")
                    .send()
                    .await
                    .map_err(|e| TtsError::Http(format!("Failed to fetch voice: {}", e)))?;

                let status = response.status();
                let body = response
                    .text()
                    .await
                    .map_err(|e| TtsError::Http(format!("Failed to read voice response: {}", e)))?;

                Ok((status, body))
            });

        let (status, body) = fetch_result.map_err(|e| {
            let elapsed = start_time.elapsed();
            log::error!(
                "ElevenLabs - failed to fetch voice {} after {:?}: {}",
                voice_id_owned,
                elapsed,
                e
            );
            e
        })?;

        let elapsed = start_time.elapsed();
        log::info!(
            "ElevenLabs - voice {} response: {} {} (took {:?})",
            voice_id,
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            elapsed
        );

        if !status.is_success() {
            log::error!("ElevenLabs API error {}: {}", status, body);
            return Err(TtsError::Http(format!("ElevenLabs API error {}: {}", status, body)));
        }

        log::debug!("ElevenLabs - raw voice {} response: {}", voice_id, body);

        let voice: ElevenLabsVoice = serde_json::from_str(&body).map_err(|e| {
            log::error!("ElevenLabs - failed to parse voice response: {}", e);
            log::error!("ElevenLabs - response text was: {}", body);
            TtsError::Http(format!("Failed to parse voice response: {}", e))
        })?;

        log::info!("ElevenLabs - fetched voice: {} ({})", voice.name.as_deref().unwrap_or("Unknown"), voice.voice_id);

        Ok(voice)
    }

    /// Convert ElevenLabs voices to generic Voice structs
    #[allow(dead_code)]
    pub fn get_voices(&self) -> Result<Vec<Voice>, TtsError> {
        let voices = self.list_voices()?;
        Ok(voices
            .into_iter()
            .map(|v| Voice {
                id: v.voice_id.clone(),
                name: v.name.clone().unwrap_or_else(|| v.voice_id.clone()),
                language: v.labels.as_ref().and_then(|l| {
                    l.get("language")
                        .and_then(|lang| lang.as_str().map(|s| s.to_string()))
                }),
                default: None,
            })
            .collect())
    }

    /// Resolve voice ID to human-readable name (lowercase) for display and filenames.
    /// Uses static lookup only (no API call) to avoid runtime issues during synthesis.
    #[allow(dead_code)]
    pub fn resolve_voice_name(&self, voice_id: &str) -> String {
        Self::resolve_voice_name_static(voice_id)
    }
}

impl TtsBackend for ElevenLabsTtsBackend {
    fn name(&self) -> &str {
        "ElevenLabs"
    }

    fn file_extension(&self) -> &str {
        match self.config.output_format {
            ElevenLabsOutputFormat::Mp3_44100_128
            | ElevenLabsOutputFormat::Mp3_44100_192
            | ElevenLabsOutputFormat::Mp3_44100_32
            | ElevenLabsOutputFormat::Mp3_22050_32 => "mp3",
            ElevenLabsOutputFormat::OggVorbis_44100 | ElevenLabsOutputFormat::OggVorbis_22050 => {
                "ogg"
            }
            ElevenLabsOutputFormat::Flac_44100 => "flac",
            // PCM and MULAW are raw sample formats — wrap them in a WAV container
            _ => "wav",
        }
    }

    fn synthesize(&self, text: &str, _voice: &str, _speed: f32) -> Result<Vec<u8>, TtsError> {
        // Note: _speed parameter is ignored as ElevenLabs API doesn't support direct speed control
        // Speed adjustment should be done at playback level via audio player

        log::info!("ElevenLabs TTS request - voice: {}, model: {}, format: {}, text length: {} chars",
            self.config.voice_id, self.config.model_id, self.config.output_format.as_str(), text.len());

        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}",
            self.config.voice_id
        );

        let body = json!({
            "text": text,
            "model_id": self.config.model_id,
            "voice_settings": VoiceSettings {
                stability: self.config.voice_stability,
                similarity_boost: self.config.voice_similarity_boost,
                style: self.config.voice_style,
                use_speaker_boost: self.config.use_speaker_boost,
            },
        });

        if crate::logging::is_debug_mode() {
            log::debug!("ElevenLabs TTS request body: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
        }

        let api_key = self.config.api_key.clone();
        let mime_type = self.config.output_format.mime_type();
        let output_format = self.config.output_format.as_str();

        let start_time = std::time::Instant::now();

        let response = Self::block_on_async(async {
            let client = Client::new();
            client
                .post(&url)
                .header("xi-api-key", api_key)
                .header("Content-Type", "application/json")
                .header("Accept", mime_type)
                .query(&[("output_format", output_format)])
                .json(&body)
                .send()
                .await
        }).map_err(|e| {
            let elapsed = start_time.elapsed();
            log::error!("ElevenLabs TTS request failed after {:?}: {}", elapsed, e);
            TtsError::Http(format!("Request failed: {}", e))
        })?;

        let elapsed = start_time.elapsed();
        let status = response.status();

        // Log response status and timing
        log::info!(
            "ElevenLabs TTS response: {} {} (took {:?})",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            elapsed
        );

        if !response.status().is_success() {
            let error_text = Self::block_on_async(async {
                response.text().await.unwrap_or_default()
            });
            log::error!(
                "ElevenLabs API error {}: {}",
                status, error_text
            );
            // Provide a user-friendly message for payment-required errors (library voices)
            if status == reqwest::StatusCode::PAYMENT_REQUIRED
                || error_text.contains("paid_plan_required")
                || error_text.contains("payment_required")
            {
                return Err(TtsError::Http(
                    "This voice is from the ElevenLabs voice library and requires a paid plan. \
                     Please use one of your own cloned voices, or upgrade your ElevenLabs subscription."
                        .to_string(),
                ));
            }
            return Err(TtsError::Http(format!(
                "ElevenLabs API error {}: {}",
                status, error_text
            )));
        }

        let bytes = Self::block_on_async(async {
            response.bytes().await
        }).map_err(|e| {
            log::error!("Failed to read response bytes: {}", e);
            TtsError::Http(format!("Failed to read bytes: {}", e))
        })?;

        log::info!("ElevenLabs TTS synthesis complete: received {} bytes", bytes.len());

        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("ElevenLabs TTS health check - validating API key");

        if self.config.api_key.trim().is_empty() {
            log::error!("ElevenLabs TTS health check failed - API key is missing");
            return Err(TtsError::Unavailable(
                "ElevenLabs API key is missing".into(),
            ));
        }

        // Optionally, we could make a lightweight API call to verify the key
        // For now, we just check that the key is present
        log::debug!("ElevenLabs TTS health check passed");
        Ok(())
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        Self::resolve_voice_name_static(voice_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_as_str() {
        assert_eq!(
            ElevenLabsOutputFormat::Mp3_44100_128.as_str(),
            "mp3_44100_128"
        );
        assert_eq!(ElevenLabsOutputFormat::Pcm_44100.as_str(), "pcm_44100");
    }

    #[test]
    fn test_output_format_mime_type() {
        assert_eq!(
            ElevenLabsOutputFormat::Mp3_44100_128.mime_type(),
            "audio/mpeg"
        );
        assert_eq!(ElevenLabsOutputFormat::Pcm_44100.mime_type(), "audio/pcm");
    }

    #[test]
    fn test_output_format_is_playable_by_rodio() {
        assert!(ElevenLabsOutputFormat::Mp3_44100_128.is_playable_by_rodio());
        assert!(ElevenLabsOutputFormat::Pcm_44100.is_playable_by_rodio());
        assert!(ElevenLabsOutputFormat::Flac_44100.is_playable_by_rodio());
        assert!(!ElevenLabsOutputFormat::Mulaw_8000.is_playable_by_rodio());
    }
}
