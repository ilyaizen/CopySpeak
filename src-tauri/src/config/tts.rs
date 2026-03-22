// TTS engine configuration: backend selection, local CLI, OpenAI, ElevenLabs, HTTP configs.

use serde::{Deserialize, Serialize};

use super::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TtsEngine {
    Local,
    OpenAI,
    ElevenLabs,
}

impl Default for TtsEngine {
    fn default() -> Self {
        TtsEngine::Local
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
#[serde(default)]
pub struct TtsConfig {
    pub active_backend: TtsEngine,

    // Local Config
    pub preset: String,
    pub command: String,
    pub args_template: Vec<String>,
    pub voice: String,

    // Cloud Configs
    pub openai: OpenAIConfig,
    pub elevenlabs: ElevenLabsConfig,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            active_backend: TtsEngine::Local,
            preset: "kitten-tts".into(),
            command: "python".into(),
            args_template: vec![
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
        }
    }
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
            TtsEngine::OpenAI => {
                errors.extend(self.openai.validate());
            }
            TtsEngine::ElevenLabs => {
                errors.extend(self.elevenlabs.validate());
            }
        }

        errors
    }
}
