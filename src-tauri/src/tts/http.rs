// Generic HTTP-serving TTS backend.
//
// Points at any local or remote TTS server (e.g. an OpenAI-compatible
// /audio/speech endpoint). The URL, method,
// headers, and JSON body are templated from config. Intentionally NOT a full
// request-templating language: a server with an exotic contract should expose a
// normalizing wrapper instead of growing this backend.
//
// Supported placeholders: {text}, {raw_text}, {voice}.

use super::block_on_async;
use super::{TtsBackend, TtsError};
use crate::config::HttpTtsConfig;
use reqwest::Client;
use std::time::Duration;

/// Flatten a reqwest error's `source()` chain into the message.
///
/// reqwest's `Display` prints only the error kind ("error sending request");
/// the transient cause (timeout, refused, reset) hides in `source()`. The
/// retry classifier matches phrases, so every provider request and
/// response-read site stringifies reqwest errors through this.
pub fn flatten_error(err: &(dyn std::error::Error + 'static)) -> String {
    let mut message = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

pub struct HttpTtsBackend {
    config: HttpTtsConfig,
}

impl HttpTtsBackend {
    pub fn new(config: HttpTtsConfig) -> Self {
        Self { config }
    }

    fn fill(template: &str, text: &str, voice: &str) -> String {
        template
            .replace("{text}", text)
            .replace("{raw_text}", text)
            .replace("{voice}", voice)
    }
}

impl TtsBackend for HttpTtsBackend {
    fn name(&self) -> &str {
        "HTTP"
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        if self.config.url_template.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "HTTP TTS url is not configured".into(),
            ));
        }

        let url = Self::fill(&self.config.url_template, text, voice);
        let body = self
            .config
            .body_template
            .as_ref()
            .map(|b| Self::fill(b, text, voice));

        log::info!(
            "HTTP TTS request - {} {}, text length: {} chars",
            self.config.method,
            url,
            text.len()
        );

        let start_time = std::time::Instant::now();
        let method = self.config.method.to_uppercase();
        let headers = self.config.headers.clone();
        let timeout = Duration::from_secs(self.config.timeout_secs.max(1));

        let response = block_on_async(async {
            let client = Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| TtsError::Http(format!("Failed to build HTTP client: {}", e)))?;
            let mut req = match method.as_str() {
                "GET" => client.get(&url),
                _ => client.post(&url),
            };
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            if let Some(ref b) = body {
                req = req
                    .header("Content-Type", "application/json")
                    .body(b.clone());
            }
            req.send()
                .await
                .map_err(|e| TtsError::Http(format!("Request failed: {}", flatten_error(&e))))
        })?;

        let status = response.status();
        log::info!(
            "HTTP TTS response: {} (took {:?})",
            status.as_u16(),
            start_time.elapsed()
        );

        let bytes = block_on_async(async { response.bytes().await })
            .map_err(|e| TtsError::Http(format!("Failed to read bytes: {}", flatten_error(&e))))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&bytes);
            log::error!("HTTP TTS error {}: {}", status, error_text);
            return Err(TtsError::Http(format!(
                "HTTP TTS error {}: {}",
                status, error_text
            )));
        }

        if bytes.is_empty() {
            return Err(TtsError::OutputNotFound(
                "HTTP TTS returned no audio bytes".into(),
            ));
        }

        Ok(bytes.to_vec())
    }

    fn health_check(&self) -> Result<(), TtsError> {
        if self.config.url_template.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "HTTP TTS url is not configured".into(),
            ));
        }
        Ok(())
    }

    fn file_extension(&self) -> &str {
        match self.config.response_format.as_str() {
            "" => "wav",
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_placeholders() {
        let out = HttpTtsBackend::fill(r#"{"input":"{text}","voice":"{voice}"}"#, "hello", "amy");
        assert_eq!(out, r#"{"input":"hello","voice":"amy"}"#);
    }

    /// Two-level cause chain: reqwest hides timeouts and refused connections
    /// behind `source()`, and the retry classifier matches those phrases.
    #[derive(Debug)]
    struct ChainedError(&'static str, Option<Box<ChainedError>>);

    impl std::fmt::Display for ChainedError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for ChainedError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.1
                .as_ref()
                .map(|c| c as &(dyn std::error::Error + 'static))
        }
    }

    #[test]
    fn flatten_error_includes_the_whole_source_chain() {
        let err = ChainedError(
            "error sending request",
            Some(Box::new(ChainedError("operation timed out", None))),
        );
        assert_eq!(
            flatten_error(&err),
            "error sending request: operation timed out"
        );
        let bare = ChainedError("error sending request", None);
        assert_eq!(flatten_error(&bare), "error sending request");
    }
}
