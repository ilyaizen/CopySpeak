use crate::config::TtsEngine;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EngineCatalogEntry {
    pub engine: TtsEngine,
    pub label: String,
    pub description: String,
    pub docs_url: String,
    pub supports_voice_refresh: bool,
    pub supports_pitch: bool,
    pub supports_bracket_emotes: bool,
    pub options: Vec<EngineOptionDescriptor>,
    pub voices: Vec<VoiceCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineOptionDescriptor {
    pub key: String,
    pub label: String,
    pub kind: EngineOptionKind,
    pub help: String,
    pub default_value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum EngineOptionKind {
    Text,
    Number,
    Boolean,
    Select,
    Textarea,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceCatalogEntry {
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub preview_url: Option<String>,
}

fn option(
    key: &str,
    label: &str,
    kind: EngineOptionKind,
    help: &str,
    default_value: serde_json::Value,
) -> EngineOptionDescriptor {
    EngineOptionDescriptor {
        key: key.into(),
        label: label.into(),
        kind,
        help: help.into(),
        default_value,
    }
}

fn voice(
    id: &str,
    label: &str,
    language: Option<&str>,
    description: Option<&str>,
) -> VoiceCatalogEntry {
    VoiceCatalogEntry {
        id: id.into(),
        label: label.into(),
        language: language.map(str::to_string),
        description: description.map(str::to_string),
        preview_url: None,
    }
}

pub fn list_engines() -> Vec<EngineCatalogEntry> {
    vec![
        EngineCatalogEntry {
            engine: TtsEngine::Local,
            label: "Local CLI".into(),
            description: "Run a local command-line TTS wrapper.".into(),
            docs_url: "docs/profile-engine-settings.md#engine-matrix".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: false,
            options: vec![
                option(
                    "preset",
                    "Preset",
                    EngineOptionKind::Text,
                    "Local engine preset label.",
                    serde_json::json!("kitten-tts"),
                ),
                option(
                    "command",
                    "Command",
                    EngineOptionKind::Text,
                    "Executable to run.",
                    serde_json::json!("py"),
                ),
                option(
                    "args_template",
                    "Arguments",
                    EngineOptionKind::Textarea,
                    "Argument template with {text}, {voice}, and {output} placeholders.",
                    serde_json::json!([]),
                ),
            ],
            voices: vec![voice(
                "Rosie",
                "Rosie",
                Some("en"),
                Some("KittenTTS fallback voice"),
            )],
        },
        EngineCatalogEntry {
            engine: TtsEngine::Http,
            label: "HTTP".into(),
            description: "Generic HTTP-serving TTS backend (OpenAI-compatible, Chatterbox server, etc.).".into(),
            docs_url: "docs/profile-engine-settings.md#engine-matrix".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: false,
            options: vec![
                option(
                    "url_template",
                    "URL Template",
                    EngineOptionKind::Text,
                    "Endpoint URL with optional placeholders.",
                    serde_json::json!(""),
                ),
                option(
                    "method",
                    "Method",
                    EngineOptionKind::Text,
                    "HTTP method.",
                    serde_json::json!("POST"),
                ),
                option(
                    "body_template",
                    "Body Template",
                    EngineOptionKind::Textarea,
                    "Optional JSON body template.",
                    serde_json::json!(null),
                ),
                option(
                    "response_format",
                    "Response Format",
                    EngineOptionKind::Text,
                    "Audio file extension returned by the server.",
                    serde_json::json!("wav"),
                ),
                option(
                    "timeout_secs",
                    "Timeout",
                    EngineOptionKind::Number,
                    "Request timeout in seconds.",
                    serde_json::json!(60),
                ),
            ],
            voices: vec![],
        },
        EngineCatalogEntry {
            engine: TtsEngine::OpenAI,
            label: "OpenAI".into(),
            description: "OpenAI speech synthesis API.".into(),
            docs_url: "https://platform.openai.com/docs/guides/text-to-speech".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: true,
            options: vec![
                option(
                    "model",
                    "Model",
                    EngineOptionKind::Text,
                    "Speech model.",
                    serde_json::json!("tts-1"),
                ),
                option(
                    "response_format",
                    "Response Format",
                    EngineOptionKind::Text,
                    "Audio response format.",
                    serde_json::json!("wav"),
                ),
                option(
                    "instructions",
                    "Instructions",
                    EngineOptionKind::Textarea,
                    "Optional speaking style instructions for models that support them.",
                    serde_json::json!(null),
                ),
            ],
            voices: [
                "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage",
                "shimmer", "verse",
            ]
            .iter()
            .map(|v| voice(v, v, Some("en"), None))
            .collect(),
        },
        EngineCatalogEntry {
            engine: TtsEngine::ElevenLabs,
            label: "ElevenLabs".into(),
            description: "ElevenLabs text-to-speech.".into(),
            docs_url: "https://elevenlabs.io/docs/api-reference/text-to-speech/convert".into(),
            supports_voice_refresh: true,
            supports_pitch: false,
            supports_bracket_emotes: false,
            options: vec![
                option(
                    "model_id",
                    "Model",
                    EngineOptionKind::Text,
                    "ElevenLabs model id.",
                    serde_json::json!("eleven_turbo_v2_5"),
                ),
                option(
                    "output_format",
                    "Output Format",
                    EngineOptionKind::Text,
                    "ElevenLabs output format.",
                    serde_json::json!("mp3_44100_128"),
                ),
                option(
                    "stability",
                    "Stability",
                    EngineOptionKind::Number,
                    "Voice stability from 0 to 1.",
                    serde_json::json!(0.5),
                ),
                option(
                    "similarity_boost",
                    "Similarity",
                    EngineOptionKind::Number,
                    "Similarity boost from 0 to 1.",
                    serde_json::json!(0.75),
                ),
                option(
                    "style",
                    "Style",
                    EngineOptionKind::Number,
                    "Optional style exaggeration.",
                    serde_json::json!(null),
                ),
                option(
                    "use_speaker_boost",
                    "Speaker Boost",
                    EngineOptionKind::Boolean,
                    "Optional speaker boost flag.",
                    serde_json::json!(null),
                ),
            ],
            voices: vec![voice(
                "21m00Tcm4TlvDq8ikWAM",
                "Rachel",
                Some("en"),
                Some("Premade fallback voice"),
            )],
        },
        EngineCatalogEntry {
            engine: TtsEngine::Cartesia,
            label: "Cartesia".into(),
            description: "Cartesia Sonic TTS.".into(),
            docs_url: "https://docs.cartesia.ai/api-reference/tts/bytes".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: false,
            options: vec![
                option(
                    "model_id",
                    "Model",
                    EngineOptionKind::Text,
                    "Cartesia model id.",
                    serde_json::json!("sonic-3.5"),
                ),
                option(
                    "output_format",
                    "Output Format",
                    EngineOptionKind::Text,
                    "Container format.",
                    serde_json::json!("wav"),
                ),
                option(
                    "encoding",
                    "Encoding",
                    EngineOptionKind::Text,
                    "Optional output encoding.",
                    serde_json::json!("pcm_f32le"),
                ),
                option(
                    "sample_rate",
                    "Sample Rate",
                    EngineOptionKind::Number,
                    "Optional sample rate.",
                    serde_json::json!(44100),
                ),
            ],
            voices: vec![
                voice(
                    "f786b574-daa5-4673-aa0c-cbe3e8534c02",
                    "Katie",
                    Some("en"),
                    None,
                ),
                voice(
                    "a5136bf9-224c-4d76-b823-52bd5efcffcc",
                    "Jameson",
                    Some("en"),
                    None,
                ),
            ],
        },
        EngineCatalogEntry {
            engine: TtsEngine::Google,
            label: "Google Gemini".into(),
            description: "Gemini native audio generation.".into(),
            docs_url: "https://ai.google.dev/gemini-api/docs/speech-generation".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: true,
            options: vec![
                option(
                    "model",
                    "Model",
                    EngineOptionKind::Text,
                    "Gemini TTS model.",
                    serde_json::json!("gemini-2.5-flash-preview-tts"),
                ),
                option(
                    "output_format",
                    "Output Format",
                    EngineOptionKind::Text,
                    "Output format after wrapping PCM.",
                    serde_json::json!("wav"),
                ),
            ],
            voices: [
                "Kore",
                "Puck",
                "Charon",
                "Fenrir",
                "Leda",
                "Orus",
                "Aoede",
                "Callirrhoe",
                "Autonoe",
                "Enceladus",
                "Iapetus",
                "Umbriel",
                "Algieba",
                "Despina",
                "Erinome",
                "Algenib",
                "Rasalgethi",
                "Laomedeia",
                "Achernar",
                "Alnilam",
                "Schedar",
                "Gacrux",
                "Pulcherrima",
                "Achird",
                "Zubenelgenubi",
                "Vindemiatrix",
                "Sadachbia",
                "Sadaltager",
                "Sulafat",
            ]
            .iter()
            .map(|v| voice(v, v, None, None))
            .collect(),
        },
        EngineCatalogEntry {
            engine: TtsEngine::Microsoft,
            label: "Microsoft AI".into(),
            description: "Microsoft MAI/Azure speech endpoint.".into(),
            docs_url:
                "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech"
                    .into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: true,
            options: vec![
                option(
                    "endpoint",
                    "Endpoint",
                    EngineOptionKind::Text,
                    "Deployment endpoint.",
                    serde_json::json!(""),
                ),
                option(
                    "model",
                    "Model",
                    EngineOptionKind::Text,
                    "Model or deployment name.",
                    serde_json::json!("mai-voice-2"),
                ),
                option(
                    "output_format",
                    "Output Format",
                    EngineOptionKind::Text,
                    "Response format.",
                    serde_json::json!("wav"),
                ),
            ],
            voices: vec![],
        },
        EngineCatalogEntry {
            engine: TtsEngine::Edge,
            label: "Edge-TTS".into(),
            description: "Free Microsoft Read Aloud voices via edge-tts. No API key required."
                .into(),
            docs_url: "https://github.com/rany2/edge-tts".into(),
            supports_voice_refresh: false,
            supports_pitch: false,
            supports_bracket_emotes: false,
            options: vec![],
            voices: [
                // ── US ──────────────────────────────────────────
                ("en-US-AvaMultilingualNeural", "Ava, Multilingual"),
                ("en-US-EmmaMultilingualNeural", "Emma, Multilingual"),
                ("en-US-AndrewMultilingualNeural", "Andrew, Multilingual"),
                ("en-US-BrianMultilingualNeural", "Brian, Multilingual"),
                ("en-US-AriaNeural", "Aria"),
                ("en-US-JennyNeural", "Jenny"),
                ("en-US-GuyNeural", "Guy"),
                ("en-US-DavisNeural", "Davis"),
                ("en-US-AmberNeural", "Amber"),
                ("en-US-AnaNeural", "Ana"),
                ("en-US-AndrewNeural", "Andrew"),
                ("en-US-AvaNeural", "Ava"),
                ("en-US-BrianNeural", "Brian"),
                ("en-US-ChristopherNeural", "Christopher"),
                ("en-US-EmmaNeural", "Emma"),
                ("en-US-EricNeural", "Eric"),
                ("en-US-MichelleNeural", "Michelle"),
                ("en-US-RogerNeural", "Roger"),
                ("en-US-SteffanNeural", "Steffan"),
                // ── GB ──────────────────────────────────────────
                ("en-GB-SoniaNeural", "Sonia, United Kingdom"),
                ("en-GB-RyanNeural", "Ryan, United Kingdom"),
                ("en-GB-LibbyNeural", "Libby, United Kingdom"),
                ("en-GB-MaisieNeural", "Maisie, United Kingdom"),
                ("en-GB-ThomasNeural", "Thomas, United Kingdom"),
                // ── AU ──────────────────────────────────────────
                ("en-AU-NatashaNeural", "Natasha, Australia"),
                ("en-AU-WilliamMultilingualNeural", "William, Australia, Multilingual"),
                // ── CA ──────────────────────────────────────────
                ("en-CA-ClaraNeural", "Clara, Canada"),
                ("en-CA-LiamNeural", "Liam, Canada"),
                // ── IE ──────────────────────────────────────────
                ("en-IE-ConnorNeural", "Connor, Ireland"),
                ("en-IE-EmilyNeural", "Emily, Ireland"),
            ]
            .iter()
            .map(|(id, label)| voice(id, label, Some("en"), None))
            .collect(),
        },
    ]
}

pub fn list_static_voices(engine: &TtsEngine) -> Vec<VoiceCatalogEntry> {
    list_engines()
        .into_iter()
        .find(|entry| &entry.engine == engine)
        .map(|entry| entry.voices)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_has_exactly_one_catalog_entry() {
        let entries = list_engines();
        for engine in [
            TtsEngine::Local,
            TtsEngine::Http,
            TtsEngine::OpenAI,
            TtsEngine::ElevenLabs,
            TtsEngine::Cartesia,
            TtsEngine::Google,
            TtsEngine::Microsoft,
            TtsEngine::Edge,
        ] {
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| entry.engine == engine)
                    .count(),
                1
            );
        }
        assert_eq!(entries.len(), 8);
    }

    #[test]
    fn catalog_entries_have_labels_and_docs() {
        for entry in list_engines() {
            assert!(!entry.label.trim().is_empty());
            assert!(!entry.docs_url.trim().is_empty());
        }
    }
}
