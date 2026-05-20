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

pub const DEFAULT_PROMPT: &str = "Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.\n\nRules:\n- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.\n- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.\n- Max 3 bullets/points. No framing or commentary. Output only rewritten text.\n\nPattern: [thing] [action] [reason]. [next step].\n\nText:\n${output}";

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
