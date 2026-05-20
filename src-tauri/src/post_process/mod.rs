// LLM post-processing: sends sanitized text through Groq Cloud's chat
// completions API to produce a concise, listener-friendly rewrite before TTS.
//
// Failure policy: this module never silently swallows errors. `process()`
// surfaces them; `try_process()` is the single fallback wrapper that callers
// in the synthesis pipeline use to keep TTS running on LLM failure.

use crate::config::{PostProcessConfig, GROQ_BASE_URL};
use log::warn;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

/// Run the configured Groq prompt against `text`. Returns `Ok(processed)` on
/// success. Caller is responsible for the fallback — this never silently
/// returns the original.
pub async fn process(text: &str, cfg: &PostProcessConfig) -> Result<String, String> {
    if cfg.api_key.trim().is_empty() {
        return Err("Groq API key is empty".into());
    }
    if cfg.model.trim().is_empty() {
        return Err("Groq model is empty".into());
    }

    let user_content = build_prompt(&cfg.prompt, text);

    let client = Client::builder()
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))?;

    let req = ChatRequest {
        model: &cfg.model,
        messages: vec![ChatMessage {
            role: "user",
            content: &user_content,
        }],
    };

    let url = format!("{}/chat/completions", GROQ_BASE_URL);
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Groq HTTP send failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Groq API {status}: {body}"));
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("Groq response parse failed: {e}"))?;

    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Groq returned empty content".into())
}

/// Convenience wrapper used at synthesis sites. Returns the input unchanged
/// when post-processing is disabled or on any failure; logs a warning so
/// problems are visible without breaking the spoken output.
pub async fn try_process(text: String, cfg: &PostProcessConfig) -> String {
    if !cfg.enabled {
        return text;
    }
    match process(&text, cfg).await {
        Ok(processed) => {
            log::info!(
                "[PostProcess] {} chars -> {} chars",
                text.len(),
                processed.len()
            );
            processed
        }
        Err(e) => {
            warn!("[PostProcess] failed, using original text: {e}");
            text
        }
    }
}

/// Substitute `${output}` in the prompt template with the actual text. If the
/// template has no placeholder, append the text on a new line so the model
/// still sees it.
fn build_prompt(template: &str, text: &str) -> String {
    if template.contains("${output}") {
        template.replace("${output}", text)
    } else {
        format!("{}\n\n{}", template, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_substitutes_placeholder() {
        let out = build_prompt("Rewrite: ${output} end", "BODY");
        assert_eq!(out, "Rewrite: BODY end");
    }

    #[test]
    fn build_prompt_appends_when_no_placeholder() {
        let out = build_prompt("Rewrite", "BODY");
        assert_eq!(out, "Rewrite\n\nBODY");
    }
}
