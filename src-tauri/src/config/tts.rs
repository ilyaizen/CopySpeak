// TTS engine configuration: backend selection, local CLI, OpenAI, ElevenLabs, HTTP configs.

use serde::{Deserialize, Serialize};

use super::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TtsEngine {
    Local,
    Http,
    OpenAI,
    ElevenLabs,
    Cartesia,
    Google,
    Microsoft,
}

impl Default for TtsEngine {
    fn default() -> Self {
        TtsEngine::Cartesia
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
    pub voice: String,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "tts-1".into(),
            voice: "alloy".into(),
        }
    }
}

impl OpenAIConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let errors = Vec::new();
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevenLabsConfig {
    pub api_key: String,
    pub voice_id: String,
    /// Cached voice name for display (resolved from API or default voices)
    #[serde(default)]
    pub voice_name: Option<String>,
    pub model_id: String,
    /// Output format for audio generation
    #[serde(default)]
    pub output_format: crate::tts::elevenlabs::ElevenLabsOutputFormat,
    /// Voice stability (0.0 - 1.0, default: 0.5)
    #[serde(default = "default_elevenlabs_stability")]
    pub voice_stability: f32,
    /// Voice similarity boost (0.0 - 1.0, default: 0.75)
    #[serde(default = "default_elevenlabs_similarity")]
    pub voice_similarity_boost: f32,
    /// Voice style exaggeration (0.0 - 1.0, default: None)
    #[serde(default)]
    pub voice_style: Option<f32>,
    /// Use speaker boost (default: None)
    #[serde(default)]
    pub use_speaker_boost: Option<bool>,
    /// Whether to use manual voice ID input instead of the API voice list
    #[serde(default)]
    pub use_manual_voice_id: bool,
}

fn default_elevenlabs_stability() -> f32 {
    0.5
}

fn default_elevenlabs_similarity() -> f32 {
    0.75
}

impl Default for ElevenLabsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            voice_id: "21m00Tcm4TlvDq8ikWAM".into(), // Rachel
            voice_name: Some("Rachel".into()),
            model_id: "eleven_turbo_v2_5".into(),
            output_format: crate::tts::elevenlabs::ElevenLabsOutputFormat::default(),
            voice_stability: default_elevenlabs_stability(),
            voice_similarity_boost: default_elevenlabs_similarity(),
            voice_style: None,
            use_speaker_boost: None,
            use_manual_voice_id: false,
        }
    }
}

impl ElevenLabsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let errors = Vec::new();
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartesiaConfig {
    pub api_key: String,
    pub model_id: String,
    pub voice_id: String,
    pub voice_name: Option<String>,
    pub output_format: String,
    pub use_manual_voice_id: bool,
}

impl Default for CartesiaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model_id: "sonic-3.5".into(),
            voice_id: "f786b574-daa5-4673-aa0c-cbe3e8534c02".into(),
            voice_name: Some("Katie".into()),
            output_format: "wav".into(),
            use_manual_voice_id: false,
        }
    }
}

impl CartesiaConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let errors = Vec::new();
        errors
    }
}

// ── Google Gemini TTS ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleTtsConfig {
    pub api_key: String,
    pub model: String,
    pub voice_name: String,
    pub output_format: String,
}

impl Default for GoogleTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gemini-2.5-flash-preview-tts".into(),
            voice_name: "Kore".into(),
            output_format: "wav".into(),
        }
    }
}

impl GoogleTtsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        Vec::new()
    }
}

// ── Microsoft MAI-Voice-2 ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MicrosoftTtsConfig {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    pub voice_name: String,
    pub output_format: String,
}

impl Default for MicrosoftTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            endpoint: String::new(),
            model: "mai-voice-2".into(),
            voice_name: String::new(),
            output_format: "wav".into(),
        }
    }
}

impl MicrosoftTtsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        Vec::new()
    }
}

// ── Generic HTTP-serving TTS ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpTtsConfig {
    pub profile_id: String,
    pub url_template: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body_template: Option<String>,
    pub voice: String,
    pub response_format: String,
    pub timeout_secs: u64,
}

impl Default for HttpTtsConfig {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            url_template: String::new(),
            method: "POST".into(),
            headers: Vec::new(),
            body_template: Some(
                r#"{"model":"tts","input":"{text}","voice":"{voice}"}"#.into(),
            ),
            voice: String::new(),
            response_format: "wav".into(),
            timeout_secs: 60,
        }
    }
}

impl HttpTtsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        Vec::new()
    }
}

// ── Voice profiles ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileEffects {
    pub enabled: bool,
    pub active_effect: crate::config::EffectId,
}

impl Default for ProfileEffects {
    fn default() -> Self {
        Self {
            enabled: false,
            active_effect: crate::config::EffectId::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub engine: TtsEngine,
    pub voice: String,
    pub speed: f32,
    pub pitch: f32,
    pub effects: ProfileEffects,
    pub engine_options: serde_json::Value,
}

impl Default for VoiceProfile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            engine: TtsEngine::Cartesia,
            voice: "f786b574-daa5-4673-aa0c-cbe3e8534c02".into(),
            speed: 1.0,
            pitch: 1.0,
            effects: ProfileEffects::default(),
            engine_options: serde_json::json!({}),
        }
    }
}

fn schema_version_absent() -> u32 {
    0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsConfig {
    // Field-level defaults override the container default so that a legacy config
    // (which lacks these keys) deserializes to 0/empty and triggers migration,
    // instead of silently inheriting the struct Default's populated profile.
    #[serde(default = "schema_version_absent")]
    pub schema_version: u32,
    pub active_backend: TtsEngine,

    // Profile layer
    pub active_profile_id: String,
    #[serde(default)]
    pub profiles: Vec<VoiceProfile>,

    // Local Config
    pub preset: String,
    pub command: String,
    pub args_template: Vec<String>,
    pub voice: String,

    // Cloud Configs
    pub openai: OpenAIConfig,
    pub elevenlabs: ElevenLabsConfig,
    pub cartesia: CartesiaConfig,
    pub google: GoogleTtsConfig,
    pub microsoft: MicrosoftTtsConfig,
    pub http: HttpTtsConfig,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_backend: TtsEngine::Cartesia,
            active_profile_id: "default".into(),
            profiles: vec![VoiceProfile::default()],
            preset: "kitten-tts".into(),
            command: "py".into(),
            args_template: vec![
                "-3.12".into(),
                "{home_dir}/kittentts/kittentts-cli.py".into(),
                "--text".into(),
                "{raw_text}".into(),
                "--voice".into(),
                "{voice}".into(),
                "--output".into(),
                "{output}".into(),
            ],
            voice: "Rosie".into(),
            openai: OpenAIConfig::default(),
            elevenlabs: ElevenLabsConfig::default(),
            cartesia: CartesiaConfig::default(),
            google: GoogleTtsConfig::default(),
            microsoft: MicrosoftTtsConfig::default(),
            http: HttpTtsConfig::default(),
        }
    }
}

/// Migrate a legacy single-engine TTS config into the profile model.
/// Idempotent: a config already at schema_version 1 with profiles is returned unchanged.
pub fn migrate_tts_config(mut tts: TtsConfig) -> TtsConfig {
    if tts.schema_version == 0 || tts.profiles.is_empty() {
        let voice = match tts.active_backend {
            TtsEngine::Local => tts.voice.clone(),
            TtsEngine::Http => tts.http.voice.clone(),
            TtsEngine::OpenAI => tts.openai.voice.clone(),
            TtsEngine::ElevenLabs => tts.elevenlabs.voice_id.clone(),
            TtsEngine::Cartesia => tts.cartesia.voice_id.clone(),
            TtsEngine::Google => tts.google.voice_name.clone(),
            TtsEngine::Microsoft => tts.microsoft.voice_name.clone(),
        };

        tts.active_profile_id = "default".into();
        tts.profiles = vec![VoiceProfile {
            id: "default".into(),
            name: "Default".into(),
            engine: tts.active_backend.clone(),
            voice,
            speed: 1.0,
            pitch: 1.0,
            effects: ProfileEffects::default(),
            engine_options: serde_json::json!({}),
        }];
        tts.schema_version = 1;
    }

    tts
}

impl TtsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        match self.active_backend {
            TtsEngine::Local => {
                if self.command.trim().is_empty() {
                    errors.push(ValidationError::CommandEmpty);
                }

                // Accept {input}, {text} (legacy), or {raw_text} (inline text) placeholder
                let has_input_placeholder = self.args_template.iter().any(|arg| {
                    arg.contains("{input}") || arg.contains("{text}") || arg.contains("{raw_text}")
                });
                if !has_input_placeholder {
                    errors.push(ValidationError::ArgsTemplateMissingPlaceholder {
                        placeholder: "{input}, {text}, or {raw_text}".into(),
                    });
                }

                let has_output_placeholder = self
                    .args_template
                    .iter()
                    .any(|arg| arg.contains("{output}"));
                if !has_output_placeholder {
                    errors.push(ValidationError::ArgsTemplateMissingPlaceholder {
                        placeholder: "{output}".into(),
                    });
                }
            }
            TtsEngine::Http => {
                errors.extend(self.http.validate());
            }
            TtsEngine::OpenAI => {
                errors.extend(self.openai.validate());
            }
            TtsEngine::ElevenLabs => {
                errors.extend(self.elevenlabs.validate());
            }
            TtsEngine::Cartesia => {
                errors.extend(self.cartesia.validate());
            }
            TtsEngine::Google => {
                errors.extend(self.google.validate());
            }
            TtsEngine::Microsoft => {
                errors.extend(self.microsoft.validate());
            }
        }

        errors
    }
}
