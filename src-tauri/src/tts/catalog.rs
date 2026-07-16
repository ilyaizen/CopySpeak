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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
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
    pub gender: Option<String>,
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
        choices: None,
    }
}

fn option_with_choices(
    key: &str,
    label: &str,
    kind: EngineOptionKind,
    help: &str,
    default_value: serde_json::Value,
    choices: Vec<String>,
) -> EngineOptionDescriptor {
    EngineOptionDescriptor {
        key: key.into(),
        label: label.into(),
        kind,
        help: help.into(),
        default_value,
        choices: Some(choices),
    }
}

fn voice(
    id: &str,
    label: &str,
    language: Option<&str>,
    description: Option<&str>,
    gender: Option<&str>,
) -> VoiceCatalogEntry {
    VoiceCatalogEntry {
        id: id.into(),
        label: label.into(),
        language: language.map(str::to_string),
        description: description.map(str::to_string),
        gender: gender.map(str::to_string),
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
                option_with_choices(
                    "preset",
                    "Preset",
                    EngineOptionKind::Select,
                    "Local engine preset.",
                    serde_json::json!("kitten-tts"),
                    vec![
                        "kitten-tts".into(),
                        "piper".into(),
                        "kokoro".into(),
                        "chatterbox".into(),
                        "custom".into(),
                    ],
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
            voices: vec![
                // KittenTTS
                voice("Rosie", "Rosie", Some("KittenTTS"), Some("KittenTTS voice (female)"), Some("female")),
                voice("Clio", "Clio", Some("KittenTTS"), Some("KittenTTS voice (female)"), Some("female")),
                voice("Hugo", "Hugo", Some("KittenTTS"), Some("KittenTTS voice (male)"), Some("male")),
                voice("Leo", "Leo", Some("KittenTTS"), Some("KittenTTS voice (male)"), Some("male")),
                // Piper
                voice("en_US-amy-medium", "Amy", Some("Piper"), Some("Piper voice (female)"), Some("female")),
                voice("en_US-lessac-medium", "Lessac", Some("Piper"), Some("Piper voice (female)"), Some("female")),
                voice("en_US-ryan-medium", "Ryan", Some("Piper"), Some("Piper voice (male)"), Some("male")),
                voice("en_US-joe-medium", "Joe", Some("Piper"), Some("Piper voice (male)"), Some("male")),
                voice("en_US-libritts-medium", "LibriTTS", Some("Piper"), Some("Piper voice (mixed)"), Some("neutral")),
                // Kokoro
                voice("af_heart", "Heart", Some("Kokoro"), Some("Kokoro voice (American female, flagship)"), Some("female")),
                voice("af_bella", "Bella", Some("Kokoro"), Some("Kokoro voice (American female)"), Some("female")),
                voice("af_nicole", "Nicole", Some("Kokoro"), Some("Kokoro voice (American female)"), Some("female")),
                voice("af_sarah", "Sarah", Some("Kokoro"), Some("Kokoro voice (American female)"), Some("female")),
                voice("am_adam", "Adam", Some("Kokoro"), Some("Kokoro voice (American male)"), Some("male")),
                voice("am_michael", "Michael", Some("Kokoro"), Some("Kokoro voice (American male)"), Some("male")),
                voice("bf_emma", "Emma", Some("Kokoro"), Some("Kokoro voice (British female)"), Some("female")),
                voice("bm_george", "George", Some("Kokoro"), Some("Kokoro voice (British male)"), Some("male")),
                // Chatterbox
                voice("default", "Default", Some("Chatterbox"), Some("Chatterbox default voice"), Some("neutral")),
            ],
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
            voices: vec![
                voice("alloy", "Alloy", Some("en"), Some("Warm, neutral all-rounder"), Some("female")),
                voice("ash", "Ash", Some("en"), Some("Calm and conversational"), Some("male")),
                voice("ballad", "Ballad", Some("en"), Some("Singing-leaning, expressive"), Some("male")),
                voice("coral", "Coral", Some("en"), Some("Warm and confident"), Some("female")),
                voice("echo", "Echo", Some("en"), Some("Calm and measured"), Some("male")),
                voice("fable", "Fable", Some("en"), Some("British and expressive"), Some("male")),
                voice("nova", "Nova", Some("en"), Some("Bright and energetic"), Some("female")),
                voice("onyx", "Onyx", Some("en"), Some("Deep and authoritative"), Some("male")),
                voice("sage", "Sage", Some("en"), Some("Calm and introspective"), Some("female")),
                voice("shimmer", "Shimmer", Some("en"), Some("Soft and airy"), Some("female")),
                voice("verse", "Verse", Some("en"), Some("Neutral and steady"), Some("male")),
            ],
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
            voices: vec![
                // 21 premade voices — fetched from GET /v1/voices (2026-07-06).
                // ponytail: static snapshot; refresh button hits the live API for the full set.
                voice("pNInz6obpgDQGcFmaJgB", "Adam", None, Some("American · male · Dominant, Firm"), Some("male")),
                voice("Xb7hH8MSUJpSbSDYk0k2", "Alice", None, Some("British · female · Clear, Engaging Educator"), Some("female")),
                voice("hpp4J3VqNfWAUOO0d1Us", "Bella", None, Some("American · female · Professional, Bright, Warm"), Some("female")),
                voice("pqHfZKP75CvOlQylNhV4", "Bill", None, Some("American · male · Wise, Mature, Balanced"), Some("male")),
                voice("nPczCjzI2devNBz1zQrb", "Brian", None, Some("American · male · Deep, Resonant and Comforting"), Some("male")),
                voice("N2lVS1w4EtoT3dr4eOWO", "Callum", None, Some("American · male · Husky Trickster"), Some("male")),
                voice("IKne3meq5aSn9XLyUdCD", "Charlie", None, Some("Australian · male · Deep, Confident, Energetic"), Some("male")),
                voice("iP95p4xoKVk53GoZ742B", "Chris", None, Some("American · male · Charming, Down-to-Earth"), Some("male")),
                voice("onwK4e9ZLuTAKqWW03F9", "Daniel", None, Some("British · male · Steady Broadcaster"), Some("male")),
                voice("cjVigY5qzO86Huf0OWal", "Eric", None, Some("American · male · Smooth, Trustworthy"), Some("male")),
                voice("JBFqnCBsd6RMkjVDRZzb", "George", None, Some("British · male · Warm, Captivating Storyteller"), Some("male")),
                voice("SOYHLrjzK2X1ezoPC6cr", "Harry", None, Some("American · male · Fierce Warrior"), Some("male")),
                voice("cgSgspJ2msm6clMCkdW9", "Jessica", None, Some("American · female · Playful, Bright, Warm"), Some("female")),
                voice("FGY2WhTYpPnrIDTdsKH5", "Laura", None, Some("American · female · Enthusiast, Quirky Attitude"), Some("female")),
                voice("TX3LPaxmHKxFdv7VOQHJ", "Liam", None, Some("American · male · Energetic, Social Media Creator"), Some("male")),
                voice("pFZP5JQG7iQjIQuC4Bku", "Lily", None, Some("British · female · Velvety Actress"), Some("female")),
                voice("XrExE9yKIg1WjnnlVkGX", "Matilda", None, Some("American · female · Knowledgable, Professional"), Some("female")),
                voice("SAz9YHcvj6GT2YYXdXww", "River", None, Some("American · neutral · Relaxed, Neutral, Informative"), Some("neutral")),
                voice("CwhRBWXzGAHq8TQ4Fs17", "Roger", None, Some("American · male · Laid-Back, Casual, Resonant"), Some("male")),
                voice("EXAVITQu4vr4xnSDxMaL", "Sarah", None, Some("American · female · Mature, Reassuring, Confident"), Some("female")),
                voice("bIHbv24MWmeRgasZH58o", "Will", None, Some("American · male · Relaxed Optimist"), Some("male")),
            ],
        },
        EngineCatalogEntry {
            engine: TtsEngine::Cartesia,
            label: "Cartesia".into(),
            description: "Cartesia Sonic TTS.".into(),
            docs_url: "https://docs.cartesia.ai/api-reference/tts/bytes".into(),
            supports_voice_refresh: true,
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
                    None,
                    Some("Warm, friendly default"),
                    Some("female"),
                ),
                voice(
                    "a5136bf9-224c-4d76-b823-52bd5efcffcc",
                    "Jameson",
                    None,
                    Some("Calm, deep default"),
                    Some("male"),
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
            voices: vec![
                voice("Kore", "Kore", None, Some("Upbeat"), Some("female")),
                voice("Puck", "Puck", None, Some("Upbeat"), Some("male")),
                voice("Charon", "Charon", None, Some("Informative"), Some("male")),
                voice("Fenrir", "Fenrir", None, Some("Breezy, casual"), Some("male")),
                voice("Leda", "Leda", None, Some("Youthful"), Some("female")),
                voice("Orus", "Orus", None, Some("Firm, industrial"), Some("male")),
                voice("Aoede", "Aoede", None, Some("Soft, dreamy"), Some("female")),
                voice("Callirrhoe", "Callirrhoe", None, Some("Girl, youthful"), Some("female")),
                voice("Autonoe", "Autonoe", None, Some("Breezy"), Some("female")),
                voice("Enceladus", "Enceladus", None, Some("Breezy"), Some("male")),
                voice("Iapetus", "Iapetus", None, Some("Clear, professional"), Some("male")),
                voice("Umbriel", "Umbriel", None, Some("Easygoing"), Some("male")),
                voice("Algieba", "Algieba", None, Some("Casual, relaxed"), Some("female")),
                voice("Despina", "Despina", None, Some("Youthful"), Some("female")),
                voice("Erinome", "Erinome", None, Some("Calm"), Some("female")),
                voice("Algenib", "Algenib", None, Some("Sturdy"), Some("male")),
                voice("Rasalgethi", "Rasalgethi", None, Some("Informative"), Some("male")),
                voice("Laomedeia", "Laomedeia", None, Some("Upbeat"), Some("female")),
                voice("Achernar", "Achernar", None, Some("Soft, low"), Some("male")),
                voice("Alnilam", "Alnilam", None, Some("Firm"), Some("male")),
                voice("Schedar", "Schedar", None, Some("Casual"), Some("female")),
                voice("Gacrux", "Gacrux", None, Some("Maternal"), Some("female")),
                voice("Pulcherrima", "Pulcherrima", None, Some("Soft, forward"), Some("female")),
                voice("Achird", "Achird", None, Some("Relaxed, easygoing"), Some("male")),
                voice("Zubenelgenubi", "Zubenelgenubi", None, Some("Casual, laid-back"), Some("male")),
                voice("Vindemiatrix", "Vindemiatrix", None, Some("Gentle"), Some("female")),
                voice("Sadachbia", "Sadachbia", None, Some("Lively, breezy"), Some("male")),
                voice("Sadaltager", "Sadaltager", None, Some("Knowledgeable, warm"), Some("male")),
                voice("Sulafat", "Sulafat", None, Some("Warm"), Some("female")),
            ],
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
                // Genders sourced from `edge-tts --list-voices` (Microsoft's own
                // readaloud metadata), lowercased to match the cloud engines.
                // ── US ──────────────────────────────────────────
                ("en-US-AvaMultilingualNeural", "Ava, Multilingual", Some("female")),
                ("en-US-EmmaMultilingualNeural", "Emma, Multilingual", Some("female")),
                ("en-US-AndrewMultilingualNeural", "Andrew, Multilingual", Some("male")),
                ("en-US-BrianMultilingualNeural", "Brian, Multilingual", Some("male")),
                ("en-US-AriaNeural", "Aria", Some("female")),
                ("en-US-JennyNeural", "Jenny", Some("female")),
                ("en-US-GuyNeural", "Guy", Some("male")),
                // ponytail: Davis & Amber are absent from the live edge-tts list
                // (likely deprecated by Microsoft); gender unknown → None.
                ("en-US-DavisNeural", "Davis", None),
                ("en-US-AmberNeural", "Amber", None),
                ("en-US-AnaNeural", "Ana", Some("female")),
                ("en-US-AndrewNeural", "Andrew", Some("male")),
                ("en-US-AvaNeural", "Ava", Some("female")),
                ("en-US-BrianNeural", "Brian", Some("male")),
                ("en-US-ChristopherNeural", "Christopher", Some("male")),
                ("en-US-EmmaNeural", "Emma", Some("female")),
                ("en-US-EricNeural", "Eric", Some("male")),
                ("en-US-MichelleNeural", "Michelle", Some("female")),
                ("en-US-RogerNeural", "Roger", Some("male")),
                ("en-US-SteffanNeural", "Steffan", Some("male")),
                // ── GB ──────────────────────────────────────────
                ("en-GB-SoniaNeural", "Sonia", Some("female")),
                ("en-GB-RyanNeural", "Ryan", Some("male")),
                ("en-GB-LibbyNeural", "Libby", Some("female")),
                ("en-GB-MaisieNeural", "Maisie", Some("female")),
                ("en-GB-ThomasNeural", "Thomas", Some("male")),
                // ── AU ──────────────────────────────────────────
                ("en-AU-NatashaNeural", "Natasha", Some("female")),
                ("en-AU-WilliamMultilingualNeural", "William, Multilingual", Some("male")),
                // ── CA ──────────────────────────────────────────
                ("en-CA-ClaraNeural", "Clara", Some("female")),
                ("en-CA-LiamNeural", "Liam", Some("male")),
                // ── IE ──────────────────────────────────────────
                ("en-IE-ConnorNeural", "Connor", Some("male")),
                ("en-IE-EmilyNeural", "Emily", Some("female")),
            ]
            .iter()
            .map(|(id, label, gender)| {
                // Edge ids encode the BCP-47 locale (en-US, en-GB, …); keep it as
                // the grouping key so the picker clusters by region, with gender
                // surfaced as per-row meta (see voice-picker.svelte::useLocale).
                let locale: String = id.split('-').take(2).collect::<Vec<_>>().join("-");
                voice(id, label, Some(locale.as_str()), None, *gender)
            })
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
