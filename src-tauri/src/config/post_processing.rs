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
    Xai,
    Aws,
    Cerebras,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingConfig {
    pub enabled: bool,
    pub provider: PostProcessingProvider,
    pub prompt: String,
    #[serde(default = "default_selected_prompt_label")]
    pub selected_prompt_label: String,
    #[serde(default = "default_prompt_presets")]
    pub prompt_presets: Vec<PostProcessingPromptPreset>,
    pub groq: LlmProviderConfig,
    pub openai: LlmProviderConfig,
    pub anthropic: LlmProviderConfig,
    pub gemini: LlmProviderConfig,
    pub openrouter: LlmProviderConfig,
    pub ollama: LlmProviderConfig,
    #[serde(default = "default_xai_config")]
    pub xai: LlmProviderConfig,
    #[serde(default = "default_aws_config")]
    pub aws: LlmProviderConfig,
    #[serde(default = "default_cerebras_config")]
    pub cerebras: LlmProviderConfig,
    pub custom: LlmProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingPromptPreset {
    pub label: String,
    pub prompt: String,
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
            prompt: DEFAULT_PROMPT_SUMMARIZE.to_string(),
            selected_prompt_label: default_selected_prompt_label(),
            prompt_presets: default_prompt_presets(),
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
            xai: default_xai_config(),
            aws: default_aws_config(),
            cerebras: default_cerebras_config(),
            custom: LlmProviderConfig {
                api_key: String::new(),
                model: String::new(),
                endpoint: String::new(),
            },
        }
    }
}

fn default_selected_prompt_label() -> String {
    "Summarize".to_string()
}

fn default_xai_config() -> LlmProviderConfig {
    LlmProviderConfig {
        api_key: String::new(),
        model: "grok-3-mini".to_string(),
        endpoint: "https://api.x.ai/v1/chat/completions".to_string(),
    }
}

fn default_aws_config() -> LlmProviderConfig {
    LlmProviderConfig {
        api_key: String::new(),
        model: "amazon.nova-lite-v1:0".to_string(),
        endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1/chat/completions".to_string(),
    }
}

fn default_cerebras_config() -> LlmProviderConfig {
    LlmProviderConfig {
        api_key: String::new(),
        model: "llama3.1-8b".to_string(),
        endpoint: "https://api.cerebras.ai/v1/chat/completions".to_string(),
    }
}

pub fn default_prompt_presets() -> Vec<PostProcessingPromptPreset> {
    vec![
        PostProcessingPromptPreset {
            label: "Concise developer".to_string(),
            prompt: DEFAULT_PROMPT_CONCISE.to_string(),
        },
        PostProcessingPromptPreset {
            label: "Cleanup".to_string(),
            prompt: "Clean up grammar, punctuation, spacing, and obvious transcription/copy artifacts. Preserve meaning and technical terms. Return only the cleaned text.".to_string(),
        },
        PostProcessingPromptPreset {
            label: "Professional".to_string(),
            prompt: "Rewrite in a concise professional tone for a technical audience. Preserve meaning and code identifiers. Return only the rewritten text.".to_string(),
        },
        PostProcessingPromptPreset {
            label: "Summarize".to_string(),
            prompt: DEFAULT_PROMPT_SUMMARIZE.to_string(),
        },
        PostProcessingPromptPreset {
            label: "TTS optimized".to_string(),
            prompt: "Optimize this text for text-to-speech. Remove markdown noise, make punctuation natural for speech, and keep technical meaning. Return only the optimized text.".to_string(),
        },
        PostProcessingPromptPreset {
            label: "Caveman".to_string(),
            prompt: "Compress aggressively into terse caveman-style developer notes. Keep technical facts, names, paths, and commands exact. Prefer short sentences over lists unless the input is a list. Return only the compressed text.".to_string(),
        },
    ]
}

pub const DEFAULT_PROMPT_CONCISE: &str = "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.";
pub const DEFAULT_PROMPT_SUMMARIZE: &str = "Summarize the text for a developer in 1-3 concise sentences. Preserve key decisions, requirements, and action items. Return only the summary.";
