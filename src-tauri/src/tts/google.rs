// Google Gemini TTS backend (native Rust HTTP).
//
// Targets the Gemini generateContent API with AUDIO response modality, not the
// legacy Google Cloud Text-to-Speech service. The API returns base64 audio in
// inlineData — headerless signed 16-bit little-endian PCM (typically 24kHz
// mono) on 2.5/3.1, a complete WAV file on 3.8 — which we normalize into a
// single WAV container.

use super::block_on_async;
use super::stream::{spawn_stream, AudioFormatMeta, ChunkItem, ChunkStream};
use super::{TtsBackend, TtsError};
use crate::config::GoogleTtsConfig;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Gemini TTS emits 24 kHz mono PCM. The exact rate only arrives with the first
/// SSE event's mimeType, and `ChunkStream` needs its format up front, so the
/// documented rate is assumed and a mismatch is logged rather than silently
/// played at the wrong speed.
const GEMINI_PCM_SAMPLE_RATE: u32 = 24000;

pub struct GoogleTtsBackend {
    config: GoogleTtsConfig,
}

impl GoogleTtsBackend {
    pub fn new(config: GoogleTtsConfig) -> Self {
        Self { config }
    }

    /// Parse the PCM sample rate out of a Gemini mime type like
    /// "audio/L16;codec=pcm;rate=24000". Falls back to 24000 if absent.
    fn rate_from_mime(mime: &str) -> u32 {
        mime.split(';')
            .filter_map(|p| p.trim().strip_prefix("rate="))
            .find_map(|r| r.parse::<u32>().ok())
            .unwrap_or(GEMINI_PCM_SAMPLE_RATE)
    }

    /// Pull the base64 audio out of one `streamGenerateContent` SSE payload.
    /// Returns `None` for events that carry no audio part (Gemini interleaves
    /// metadata-only events), which are skipped rather than treated as errors.
    fn audio_from_sse_event(payload: &str) -> Option<(Vec<u8>, u32)> {
        let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;
        let part = parsed.pointer("/candidates/0/content/parts/0/inlineData")?;
        let data_b64 = part.get("data")?.as_str()?;
        let mime = part.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
        let pcm = general_purpose::STANDARD.decode(data_b64).ok()?;
        Some((pcm, Self::rate_from_mime(mime)))
    }

    /// Split an SSE byte stream into complete `data:` payloads, keeping any
    /// trailing partial line in `buffer` for the next chunk.
    fn drain_sse_payloads(buffer: &mut String) -> Vec<String> {
        let mut payloads = Vec::new();
        while let Some(newline) = buffer.find('\n') {
            let line: String = buffer.drain(..=newline).collect();
            if let Some(data) = line.trim_end().strip_prefix("data:") {
                let data = data.trim();
                if !data.is_empty() && data != "[DONE]" {
                    payloads.push(data.to_string());
                }
            }
        }
        payloads
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

/// The system direction anchoring TTS behavior: the model must read the
/// request text verbatim instead of treating clipboard content that reads
/// like a question or command as a prompt to answer — the deprecated 2.5
/// preview answered such text and 400'd with "should only be used for TTS".
const READ_ALOUD_INSTRUCTION: &str = "Read the user's message aloud verbatim as speech. Do not answer, reply, or add anything; the audio must contain only the message itself.";

/// Pre-3.8 preview TTS models ignore `systemInstruction`, so their direction
/// must ride inline in the text itself. 3.8 and later would SPEAK an inline
/// direction, so it must stay out of the text there.
fn needs_inline_read_aloud_prefix(model: &str) -> bool {
    model.contains("preview-tts")
}

/// Build the generateContent request body shared by the unary and streaming
/// paths. Framing is verified by request-shape tests; live model behavior is
/// not (that needs a live probe, deferred with #69).
fn tts_request_body(model: &str, text: &str, voice: &str) -> serde_json::Value {
    let spoken = if needs_inline_read_aloud_prefix(model) {
        format!("Read the following text aloud verbatim: {text}")
    } else {
        text.to_string()
    };
    json!({
        "contents": [{ "parts": [{ "text": spoken }] }],
        "systemInstruction": { "parts": [{ "text": READ_ALOUD_INSTRUCTION }] },
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {
                "voiceConfig": {
                    "prebuiltVoiceConfig": { "voiceName": voice }
                }
            }
        }
    })
}

/// The unary reply's audio bytes: a reply that already carries its own WAV
/// header passes through unwrapped (wrapping again produces a broken nested
/// container), headerless PCM gets the mono 16-bit header as before.
fn unary_audio_bytes(decoded: Vec<u8>, mime: &str) -> Vec<u8> {
    if decoded.starts_with(b"RIFF") {
        log::info!(
            "Google TTS: unary reply already carries a WAV header; passing through ({} bytes)",
            decoded.len()
        );
        decoded
    } else {
        pcm16_to_wav(&decoded, GoogleTtsBackend::rate_from_mime(mime), 1)
    }
}

impl TtsBackend for GoogleTtsBackend {
    fn name(&self) -> &str {
        "Google Gemini"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn synthesize_streaming(&self, text: &str, voice: &str) -> Result<ChunkStream, TtsError> {
        let api_key =
            crate::secrets::resolve(&self.config.api_key, &["GEMINI_API_KEY", "GOOGLE_API_KEY"]);
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("Google API key is missing".into()));
        }
        // `alt=sse` turns streamGenerateContent's JSON array into server-sent
        // events, so audio parts can be decoded as they land instead of after
        // the whole array has been buffered.
        let url = format!(
            "{}/{}:streamGenerateContent?alt=sse",
            GEMINI_BASE, self.config.model
        );
        let body = tts_request_body(&self.config.model, text, voice);

        log::info!(
            "Google TTS stream - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        let meta = AudioFormatMeta {
            sample_rate: GEMINI_PCM_SAMPLE_RATE,
            channels: 1,
            bits_per_sample: 16,
        };
        Ok(spawn_stream(meta, move |sink| async move {
            let mut response = Client::new()
                .post(&url)
                .header("x-goog-api-key", api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    TtsError::Http(format!(
                        "Google stream request failed: {}",
                        super::http::flatten_error(&e)
                    ))
                })?;
            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(TtsError::Http(format!(
                    "Google API error {status}: {error_text}"
                )));
            }

            let mut buffer = String::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| {
                TtsError::Http(format!(
                    "Google stream read failed: {}",
                    super::http::flatten_error(&e)
                ))
            })? {
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                for payload in Self::drain_sse_payloads(&mut buffer) {
                    let Some((pcm, rate)) = Self::audio_from_sse_event(&payload) else {
                        continue;
                    };
                    if rate != GEMINI_PCM_SAMPLE_RATE {
                        log::warn!(
                            "[TTS] Gemini returned {rate} Hz audio but the stream was opened at {GEMINI_PCM_SAMPLE_RATE} Hz; playback will be off-pitch"
                        );
                    }
                    if !sink.send(ChunkItem::Pcm(pcm)) {
                        // Consumer dropped (stop/abort): stop pulling.
                        return Ok(());
                    }
                }
            }
            Ok(())
        }))
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        let url = format!("{}/{}:generateContent", GEMINI_BASE, self.config.model);
        let body = tts_request_body(&self.config.model, text, voice);

        log::info!(
            "Google TTS request - model: {}, voice: {}, text length: {} chars",
            self.config.model,
            voice,
            text.len()
        );

        let start_time = std::time::Instant::now();
        let api_key =
            crate::secrets::resolve(&self.config.api_key, &["GEMINI_API_KEY", "GOOGLE_API_KEY"]);

        let response = block_on_async(async {
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
            TtsError::Http(format!(
                "Request failed: {}",
                super::http::flatten_error(&e)
            ))
        })?;

        let status = response.status();
        log::info!(
            "Google TTS response: {} (took {:?})",
            status.as_u16(),
            start_time.elapsed()
        );

        let text_body = block_on_async(async { response.text().await }).map_err(|e| {
            TtsError::Http(format!(
                "Failed to read body: {}",
                super::http::flatten_error(&e)
            ))
        })?;

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

        let wav = unary_audio_bytes(pcm, mime);
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

    #[test]
    fn drain_sse_payloads_keeps_partial_lines_for_the_next_chunk() {
        let mut buffer = String::from("data: {\"a\":1}\n\ndata: {\"b\"");
        assert_eq!(
            GoogleTtsBackend::drain_sse_payloads(&mut buffer),
            vec!["{\"a\":1}".to_string()]
        );
        // The half-received event stays buffered until its newline arrives.
        assert_eq!(buffer, "data: {\"b\"");

        buffer.push_str(":2}\n");
        assert_eq!(
            GoogleTtsBackend::drain_sse_payloads(&mut buffer),
            vec!["{\"b\":2}".to_string()]
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn audio_from_sse_event_skips_events_without_audio() {
        assert!(GoogleTtsBackend::audio_from_sse_event("{\"candidates\":[]}").is_none());
        assert!(GoogleTtsBackend::audio_from_sse_event("not json").is_none());

        let event = serde_json::json!({
            "candidates": [{ "content": { "parts": [{ "inlineData": {
                "mimeType": "audio/L16;codec=pcm;rate=24000",
                "data": general_purpose::STANDARD.encode([1u8, 2, 3, 4]),
            }}]}}]
        })
        .to_string();
        let (pcm, rate) = GoogleTtsBackend::audio_from_sse_event(&event).expect("audio part");
        assert_eq!(pcm, vec![1, 2, 3, 4]);
        assert_eq!(rate, 24000);
    }

    #[test]
    fn request_body_anchors_verbatim_read_aloud_and_keeps_text_clean() {
        let body = tts_request_body("gemini-3.8-flash-tts", "What time is it?", "Kore");
        // The anchor lives in systemInstruction...
        assert_eq!(
            body.pointer("/systemInstruction/parts/0/text"),
            Some(&serde_json::json!(READ_ALOUD_INSTRUCTION))
        );
        // ...and 3.8's text stays verbatim: an inline direction would be spoken.
        assert_eq!(
            body.pointer("/contents/0/parts/0/text")
                .and_then(|t| t.as_str()),
            Some("What time is it?")
        );
        assert_eq!(
            body.pointer(
                "/generationConfig/speechConfig/voiceConfig/prebuiltVoiceConfig/voiceName"
            )
            .and_then(|v| v.as_str()),
            Some("Kore")
        );
        assert_eq!(
            body.pointer("/generationConfig/responseModalities/0")
                .and_then(|v| v.as_str()),
            Some("AUDIO")
        );
    }

    #[test]
    fn legacy_preview_models_get_the_direction_inline() {
        // Preview TTS models ignore systemInstruction and would otherwise read
        // clipboard content as a prompt — the 400 the fork hit.
        let body = tts_request_body("gemini-2.5-flash-preview-tts", "Close the door", "Kore");
        let spoken = body
            .pointer("/contents/0/parts/0/text")
            .and_then(|t| t.as_str())
            .expect("text part");
        assert!(spoken.starts_with("Read the following text aloud verbatim: "));
        assert!(spoken.ends_with("Close the door"));
    }

    #[test]
    fn unary_reply_with_its_own_wav_header_passes_through_unwrapped() {
        let headered = pcm16_to_wav(&[1, 2, 3, 4], 24000, 1);
        let out = unary_audio_bytes(headered.clone(), "audio/wav;rate=24000");
        assert_eq!(
            out, headered,
            "a reply that already carries a WAV header must not be wrapped again"
        );
    }

    #[test]
    fn headerless_pcm_still_gets_a_wav_header() {
        let out = unary_audio_bytes(vec![1, 2, 3, 4], "audio/L16;codec=pcm;rate=24000");
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(out.len(), 44 + 4);
    }
}
