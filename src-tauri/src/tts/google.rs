// Google Gemini TTS backend (native Rust HTTP).
//
// Targets the Gemini generateContent API with AUDIO response modality, not the
// legacy Google Cloud Text-to-Speech service. The API returns base64-encoded
// signed 16-bit little-endian PCM (typically 24kHz mono), which we wrap into WAV.

use super::{TtsBackend, TtsError};
use crate::config::GoogleTtsConfig;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GoogleTtsBackend {
    config: GoogleTtsConfig,
}

impl GoogleTtsBackend {
    pub fn new(config: GoogleTtsConfig) -> Self {
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

    /// Parse the PCM sample rate out of a Gemini mime type like
    /// "audio/L16;codec=pcm;rate=24000". Falls back to 24000 if absent.
    fn rate_from_mime(mime: &str) -> u32 {
        mime.split(';')
            .filter_map(|p| p.trim().strip_prefix("rate="))
            .find_map(|r| r.parse::<u32>().ok())
            .unwrap_or(24000)
    }
}

/// Wrap raw signed-16-bit-LE mono PCM into a minimal WAV container.
pub fn pcm16_to_wav(pcm: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = channels * (bits_per_sample / 8);
    let data_len = pcm.len() as u32;
    let riff_len = 36 + data_len;

    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

impl TtsBackend for GoogleTtsBackend {
    fn name(&self) -> &str {
        "Google Gemini"
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        let url = format!("{}/{}:generateContent", GEMINI_BASE, self.config.model);
        let body = json!({
            "contents": [{ "parts": [{ "text": text }] }],
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": { "voiceName": voice }
                    }
                }
            }
        });

        log::info!(
            "Google TTS request - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        let start_time = std::time::Instant::now();
        let api_key =
            crate::secrets::resolve(&self.config.api_key, &["GEMINI_API_KEY", "GOOGLE_API_KEY"]);

        let response = Self::block_on_async(async {
            let client = Client::new();
            client
                .post(&url)
                .header("x-goog-api-key", api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
        })
        .map_err(|e| {
            log::error!(
                "Google TTS request failed after {:?}: {}",
                start_time.elapsed(),
                e
            );
            TtsError::Http(format!("Request failed: {}", e))
        })?;

        let status = response.status();
        log::info!(
            "Google TTS response: {} (took {:?})",
            status.as_u16(),
            start_time.elapsed()
        );

        let text_body = Self::block_on_async(async { response.text().await })
            .map_err(|e| TtsError::Http(format!("Failed to read body: {}", e)))?;

        if !status.is_success() {
            log::error!("Google API error {}: {}", status, text_body);
            return Err(TtsError::Http(format!(
                "Google API error {}: {}",
                status, text_body
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&text_body)
            .map_err(|e| TtsError::Http(format!("Invalid JSON response: {}", e)))?;

        let part = parsed
            .pointer("/candidates/0/content/parts/0/inlineData")
            .ok_or_else(|| TtsError::Http("No audio in Google response".into()))?;

        let data_b64 = part
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| TtsError::Http("Missing inlineData.data".into()))?;
        let mime = part.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");

        let pcm = general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| TtsError::Http(format!("Base64 decode failed: {}", e)))?;

        let wav = pcm16_to_wav(&pcm, Self::rate_from_mime(mime), 1);
        log::info!("Google TTS synthesis complete: {} WAV bytes", wav.len());
        Ok(wav)
    }

    fn health_check(&self) -> Result<(), TtsError> {
        if crate::secrets::resolve(&self.config.api_key, &["GEMINI_API_KEY", "GOOGLE_API_KEY"])
            .trim()
            .is_empty()
        {
            return Err(TtsError::Unavailable("Google API key is missing".into()));
        }
        Ok(())
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm16_to_wav_header() {
        let pcm = vec![0u8; 8];
        let wav = pcm16_to_wav(&pcm, 24000, 1);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 8);
    }

    #[test]
    fn test_rate_from_mime() {
        assert_eq!(
            GoogleTtsBackend::rate_from_mime("audio/L16;codec=pcm;rate=24000"),
            24000
        );
        assert_eq!(
            GoogleTtsBackend::rate_from_mime("audio/L16;rate=16000"),
            16000
        );
        assert_eq!(GoogleTtsBackend::rate_from_mime("audio/wav"), 24000);
    }
}
