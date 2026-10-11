// LLM post-processing: sends sanitized text through the active post-process
// provider's OpenAI-compatible chat completions API to produce a concise,
// listener-friendly rewrite before TTS.
//
// Failure policy: this module never silently swallows errors. `process()`
// surfaces them; `try_process()` is the single fallback wrapper that callers
// in the synthesis pipeline use to keep TTS running on LLM failure.

use crate::config::{PostProcessConfig, PostProcessProvider, SanitizationConfig};
use log::warn;
use reqwest::header::{HeaderValue, AUTHORIZATION};
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

/// Run the configured prompt against `text` using the active provider's
/// OpenAI-compatible endpoint. Returns `Ok(processed)` on success. Caller is
/// responsible for the fallback — this never silently returns the original.
pub async fn process(text: &str, cfg: &PostProcessConfig) -> Result<String, String> {
    let provider = cfg
        .active_provider()
        .ok_or_else(|| "No post-process provider configured".to_string())?;
    let api_key = crate::secrets::resolve(&provider.api_key, &["POST_PROCESS_API_KEY"]);
    // Ollama and other local endpoints legitimately run without a key.
    if api_key.trim().is_empty() && !is_local_provider(provider) {
        return Err(format!("{} API key is empty", provider.label));
    }
    if provider.model.trim().is_empty() {
        return Err(format!("{} model is empty", provider.label));
    }
    if provider.base_url.trim().is_empty() {
        return Err(format!("{} base URL is empty", provider.label));
    }

    let user_content = build_prompt(&cfg.prompt, text);

    // timeout_secs > 0 bounds the whole request; 0 = never (the
    // daemon_idle_timeout_secs convention). Without a timeout a stalled
    // provider hangs the speak path indefinitely.
    let builder = if cfg.timeout_secs > 0 {
        Client::builder().timeout(std::time::Duration::from_secs(cfg.timeout_secs))
    } else {
        Client::builder()
    };
    let client = builder
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))?;

    let req = ChatRequest {
        model: &provider.model,
        messages: vec![ChatMessage {
            role: "user",
            content: &user_content,
        }],
    };

    let url = format!("{}/chat/completions", provider.base_url.trim());
    let mut builder = client.post(&url);
    if !api_key.trim().is_empty() {
        builder = builder.header(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
                .map_err(|e| format!("Invalid API key header: {e}"))?,
        );
    }
    let resp = builder
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("{} HTTP send failed: {e}", provider.label))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("{} API {status}: {body}", provider.label));
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("{} response parse failed: {e}", provider.label))?;

    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{} returned empty content", provider.label))
}

/// Local runtimes (loopback addresses) don't need credentials; anything else
/// is treated as a hosted API that requires a key.
fn is_local_provider(provider: &PostProcessProvider) -> bool {
    provider.base_url.contains("localhost") || provider.base_url.contains("127.0.0.1")
}

/// Whether this request wants condensing. `require_request_flag: false`
/// (global mode) condenses every qualifying text; `true` condenses only
/// callers that explicitly set the flag (control server `condense`, CLI
/// `--condense`).
fn condense_requested(cfg: &PostProcessConfig, request_flag: bool) -> bool {
    !cfg.require_request_flag || request_flag
}

/// Convenience wrapper used at synthesis sites. Returns the input unchanged
/// when post-processing is disabled, gated off (length/request flag), or on
/// any failure; logs a warning so problems are visible without breaking the
/// spoken output. The sanitized input is re-sanitized on the LLM OUTPUT: the
/// condenser can reintroduce markup/symbols, and its text goes straight to
/// the engine.
pub async fn try_process(
    text: String,
    cfg: &PostProcessConfig,
    sanitization: &SanitizationConfig,
    request_flag: bool,
) -> String {
    if !cfg.enabled || !condense_requested(cfg, request_flag) {
        return text;
    }
    if text.chars().count() < cfg.min_chars {
        return text;
    }
    match process(&text, cfg).await {
        Ok(processed) => {
            let processed = crate::sanitize::sanitize_text(&processed, sanitization);
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

    #[test]
    fn condense_requested_global_mode_ignores_flag() {
        let cfg = PostProcessConfig {
            require_request_flag: false,
            ..Default::default()
        };
        assert!(condense_requested(&cfg, false));
        assert!(condense_requested(&cfg, true));
    }

    #[test]
    fn condense_requested_strict_mode_needs_flag() {
        let cfg = PostProcessConfig {
            require_request_flag: true,
            ..Default::default()
        };
        assert!(!condense_requested(&cfg, false));
        assert!(condense_requested(&cfg, true));
    }

    /// Below min_chars the condenser must not run: short text returns
    /// verbatim with no network call (the test text is under 200 chars and
    /// the provider would fail loudly if contacted).
    #[tokio::test]
    async fn try_process_skips_text_below_min_chars() {
        let cfg = PostProcessConfig {
            enabled: true,
            min_chars: 200,
            ..Default::default()
        };
        let sanitization = SanitizationConfig::default();
        let out = try_process("short clip".to_string(), &cfg, &sanitization, true).await;
        assert_eq!(out, "short clip");
    }

    #[tokio::test]
    async fn try_process_noop_when_disabled() {
        let cfg = PostProcessConfig {
            enabled: false,
            ..Default::default()
        };
        let sanitization = SanitizationConfig::default();
        let text = "x".repeat(500);
        let out = try_process(text.clone(), &cfg, &sanitization, true).await;
        assert_eq!(out, text);
    }
}
