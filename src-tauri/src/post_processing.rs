use crate::config::{
    BracketedEmoteStrategy, LlmProviderConfig, PostProcessingConfig, PostProcessingProvider,
    ProfileTextProcessing, ProfileTextProcessingMode,
};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

pub async fn process_text(config: &PostProcessingConfig, text: &str) -> Result<String, String> {
    if !config.enabled || text.trim().is_empty() {
        return Ok(text.to_string());
    }

    let provider_config = provider_config(config);
    if provider_config.model.trim().is_empty() || provider_config.endpoint.trim().is_empty() {
        return Ok(text.to_string());
    }

    match config.provider {
        PostProcessingProvider::Anthropic => process_anthropic(config, provider_config, text).await,
        PostProcessingProvider::Gemini => process_gemini(config, provider_config, text).await,
        _ => process_openai_compatible(config, provider_config, text).await,
    }
}

pub async fn process_text_for_profile(
    config: &PostProcessingConfig,
    profile: &ProfileTextProcessing,
    text: &str,
) -> Result<String, String> {
    let text = apply_bracketed_emote_strategy(text, &profile.bracketed_emote_strategy);
    match profile.mode {
        ProfileTextProcessingMode::Disabled => Ok(text),
        ProfileTextProcessingMode::InheritGlobal | ProfileTextProcessingMode::Enabled => {
            process_text(config, &text).await
        }
    }
}

pub fn apply_bracketed_emote_strategy(text: &str, strategy: &BracketedEmoteStrategy) -> String {
    match strategy {
        BracketedEmoteStrategy::KeepLiteral => text.to_string(),
        BracketedEmoteStrategy::Strip | BracketedEmoteStrategy::ConvertToSsmlOrInstruction => {
            strip_bracketed_emotes(text)
        }
    }
}

fn strip_bracketed_emotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0_u32;
    for ch in text.chars() {
        match ch {
            '[' => depth = depth.saturating_add(1),
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn provider_config(config: &PostProcessingConfig) -> &LlmProviderConfig {
    match config.provider {
        PostProcessingProvider::Groq => &config.groq,
        PostProcessingProvider::OpenAI => &config.openai,
        PostProcessingProvider::Anthropic => &config.anthropic,
        PostProcessingProvider::Gemini => &config.gemini,
        PostProcessingProvider::OpenRouter => &config.openrouter,
        PostProcessingProvider::Ollama => &config.ollama,
        PostProcessingProvider::Xai => &config.xai,
        PostProcessingProvider::Aws => &config.aws,
        PostProcessingProvider::Cerebras => &config.cerebras,
        PostProcessingProvider::Custom => &config.custom,
    }
}

async fn process_openai_compatible(
    config: &PostProcessingConfig,
    provider: &LlmProviderConfig,
    text: &str,
) -> Result<String, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if !provider.api_key.trim().is_empty() {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", provider.api_key.trim()))
                .map_err(|e| e.to_string())?,
        );
    }

    let body = json!({
        "model": provider.model,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": config.prompt },
            { "role": "user", "content": text }
        ]
    });

    let value: Value = reqwest::Client::new()
        .post(&provider.endpoint)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Post-Processing request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Post-Processing API error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Post-Processing response parse failed: {e}"))?;

    extract_openai_text(&value).ok_or_else(|| "Post-Processing response had no text".to_string())
}

async fn process_anthropic(
    config: &PostProcessingConfig,
    provider: &LlmProviderConfig,
    text: &str,
) -> Result<String, String> {
    let body = json!({
        "model": provider.model,
        "max_tokens": 2048,
        "temperature": 0.2,
        "system": config.prompt,
        "messages": [{ "role": "user", "content": text }]
    });

    let value: Value = reqwest::Client::new()
        .post(&provider.endpoint)
        .header("x-api-key", provider.api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Post-Processing request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Post-Processing API error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Post-Processing response parse failed: {e}"))?;

    value["content"]
        .as_array()
        .and_then(|items| items.iter().find_map(|item| item["text"].as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Post-Processing response had no text".to_string())
}

async fn process_gemini(
    config: &PostProcessingConfig,
    provider: &LlmProviderConfig,
    text: &str,
) -> Result<String, String> {
    let endpoint = format!(
        "{}/{}:generateContent?key={}",
        provider.endpoint.trim_end_matches('/'),
        provider.model,
        provider.api_key.trim()
    );
    let body = json!({
        "systemInstruction": { "parts": [{ "text": config.prompt }] },
        "contents": [{ "role": "user", "parts": [{ "text": text }] }],
        "generationConfig": { "temperature": 0.2 }
    });

    let value: Value = reqwest::Client::new()
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Post-Processing request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Post-Processing API error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Post-Processing response parse failed: {e}"))?;

    value["candidates"][0]["content"]["parts"]
        .as_array()
        .and_then(|parts| parts.iter().find_map(|part| part["text"].as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Post-Processing response had no text".to_string())
}

fn extract_openai_text(value: &Value) -> Option<String> {
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_literal_leaves_brackets_untouched() {
        let out = apply_bracketed_emote_strategy("[laughs] hello", &BracketedEmoteStrategy::KeepLiteral);
        assert_eq!(out, "[laughs] hello");
    }

    #[test]
    fn strip_removes_brackets_and_normalizes_whitespace() {
        let out = apply_bracketed_emote_strategy("[laughs] hello", &BracketedEmoteStrategy::Strip);
        assert_eq!(out, "hello");
    }

    #[test]
    fn convert_falls_back_to_strip_deterministically() {
        let out = apply_bracketed_emote_strategy(
            "[sighs] hello [whispers] there",
            &BracketedEmoteStrategy::ConvertToSsmlOrInstruction,
        );
        assert_eq!(out, "hello there");
    }

    #[test]
    fn strip_handles_nested_brackets() {
        let out = apply_bracketed_emote_strategy("[a [b] c] keep", &BracketedEmoteStrategy::Strip);
        assert_eq!(out, "keep");
    }
}
