// LLM post-processing: sends sanitized text through the active post-process
// provider's OpenAI-compatible chat completions API before TTS. The active
// template's class drives the pipeline: `preserve` renditions keep every
// word and add emote tags (Dramatize semantics, tag-capable engines only);
// `rewrite` templates may change words and run on any engine.
//
// Failure policy: this module never silently swallows errors. `process()`
// surfaces them; `try_process()` is the single fallback wrapper that callers
// in the synthesis pipeline use to keep TTS running on LLM failure.

use crate::config::{
    PostProcessConfig, PostProcessProvider, PostProcessTemplateClass, SanitizationConfig,
};
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
pub async fn process(text: &str, prompt: &str, cfg: &PostProcessConfig) -> Result<String, String> {
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

    let user_content = build_prompt(prompt, text);

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

/// Whether this request wants post-processing. `require_request_flag: false`
/// (global mode) processes every qualifying text; `true` processes only
/// callers that explicitly set the flag (control server `post_process`, CLI
/// `--post-process`).
fn post_process_requested(cfg: &PostProcessConfig, request_flag: bool) -> bool {
    !cfg.require_request_flag || request_flag
}

/// The prompt for this reading, resolved from the active template's class.
/// Preserve-class reads speak the engine's own tag dialect when it has one
/// (xAI, Mistral): the generic bracket-dialect prompt produces tags those
/// engines would read aloud. Rewrite templates always run their own prompt —
/// their output is plain spoken text with no dialect to match.
fn active_prompt<'a>(cfg: &'a PostProcessConfig, gate: &PostProcessGate) -> &'a str {
    match cfg.active_template() {
        Some(t) if t.class == PostProcessTemplateClass::Preserve => {
            gate.prompt_override.unwrap_or(t.prompt.as_str())
        }
        Some(t) => t.prompt.as_str(),
        None => "",
    }
}

/// Convenience wrapper used at synthesis sites. Returns the input unchanged
/// when post-processing is disabled, gated off (experimental flag, engine
/// support for preserve-class templates, length/request flag), or on any
/// failure. Gate blocks that override a WANTED run (experimental off,
/// unsupported engine/model) log at info so a skipped run is never silent;
/// feature-off and unrequested skips are the feature working as configured
/// and stay quiet.
///
/// Sanitization branches on the active template's class: preserve output
/// goes straight to tag-aware engines, so its emote markup must survive
/// (tag-safe variant). Rewrite output is plain spoken text — brackets in it
/// are literal and the standard pipeline sanitizer strips them like any
/// other input.
pub async fn try_process(
    text: String,
    cfg: &PostProcessConfig,
    sanitization: &SanitizationConfig,
    request_flag: bool,
    gate: PostProcessGate,
) -> String {
    if let Some(reason) = post_process_skip_reason(cfg, request_flag, text.chars().count(), &gate) {
        if reason.loud() {
            log::info!("[PostProcess] post-process skipped for this reading: {reason:?}");
        }
        return text;
    }
    let preserve =
        cfg.active_template().map(|t| t.class) == Some(PostProcessTemplateClass::Preserve);
    let prompt = active_prompt(cfg, &gate);
    match process(&text, prompt, cfg).await {
        Ok(processed) => {
            // Preserve-class only: the LLM may tag but never reword. A
            // rewrite template changes words by design.
            if preserve && !words_preserved(&text, &processed, gate.angle_wraps) {
                warn!("[PostProcess] LLM output changed the spoken words; using the original text");
                return text;
            }
            let processed = if preserve {
                crate::sanitize::sanitize_text_tag_safe(&processed, sanitization)
            } else {
                crate::sanitize::sanitize_text(&processed, sanitization)
            };
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

/// Per-reading inputs the synthesis sites resolve once: the Experimental
/// master switch, and whether the active (engine, model) pair takes bracket
/// tags (see `tts::catalog::supports_emote_tags`).
#[derive(Debug, Clone)]
pub struct PostProcessGate {
    pub experimental_enabled: bool,
    pub emote_supported: bool,
    /// The engine's own Dramatize prompt dialect (`catalog::
    /// dramatize_prompt_override`); `None` keeps the template's prompt.
    /// Applied to preserve-class templates only.
    pub prompt_override: Option<&'static str>,
    /// The engine's dialect also interprets `<...>` wraps (xAI): the
    /// word-comparison and caption paths must strip them, not read them as
    /// spoken words.
    pub angle_wraps: bool,
}

/// Why a reading will not be post-processed. `ExperimentalOff` and
/// `Unsupported` are loud: post-processing was enabled and wanted to run,
/// and the gate blocked it — that must never look like a silent no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkipReason {
    Disabled,
    NotRequested,
    TooShort,
    ExperimentalOff,
    Unsupported,
}

impl SkipReason {
    /// Only gate blocks are logged; feature-off and unrequested are the
    /// feature working as configured.
    fn loud(&self) -> bool {
        matches!(self, SkipReason::ExperimentalOff | SkipReason::Unsupported)
    }
}

/// The config-level decision for one reading: `None` = run, `Some` = skip
/// with the reason. An explicit request (`request_flag`, the Post-process
/// switch or the control server's `post_process` field) bypasses
/// `min_chars`; global mode keeps the length gate. Synthesis sites also call
/// this to decide whether the waiting HUD must show before the LLM round
/// trip.
pub(crate) fn post_process_skip_reason(
    cfg: &PostProcessConfig,
    request_flag: bool,
    text_len: usize,
    gate: &PostProcessGate,
) -> Option<SkipReason> {
    if !cfg.enabled {
        return Some(SkipReason::Disabled);
    }
    if !post_process_requested(cfg, request_flag) {
        return Some(SkipReason::NotRequested);
    }
    if !request_flag && text_len < cfg.min_chars {
        return Some(SkipReason::TooShort);
    }
    if !gate.experimental_enabled {
        return Some(SkipReason::ExperimentalOff);
    }
    // The emote gate protects preserve-class reads only: their output feeds
    // tag-aware engines. A rewrite template produces plain text any engine
    // can speak.
    if !gate.emote_supported
        && cfg.active_template().map(|t| t.class) == Some(PostProcessTemplateClass::Preserve)
    {
        return Some(SkipReason::Unsupported);
    }
    None
}

/// True when `tagged` keeps every word of `original`: lowercase alphanumeric
/// runs compared in order, with `[...]` emote tags, added punctuation, and
/// emphasis capitalization ignored. `angle_wraps` additionally ignores
/// `<...>` style tags (the xAI dialect's `<whisper>`-family wraps), whose
/// names would otherwise read as added words. Guards against the LLM
/// rewriting, truncating, or padding the text — tags may be inserted, words
/// may not. Applied to preserve-class templates only.
pub(crate) fn words_preserved(original: &str, tagged: &str, angle_wraps: bool) -> bool {
    let strip = |text: &str| -> String {
        if angle_wraps {
            crate::emote::strip_tags_and_angle_wraps(text)
        } else {
            crate::emote::strip_tags(text)
        }
    };
    fn words(text: &str) -> Vec<String> {
        text.chars()
            .fold(Vec::new(), |mut runs, ch| {
                if ch.is_alphanumeric() {
                    runs.last_mut()
                        .map(|last: &mut String| last.push(ch))
                        .unwrap_or_else(|| runs.push(ch.to_string()));
                }
                runs
            })
            .into_iter()
            .map(|run| run.to_lowercase())
            .collect()
    }
    words(&strip(original)) == words(&strip(tagged))
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
    use crate::config::CONCISE_TEMPLATE_ID;

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
    fn post_process_requested_global_mode_ignores_flag() {
        let cfg = PostProcessConfig {
            require_request_flag: false,
            ..Default::default()
        };
        assert!(post_process_requested(&cfg, false));
        assert!(post_process_requested(&cfg, true));
    }

    #[test]
    fn post_process_requested_strict_mode_needs_flag() {
        let cfg = PostProcessConfig {
            require_request_flag: true,
            ..Default::default()
        };
        assert!(!post_process_requested(&cfg, false));
        assert!(post_process_requested(&cfg, true));
    }

    fn permissive_gate() -> PostProcessGate {
        PostProcessGate {
            experimental_enabled: true,
            emote_supported: true,
            prompt_override: None,
            angle_wraps: false,
        }
    }

    #[test]
    fn post_process_runs_when_every_condition_holds() {
        let cfg = PostProcessConfig {
            enabled: true,
            ..Default::default()
        };
        assert!(post_process_skip_reason(&cfg, false, 1000, &permissive_gate()).is_none());
        assert!(post_process_skip_reason(&cfg, true, 1000, &permissive_gate()).is_none());
    }

    #[test]
    fn preserve_template_names_its_block_reasons() {
        // Default config: the active template is Dramatize (preserve class).
        let cfg = PostProcessConfig {
            enabled: true,
            ..Default::default()
        };
        let experimental_off = PostProcessGate {
            experimental_enabled: false,
            emote_supported: true,
            prompt_override: None,
            angle_wraps: false,
        };
        assert!(matches!(
            post_process_skip_reason(&cfg, false, 1000, &experimental_off),
            Some(SkipReason::ExperimentalOff)
        ));
        let unsupported = PostProcessGate {
            experimental_enabled: true,
            emote_supported: false,
            prompt_override: None,
            angle_wraps: false,
        };
        assert!(matches!(
            post_process_skip_reason(&cfg, false, 1000, &unsupported),
            Some(SkipReason::Unsupported)
        ));
    }

    /// A rewrite template (Concise) produces plain text any engine can
    /// speak: the emote gate does not block it.
    #[test]
    fn rewrite_template_skips_the_emote_gate() {
        let cfg = PostProcessConfig {
            enabled: true,
            active_template_id: CONCISE_TEMPLATE_ID.to_string(),
            ..Default::default()
        };
        let gate = PostProcessGate {
            experimental_enabled: true,
            emote_supported: false,
            prompt_override: Some("engine dialect"),
            angle_wraps: false,
        };
        assert!(post_process_skip_reason(&cfg, false, 1000, &gate).is_none());
        assert!(post_process_skip_reason(&cfg, true, 1000, &gate).is_none());
    }

    #[test]
    fn post_process_off_and_unrequested_stay_silent_skips() {
        let gate = permissive_gate();
        let disabled = PostProcessConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(matches!(
            post_process_skip_reason(&disabled, false, 1000, &gate),
            Some(SkipReason::Disabled)
        ));
        let strict = PostProcessConfig {
            enabled: true,
            require_request_flag: true,
            ..Default::default()
        };
        assert!(matches!(
            post_process_skip_reason(&strict, false, 1000, &gate),
            Some(SkipReason::NotRequested)
        ));
    }

    /// `min_chars` gates implicit requests only: an explicit request bypasses
    /// it (the Post-process switch speaks short clips too).
    #[test]
    fn explicit_requests_bypass_min_chars() {
        let gate = permissive_gate();
        let cfg = PostProcessConfig {
            enabled: true,
            min_chars: 500,
            ..Default::default()
        };
        assert!(matches!(
            post_process_skip_reason(&cfg, false, 100, &gate),
            Some(SkipReason::TooShort)
        ));
        assert!(post_process_skip_reason(&cfg, true, 100, &gate).is_none());
    }

    /// Preserve-class: the engine's tag-dialect override replaces the
    /// template's prompt. Rewrite-class: the override is ignored — the
    /// template's own prompt runs.
    #[test]
    fn prompt_override_applies_to_preserve_class_only() {
        let mut cfg = PostProcessConfig::default();
        cfg.normalize();
        let gate = PostProcessGate {
            experimental_enabled: true,
            emote_supported: true,
            prompt_override: Some("ENGINE DIALECT"),
            angle_wraps: false,
        };
        // Default active template is Dramatize (preserve).
        assert_eq!(active_prompt(&cfg, &gate), "ENGINE DIALECT");
        // Switch to Concise (rewrite): the override must NOT apply.
        cfg.active_template_id = CONCISE_TEMPLATE_ID.to_string();
        assert_eq!(
            active_prompt(&cfg, &gate),
            cfg.templates
                .iter()
                .find(|t| t.id == CONCISE_TEMPLATE_ID)
                .unwrap()
                .prompt
        );
    }

    #[test]
    fn words_preserved_accepts_tags_emphasis_and_punctuation() {
        assert!(words_preserved(
            "Hello world, again.",
            "[sighs] Hello [happy] WORLD, again!",
            false
        ));
        assert!(words_preserved(
            "Are you serious? I can't believe you did that!",
            "[appalled] Are you serious? [sighs] I can't believe you did that!",
            false
        ));
        assert!(words_preserved("", "", false));
    }

    #[test]
    fn words_preserved_rejects_rewrites_truncations_and_additions() {
        assert!(!words_preserved("Hello world", "Hello brave world", false));
        assert!(!words_preserved("Hello world again", "Hello world", false));
        assert!(!words_preserved("Hello world", "Hello earth", false));
        assert!(!words_preserved("Calm text", "[sighs]", false));
        assert!(!words_preserved("Hello", "", false));
    }

    /// The xAI dialect's `<...>` wraps are tags, not words: without the angle
    /// flag the wrap names read as added words and the output would always be
    /// discarded; with it they pass.
    #[test]
    fn words_preserved_handles_angle_wraps_only_when_asked() {
        assert!(!words_preserved(
            "Are you serious?",
            "Are you serious? <whisper>I can't believe it</whisper>",
            false
        ));
        assert!(words_preserved(
            "Are you serious? I can't believe it",
            "Are you serious? <whisper>I can't believe it</whisper>",
            true
        ));
    }

    /// Below min_chars post-processing must not run: short text returns
    /// verbatim with no network call (the test text is short and the
    /// provider would fail loudly if contacted).
    #[tokio::test]
    async fn try_process_skips_text_below_min_chars() {
        let cfg = PostProcessConfig {
            enabled: true,
            min_chars: 200,
            ..Default::default()
        };
        let sanitization = SanitizationConfig::default();
        let out = try_process(
            "short clip".to_string(),
            &cfg,
            &sanitization,
            false,
            permissive_gate(),
        )
        .await;
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
        let out = try_process(text.clone(), &cfg, &sanitization, false, permissive_gate()).await;
        assert_eq!(out, text);
    }

    /// Gate blocked (experimental off): the text passes through without an
    /// LLM round trip, loudly logged in production.
    #[tokio::test]
    async fn try_process_skips_when_experimental_is_off() {
        let cfg = PostProcessConfig {
            enabled: true,
            ..Default::default()
        };
        let gate = PostProcessGate {
            experimental_enabled: false,
            emote_supported: true,
            prompt_override: None,
            angle_wraps: false,
        };
        let out = try_process(
            "[sighs] untouched".to_string(),
            &cfg,
            &SanitizationConfig::default(),
            false,
            gate,
        )
        .await;
        assert_eq!(out, "[sighs] untouched");
    }

    /// The LLM rewrote the words under a preserve template: the output is
    /// discarded and the original text is spoken instead.
    #[tokio::test]
    async fn words_preserved_passes_tag_only_and_fails_word_diffs() {
        // words_preserved is pure; this pins the contract try_process relies
        // on for preserve-class templates: a tag-only diff passes, any word
        // diff fails. The discard branch inside try_process itself needs a
        // provider seam and is not covered here.
        assert!(words_preserved("one two", "[happy] one two", false));
        assert!(!words_preserved("one two", "one three", false));
    }
}
