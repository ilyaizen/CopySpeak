// LLM post-processing configuration (Groq Cloud).
//
// Optional pass between the sanitize stage and TTS synthesis. When enabled,
// copied text is rewritten by a Groq chat-completions model to be more concise
// and listener-friendly before being spoken.

use serde::{Deserialize, Serialize};

pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";

pub const AVAILABLE_MODELS: &[&str] = &[
    "openai/gpt-oss-20b",
    "llama-3.3-70b-versatile",
    "llama-3.1-8b-instant",
];

pub const DEFAULT_PROMPT: &str = "Rewrite the following text so it is concise and easy to listen to for a software developer. Goals:\n- Cut filler, repetition, and boilerplate.\n- Preserve every technical fact, name, number, code identifier, and command verbatim.\n- Use short sentences. Prefer active voice.\n- Do not add commentary or framing (\"Here is...\", \"Sure!\"). Output only the rewritten text.\n- Keep the original language.\n\nText:\n${output}";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessConfig {
    pub enabled: bool,
    pub api_key: String,
    pub model: String,
    pub prompt: String,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            model: AVAILABLE_MODELS[0].into(),
            prompt: DEFAULT_PROMPT.into(),
        }
    }
}
