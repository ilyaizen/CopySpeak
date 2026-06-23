// Generic HTTP-serving TTS backend.
//
// Points at any local or remote TTS server (e.g. an OpenAI-compatible
// /audio/speech endpoint, or a Chatterbox HTTP server). The URL, method,
// headers, and JSON body are templated from config. Intentionally NOT a full
// request-templating language: a server with an exotic contract should expose a
// normalizing wrapper instead of growing this backend.
//
// Supported placeholders: {text}, {raw_text}, {voice}, {speed}.

use super::{TtsBackend, TtsError};
use crate::config::HttpTtsConfig;
use reqwest::Client;
use std::time::Duration;

pub struct HttpTtsBackend {
    config: HttpTtsConfig,
}

impl HttpTtsBackend {
    pub fn new(config: HttpTtsConfig) -> Self {
        Self { config }
    }

    fn block_on_async<F, T>(f: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(f),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(f)
            }
        }
    }

    fn fill(template: &str, text: &str, voice: &str, speed: f32) -> String {
        template
            .replace("{text}", text)
            .replace("{raw_text}", text)
            .replace("{voice}", voice)
            .replace("{speed}", &speed.to_string())
    }
}

impl TtsBackend for HttpTtsBackend {
    fn name(&self) -> &str {
        "HTTP"
    }

    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        if self.config.url_template.trim().is_empty() {
            return Err(TtsError::Unavailable(
                "HTTP TTS url is not configured".into(),
            ));
        }

        let url = Self::fill(&self.config.url_template, text, voice, speed);
        let body = self
            .config
            .body_template
            .as_ref()
            .map(|b| Self::fill(b, text, voice, speed));

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

        let response = Self::block_on_async(async {
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
                .map_err(|e| TtsError::Http(format!("Request failed: {}", e)))
        })?;

        let status = response.status();
        log::info!(
            "HTTP TTS response: {} (took {:?})",
            status.as_u16(),
            start_time.elapsed()
        );

        let bytes = Self::block_on_async(async { response.bytes().await })
            .map_err(|e| TtsError::Http(format!("Failed to read bytes: {}", e)))?;

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
        let out = HttpTtsBackend::fill(
            r#"{"input":"{text}","voice":"{voice}","speed":{speed}}"#,
            "hello",
            "amy",
            1.25,
        );
        assert_eq!(out, r#"{"input":"hello","voice":"amy","speed":1.25}"#);
    }
}
