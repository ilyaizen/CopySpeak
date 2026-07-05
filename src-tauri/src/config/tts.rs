// TTS engine configuration: backend selection, local CLI, OpenAI, ElevenLabs, HTTP configs.

use serde::{Deserialize, Serialize};

use super::ValidationError;
use crate::tts::catalog;

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
    Edge,
}

impl Default for TtsEngine {
    fn default() -> Self {
        TtsEngine::Edge
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    // Credential — persisted. Profile owns model/voice/format/instructions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(skip_serializing, default = "default_openai_model")]
    pub model: String,
    #[serde(skip_serializing, default)]
    pub voice: String,
    #[serde(skip_serializing, default = "default_openai_response_format")]
    pub response_format: String,
    #[serde(skip_serializing, default)]
    pub instructions: Option<String>,
}

fn default_openai_model() -> String {
    "tts-1".into()
}

fn default_openai_response_format() -> String {
    "wav".into()
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "tts-1".into(),
            voice: "alloy".into(),
            response_format: default_openai_response_format(),
            instructions: None,
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
    // Credential — persisted. Profile owns voice/model/format/knobs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(skip_serializing, default)]
    pub voice_id: String,
    /// Cached voice name for display (resolved from API or default voices)
    #[serde(skip_serializing, default)]
    pub voice_name: Option<String>,
    #[serde(skip_serializing, default)]
    pub model_id: String,
    /// Output format for audio generation
    #[serde(skip_serializing, default)]
    pub output_format: crate::tts::elevenlabs::ElevenLabsOutputFormat,
    /// Voice stability (0.0 - 1.0, default: 0.5)
    #[serde(skip_serializing, default = "default_elevenlabs_stability")]
    pub voice_stability: f32,
    /// Voice similarity boost (0.0 - 1.0, default: 0.75)
    #[serde(skip_serializing, default = "default_elevenlabs_similarity")]
    pub voice_similarity_boost: f32,
    /// Voice style exaggeration (0.0 - 1.0, default: None)
    #[serde(skip_serializing, default)]
    pub voice_style: Option<f32>,
    /// Use speaker boost (default: None)
    #[serde(skip_serializing, default)]
    pub use_speaker_boost: Option<bool>,
    /// Whether to use manual voice ID input instead of the API voice list
    #[serde(skip_serializing, default)]
    #[allow(dead_code)]
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
    // Credential — persisted. Profile owns model/voice/format/knobs.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(skip_serializing, default)]
    pub model_id: String,
    #[serde(skip_serializing, default)]
    pub voice_id: String,
    #[serde(skip_serializing, default)]
    pub voice_name: Option<String>,
    #[serde(skip_serializing, default)]
    pub output_format: String,
    #[serde(skip_serializing, default)]
    pub encoding: Option<String>,
    #[serde(skip_serializing, default)]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing, default)]
    #[allow(dead_code)]
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
            encoding: Some("pcm_f32le".into()),
            sample_rate: Some(44100),
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
    // Credential — persisted. Profile owns model/voice/format.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(skip_serializing)]
    pub model: String,
    #[serde(skip_serializing)]
    pub voice_name: String,
    #[serde(skip_serializing)]
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
    // Credentials — persisted. Profile owns model/voice/format.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
    #[serde(skip_serializing)]
    pub model: String,
    #[serde(skip_serializing)]
    pub voice_name: String,
    #[serde(skip_serializing)]
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

// ── Edge-TTS (free Microsoft Read Aloud via rany2/edge-tts) ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeTtsConfig {
    pub voice: String,
}

impl Default for EdgeTtsConfig {
    fn default() -> Self {
        Self {
            voice: "en-US-AvaMultilingualNeural".into(),
        }
    }
}

impl EdgeTtsConfig {
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
            body_template: Some(r#"{"model":"tts","input":"{text}","voice":"{voice}"}"#.into()),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTextProcessingMode {
    InheritGlobal,
    Disabled,
    Enabled,
}

impl Default for ProfileTextProcessingMode {
    fn default() -> Self {
        ProfileTextProcessingMode::InheritGlobal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BracketedEmoteStrategy {
    KeepLiteral,
    Strip,
    ConvertToSsmlOrInstruction,
}

impl Default for BracketedEmoteStrategy {
    fn default() -> Self {
        BracketedEmoteStrategy::KeepLiteral
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileTextProcessing {
    pub mode: ProfileTextProcessingMode,
    pub strip_emote_brackets: bool,
    pub bracketed_emote_strategy: BracketedEmoteStrategy,
}

impl Default for ProfileTextProcessing {
    fn default() -> Self {
        Self {
            mode: ProfileTextProcessingMode::InheritGlobal,
            strip_emote_brackets: false,
            bracketed_emote_strategy: BracketedEmoteStrategy::KeepLiteral,
        }
    }
}

// ── Typed per-engine profile options ──────────────────────────────────────────
//
// On disk each profile's options are an object tagged with the engine name
// (e.g. `{ "engine": "openai", "model": "tts-1" }`). Legacy configs/exports
// stored a plain untagged object (or `{}`); those deserialize as `Legacy` and
// are normalized into the right typed variant during `migrate_tts_config`, using
// the profile's own `engine` field as the discriminant. Unset keys are omitted
// on serialize so exports stay minimal and import-compatible.

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args_template: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAiEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElevenLabsEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_boost: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_speaker_boost: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CartesiaEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MicrosoftEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeEngineOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

/// Engine-specific, non-secret synthesis knobs carried by a voice profile.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileEngineOptions {
    Local(LocalEngineOptions),
    Http(HttpEngineOptions),
    OpenAI(OpenAiEngineOptions),
    ElevenLabs(ElevenLabsEngineOptions),
    Cartesia(CartesiaEngineOptions),
    Google(GoogleEngineOptions),
    Microsoft(MicrosoftEngineOptions),
    Edge(EdgeEngineOptions),
    /// Untagged legacy bag captured at load; resolved during migration.
    Legacy(serde_json::Map<String, serde_json::Value>),
}

impl Default for ProfileEngineOptions {
    fn default() -> Self {
        ProfileEngineOptions::Edge(EdgeEngineOptions::default())
    }
}

impl ProfileEngineOptions {
    /// Empty typed options for the given engine.
    pub fn default_for(engine: &TtsEngine) -> Self {
        Self::from_engine_map(engine, serde_json::Map::new())
    }

    fn matches_engine(&self, engine: &TtsEngine) -> bool {
        matches!(
            (self, engine),
            (Self::Local(_), TtsEngine::Local)
                | (Self::Http(_), TtsEngine::Http)
                | (Self::OpenAI(_), TtsEngine::OpenAI)
                | (Self::ElevenLabs(_), TtsEngine::ElevenLabs)
                | (Self::Cartesia(_), TtsEngine::Cartesia)
                | (Self::Google(_), TtsEngine::Google)
                | (Self::Microsoft(_), TtsEngine::Microsoft)
                | (Self::Edge(_), TtsEngine::Edge)
        )
    }

    fn into_raw_map(self) -> serde_json::Map<String, serde_json::Value> {
        if let Self::Legacy(map) = self {
            return map;
        }
        match serde_json::to_value(&self) {
            Ok(serde_json::Value::Object(mut map)) => {
                map.remove("engine");
                map
            }
            _ => serde_json::Map::new(),
        }
    }

    fn from_engine_map(
        engine: &TtsEngine,
        map: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        let value = serde_json::Value::Object(map);
        match engine {
            TtsEngine::Local => Self::Local(serde_json::from_value(value).unwrap_or_default()),
            TtsEngine::Http => Self::Http(serde_json::from_value(value).unwrap_or_default()),
            TtsEngine::OpenAI => Self::OpenAI(serde_json::from_value(value).unwrap_or_default()),
            TtsEngine::ElevenLabs => {
                Self::ElevenLabs(serde_json::from_value(value).unwrap_or_default())
            }
            TtsEngine::Cartesia => {
                Self::Cartesia(serde_json::from_value(value).unwrap_or_default())
            }
            TtsEngine::Google => Self::Google(serde_json::from_value(value).unwrap_or_default()),
            TtsEngine::Microsoft => {
                Self::Microsoft(serde_json::from_value(value).unwrap_or_default())
            }
            TtsEngine::Edge => Self::Edge(serde_json::from_value(value).unwrap_or_default()),
        }
    }

    /// Normalize to the typed variant matching `engine`, preserving any
    /// compatible fields from a legacy or mismatched options bag.
    pub fn normalized_for(self, engine: &TtsEngine) -> Self {
        if self.matches_engine(engine) {
            self
        } else {
            Self::from_engine_map(engine, self.into_raw_map())
        }
    }

    pub fn local(&self) -> Option<&LocalEngineOptions> {
        match self {
            Self::Local(o) => Some(o),
            _ => None,
        }
    }
    pub fn http(&self) -> Option<&HttpEngineOptions> {
        match self {
            Self::Http(o) => Some(o),
            _ => None,
        }
    }
    pub fn openai(&self) -> Option<&OpenAiEngineOptions> {
        match self {
            Self::OpenAI(o) => Some(o),
            _ => None,
        }
    }
    pub fn elevenlabs(&self) -> Option<&ElevenLabsEngineOptions> {
        match self {
            Self::ElevenLabs(o) => Some(o),
            _ => None,
        }
    }
    pub fn cartesia(&self) -> Option<&CartesiaEngineOptions> {
        match self {
            Self::Cartesia(o) => Some(o),
            _ => None,
        }
    }
    pub fn google(&self) -> Option<&GoogleEngineOptions> {
        match self {
            Self::Google(o) => Some(o),
            _ => None,
        }
    }
    pub fn microsoft(&self) -> Option<&MicrosoftEngineOptions> {
        match self {
            Self::Microsoft(o) => Some(o),
            _ => None,
        }
    }
    pub fn edge(&self) -> Option<&EdgeEngineOptions> {
        match self {
            Self::Edge(o) => Some(o),
            _ => None,
        }
    }
}

impl Serialize for ProfileEngineOptions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;
        let (engine, value) = match self {
            Self::Local(o) => ("local", serde_json::to_value(o)),
            Self::Http(o) => ("http", serde_json::to_value(o)),
            Self::OpenAI(o) => ("openai", serde_json::to_value(o)),
            Self::ElevenLabs(o) => ("elevenlabs", serde_json::to_value(o)),
            Self::Cartesia(o) => ("cartesia", serde_json::to_value(o)),
            Self::Google(o) => ("google", serde_json::to_value(o)),
            Self::Microsoft(o) => ("microsoft", serde_json::to_value(o)),
            Self::Edge(o) => ("edge", serde_json::to_value(o)),
            // Legacy bags serialize back as their raw untagged object.
            Self::Legacy(map) => {
                return serde_json::Value::Object(map.clone()).serialize(serializer);
            }
        };
        let mut value = value.map_err(S::Error::custom)?;
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "engine".to_string(),
                serde_json::Value::String(engine.to_string()),
            );
        }
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProfileEngineOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let map = match value {
            serde_json::Value::Object(map) => map,
            serde_json::Value::Null => return Ok(Self::Legacy(serde_json::Map::new())),
            other => {
                return Err(D::Error::custom(format!(
                    "engine_options must be an object, got {other}"
                )))
            }
        };
        // Tagged objects resolve directly; untagged legacy bags are deferred to
        // migration, which knows the owning profile's engine.
        match map.get("engine").and_then(|v| v.as_str()) {
            Some("local") => Ok(Self::from_engine_map(&TtsEngine::Local, map)),
            Some("http") => Ok(Self::from_engine_map(&TtsEngine::Http, map)),
            Some("openai") => Ok(Self::from_engine_map(&TtsEngine::OpenAI, map)),
            Some("elevenlabs") => Ok(Self::from_engine_map(&TtsEngine::ElevenLabs, map)),
            Some("cartesia") => Ok(Self::from_engine_map(&TtsEngine::Cartesia, map)),
            Some("google") => Ok(Self::from_engine_map(&TtsEngine::Google, map)),
            Some("microsoft") => Ok(Self::from_engine_map(&TtsEngine::Microsoft, map)),
            Some("edge") => Ok(Self::from_engine_map(&TtsEngine::Edge, map)),
            _ => Ok(Self::Legacy(map)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub engine: TtsEngine,
    pub voice: String,
    pub voice_label: Option<String>,
    pub speed: f32,
    pub pitch: f32,
    pub effects: ProfileEffects,
    pub text_processing: ProfileTextProcessing,
    pub engine_options: ProfileEngineOptions,
}

fn prettify_voice_id(voice: &str) -> String {
    voice
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn profile_display_name(engine: &TtsEngine, voice_label: Option<&str>, voice: &str) -> String {
    let engine_label = catalog::list_engines()
        .into_iter()
        .find(|entry| entry.engine == *engine)
        .map(|entry| entry.label)
        .unwrap_or_else(|| format!("{:?}", engine));
    let voice_name = voice_label
        .filter(|label| !label.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| prettify_voice_id(voice));
    format!("{} - {}", engine_label, voice_name)
}

fn catalog_voice_label(engine: &TtsEngine, voice: &str) -> Option<String> {
    catalog::list_engines()
        .into_iter()
        .find(|entry| entry.engine == *engine)
        .and_then(|entry| entry.voices.into_iter().find(|v| v.id == voice))
        .map(|voice| voice.label)
}

impl Default for VoiceProfile {
    fn default() -> Self {
        let engine = TtsEngine::Edge;
        let voice = "en-US-AvaMultilingualNeural".to_string();
        let voice_label = Some("Ava, Multilingual".to_string());
        Self {
            id: "default".into(),
            name: profile_display_name(&engine, voice_label.as_deref(), &voice),
            description: None,
            engine,
            voice,
            voice_label,
            speed: 1.0,
            pitch: 1.0,
            effects: ProfileEffects::default(),
            text_processing: ProfileTextProcessing::default(),
            engine_options: ProfileEngineOptions::default_for(&TtsEngine::Edge),
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

    // Legacy fields — kept for deserialization of old configs, skipped on serialize.
    // Migration (v0→v2) copies what's needed into profiles.
    #[serde(default, skip_serializing)]
    pub preset: String,
    #[serde(default, skip_serializing)]
    pub command: String,
    #[serde(default, skip_serializing)]
    pub args_template: Vec<String>,
    #[serde(default, skip_serializing)]
    pub voice: String,

    // Per-engine global structs persist ONLY their credential fields
    // (api_key, endpoint); all profile-owned knobs are skip_serializing at
    // the field level. This keeps secrets on disk (engines page relies on it)
    // without duplicating profile-owned data into the global config.
    #[serde(default)]
    pub openai: OpenAIConfig,
    #[serde(default)]
    pub elevenlabs: ElevenLabsConfig,
    #[serde(default)]
    pub cartesia: CartesiaConfig,
    #[serde(default)]
    pub google: GoogleTtsConfig,
    #[serde(default)]
    pub microsoft: MicrosoftTtsConfig,
    #[serde(default, skip_serializing)]
    pub edge: EdgeTtsConfig,
    #[serde(default, skip_serializing)]
    pub http: HttpTtsConfig,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            schema_version: 3,
            active_backend: TtsEngine::Edge,
            active_profile_id: "default".into(),
            profiles: vec![VoiceProfile::default()],
            preset: "kitten-tts".into(),
            command: "uv".into(),
            args_template: vec![
                "run".into(),
                "--project".into(),
                "{engine_dir}/kitten".into(),
                "python".into(),
                "scripts/copyspeak-kitten.py".into(),
                "--text-file".into(),
                "{input}".into(),
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
            edge: EdgeTtsConfig::default(),
            http: HttpTtsConfig::default(),
        }
    }
}

/// Migrate a legacy single-engine TTS config into the profile model.
/// Idempotent: a config already at schema_version 1 with profiles is returned unchanged.
pub fn sync_active_backend_mirror(tts: &mut TtsConfig) {
    if let Some(profile) = tts.profiles.iter().find(|p| p.id == tts.active_profile_id) {
        tts.active_backend = profile.engine.clone();
    }
}

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
            TtsEngine::Edge => tts.edge.voice.clone(),
        };

        let voice_label = catalog_voice_label(&tts.active_backend, &voice).or_else(|| match tts.active_backend {
            TtsEngine::ElevenLabs => tts.elevenlabs.voice_name.clone(),
            TtsEngine::Cartesia => tts.cartesia.voice_name.clone(),
            _ => None,
        });
        tts.active_profile_id = "default".into();
        tts.profiles = vec![VoiceProfile {
            id: "default".into(),
            name: profile_display_name(&tts.active_backend, voice_label.as_deref(), &voice),
            description: None,
            engine: tts.active_backend.clone(),
            voice,
            voice_label,
            speed: 1.0,
            pitch: 1.0,
            effects: ProfileEffects::default(),
            text_processing: ProfileTextProcessing::default(),
            engine_options: ProfileEngineOptions::default_for(&tts.active_backend),
        }];
    }

    if tts.active_profile_id.trim().is_empty() {
        tts.active_profile_id = "default".into();
    }
    // Resolve legacy/untagged option bags into the typed variant matching each
    // profile's engine, filling defaults for any unset knobs.
    for profile in &mut tts.profiles {
        let engine = profile.engine.clone();
        let opts = std::mem::take(&mut profile.engine_options);
        profile.engine_options = opts.normalized_for(&engine);
    }
    tts.schema_version = 2;
    sync_active_backend_mirror(&mut tts);

    tts
}

impl TtsConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        let active_profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == self.active_profile_id);
        let active_engine = active_profile
            .map(|profile| &profile.engine)
            .unwrap_or(&self.active_backend);

        match active_engine {
            TtsEngine::Local => {
                // Read command/args_template from the active profile's engine_options
                // (the real source of truth). Fall back to legacy global fields for
                // configs that haven't been migrated yet — those still hold the
                // pre-profile values until v2 migration runs.
                let local_opts = active_profile.and_then(|p| p.engine_options.local());
                let command = local_opts
                    .and_then(|o| o.command.clone())
                    .unwrap_or_else(|| self.command.clone());
                let args_template = local_opts
                    .and_then(|o| o.args_template.clone())
                    .unwrap_or_else(|| self.args_template.clone());

                if command.trim().is_empty() {
                    errors.push(ValidationError::CommandEmpty);
                }

                // Accept {input}, {text} (legacy), or {raw_text} (inline text) placeholder
                let has_input_placeholder = args_template.iter().any(|arg| {
                    arg.contains("{input}") || arg.contains("{text}") || arg.contains("{raw_text}")
                });
                if !has_input_placeholder {
                    errors.push(ValidationError::ArgsTemplateMissingPlaceholder {
                        placeholder: "{input}, {text}, or {raw_text}".into(),
                    });
                }

                let has_output_placeholder =
                    args_template.iter().any(|arg| arg.contains("{output}"));
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
            TtsEngine::Edge => {
                errors.extend(self.edge.validate());
            }
        }

        errors
    }
}
