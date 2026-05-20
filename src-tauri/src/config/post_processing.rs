use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PostProcessingProvider {
    Groq,
    OpenAI,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingConfig {
    pub enabled: bool,
    pub provider: PostProcessingProvider,
    pub prompt: String,
    pub groq: LlmProviderConfig,
    pub openai: LlmProviderConfig,
    pub anthropic: LlmProviderConfig,
    pub gemini: LlmProviderConfig,
    pub openrouter: LlmProviderConfig,
    pub ollama: LlmProviderConfig,
    pub custom: LlmProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub api_key: String,
    pub model: String,
    pub endpoint: String,
}

impl Default for PostProcessingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: PostProcessingProvider::Groq,
            prompt: DEFAULT_PROMPT_CONCISE.to_string(),
            groq: LlmProviderConfig {
                api_key: String::new(),
                model: "llama-3.1-8b-instant".to_string(),
                endpoint: "https://api.groq.com/openai/v1/chat/completions".to_string(),
            },
            openai: LlmProviderConfig {
                api_key: String::new(),
                model: "gpt-4o-mini".to_string(),
                endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            },
            anthropic: LlmProviderConfig {
                api_key: String::new(),
                model: "claude-3-5-haiku-latest".to_string(),
                endpoint: "https://api.anthropic.com/v1/messages".to_string(),
            },
            gemini: LlmProviderConfig {
                api_key: String::new(),
                model: "gemini-1.5-flash".to_string(),
                endpoint: "https://generativelanguage.googleapis.com/v1beta/models".to_string(),
            },
            openrouter: LlmProviderConfig {
                api_key: String::new(),
                model: "openai/gpt-4o-mini".to_string(),
                endpoint: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            },
            ollama: LlmProviderConfig {
                api_key: String::new(),
                model: "llama3.1".to_string(),
                endpoint: "http://localhost:11434/v1/chat/completions".to_string(),
            },
            custom: LlmProviderConfig {
                api_key: String::new(),
                model: String::new(),
                endpoint: String::new(),
            },
        }
    }
}

pub const DEFAULT_PROMPT_CONCISE: &str = "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.";
