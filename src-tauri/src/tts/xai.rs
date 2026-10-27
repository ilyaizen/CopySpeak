use super::block_on_async;
use super::captions::{CaptionAlignment, SpeechAudio, WordTiming};
use super::{TtsBackend, TtsError, Voice};
use crate::config::XaiConfig;
use crate::text_map::tokenize;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const XAI_TTS_URL: &str = "https://api.x.ai/v1/tts";
const XAI_VOICES_URL: &str = "https://api.x.ai/v1/tts/voices";

/// Synthesis speed bounds from the xAI docs (1.0 = normal speech).
const XAI_SPEED_RANGE: (f32, f32) = (0.7, 1.5);
/// Hardcoded output container (v1 decision): 16-bit WAV at 24 kHz.
const XAI_SAMPLE_RATE: u32 = 24000;

pub struct XaiTtsBackend {
    config: XaiConfig,
}

impl XaiTtsBackend {
    pub fn new(config: XaiConfig) -> Self {
        Self { config }
    }

    /// Resolve the API key: config first, then the `XAI_API_KEY` env var.
    fn api_key(&self) -> String {
        crate::secrets::resolve(&self.config.api_key, &["XAI_API_KEY"])
    }

    /// The batch request body. v1 pins `language: "auto"` (BCP-47 detection;
    /// note: xAI lists 20 languages and none is Hebrew) and WAV @ 24 kHz, and
    /// always asks for character timestamps — the native captions the engine
    /// is advertised with. Empty voice = omit the field; the API defaults to
    /// `eve`.
    fn request_body(&self, text: &str) -> serde_json::Value {
        let mut body = json!({
            "text": text,
            "language": "auto",
            "with_timestamps": true,
            "output_format": {
                "codec": "wav",
                "sample_rate": XAI_SAMPLE_RATE,
            },
        });
        let voice = self.config.voice_id.trim();
        if !voice.is_empty() {
            body["voice_id"] = json!(voice);
        }
        if let Some(speed) = self.config.speed {
            body["speed"] = json!(speed.clamp(XAI_SPEED_RANGE.0, XAI_SPEED_RANGE.1));
        }
        body
    }

    /// Synthesize and return the audio bytes plus native captions when the
    /// envelope carried character timings.
    fn synthesize_full(&self, text: &str) -> Result<(Vec<u8>, Option<CaptionAlignment>), TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("xAI API key is missing".into()));
        }

        let body = self.request_body(text);

        log::info!(
            "xAI TTS request - voice: {}, speed: {}, text length: {} chars",
            if self.config.voice_id.trim().is_empty() {
                "(default eve)"
            } else {
                &self.config.voice_id
            },
            self.config
                .speed
                .map(|s| format!("{s:.2}"))
                .unwrap_or_else(|| "1.00".into()),
            text.len()
        );

        if crate::logging::is_debug_mode() {
            log::debug!(
                "xAI TTS request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let start_time = std::time::Instant::now();

        // The whole request AND its body read live in one block_on_async call:
        // with no ambient runtime each call builds a fresh runtime, and a
        // Response carried across calls loses its connection driver.
        let (bytes, captions) = block_on_async(async move {
            let client = Client::new();
            let response = client
                .post(XAI_TTS_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    TtsError::Http(format!(
                        "Request failed: {}",
                        super::http::flatten_error(&e)
                    ))
                })?;

            let status = response.status();
            log::info!(
                "xAI TTS response: {} {} (took {:?})",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                start_time.elapsed()
            );

            // Same block: read the envelope text while the connection is live.
            let body_text = response.text().await.map_err(|e| {
                TtsError::Http(format!(
                    "Failed to read response body: {}",
                    super::http::flatten_error(&e)
                ))
            })?;

            if !status.is_success() {
                log::error!("xAI API error {}: {}", status, body_text);
                return Err(TtsError::Http(format!(
                    "xAI API error {}: {}",
                    status, body_text
                )));
            }

            let envelope: TtsEnvelope = serde_json::from_str(&body_text)
                .map_err(|e| TtsError::Http(format!("Failed to parse xAI TTS envelope: {e}")))?;
            let mut bytes = STANDARD
                .decode(envelope.audio.as_bytes())
                .map_err(|e| TtsError::Http(format!("Invalid xAI audio payload: {e}")))?;
            // xAI writes streaming placeholders (data = i32::MAX, RIFF size
            // with the sign bit set). Browsers clamp and play fine, but every
            // in-app decoder and history file needs honest sizes.
            crate::audio::wav::clamp_chunk_sizes_to_file(&mut bytes);
            log::info!(
                "xAI TTS synthesis complete: {} bytes, {} chars of timing data",
                bytes.len(),
                envelope
                    .audio_timestamps
                    .as_ref()
                    .map(|t| t.graph_chars.len())
                    .unwrap_or(0)
            );
            let captions = envelope
                .audio_timestamps
                .and_then(|t| captions_from_timestamps(&t.graph_chars, &t.graph_times));
            Ok::<_, TtsError>((bytes, captions))
        })?;

        Ok((bytes, captions))
    }

    /// List the built-in voices. Custom voices are covered by manual voice-ID
    /// entry (v1 scope); `GET /v1/custom-voices` exists but is not wired.
    pub fn list_voices(&self) -> Result<Vec<Voice>, TtsError> {
        let api_key = self.api_key();
        if api_key.trim().is_empty() {
            return Err(TtsError::Unavailable("xAI API key is missing".into()));
        }
        let (status, body) = block_on_async(async move {
            let client = Client::new();
            let response = client
                .get(XAI_VOICES_URL)
                .bearer_auth(api_key)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| {
                    TtsError::Http(format!(
                        "Failed to fetch voices: {}",
                        super::http::flatten_error(&e)
                    ))
                })?;
            let status = response.status();
            let body = response.text().await.map_err(|e| {
                TtsError::Http(format!(
                    "Failed to read voices response: {}",
                    super::http::flatten_error(&e)
                ))
            })?;
            Ok::<_, TtsError>((status, body))
        })?;

        if !status.is_success() {
            return Err(TtsError::Http(format!(
                "xAI API error {}: {}",
                status,
                body.chars().take(300).collect::<String>()
            )));
        }

        #[derive(Deserialize)]
        struct VoicePage {
            voices: Vec<VoiceEntry>,
        }
        #[derive(Deserialize)]
        struct VoiceEntry {
            voice_id: String,
            name: String,
            #[serde(default)]
            language: Option<String>,
        }

        let page: VoicePage = serde_json::from_str(&body)
            .map_err(|e| TtsError::Http(format!("Failed to parse voices response: {}", e)))?;
        Ok(page
            .voices
            .into_iter()
            .map(|v| Voice {
                id: v.voice_id,
                name: v.name,
                language: v.language,
                default: None,
            })
            .collect())
    }
}

impl TtsBackend for XaiTtsBackend {
    fn name(&self) -> &str {
        "xAI"
    }

    fn synthesize(&self, text: &str, _voice: &str) -> Result<Vec<u8>, TtsError> {
        self.synthesize_full(text).map(|(bytes, _)| bytes)
    }

    /// Native captions: the API echoes the request text (speech tags
    /// included) back as per-character timings, so the alignment is exact —
    /// only the tag characters need stripping before use.
    fn synthesize_with_captions(&self, text: &str, voice: &str) -> Result<SpeechAudio, TtsError> {
        let (bytes, captions) = self.synthesize_full(text)?;
        // An envelope without timings still plays; it just carries no captions.
        let captions = match captions {
            Some(alignment) => {
                let stripped = alignment.stripped_of_all_emote_tags();
                match stripped.validate() {
                    Ok(()) => Some(stripped),
                    Err(error) => {
                        log::warn!("Dropping invalid xAI captions: {error}");
                        None
                    }
                }
            }
            None => None,
        };
        let _ = voice;
        Ok(SpeechAudio { bytes, captions })
    }

    fn health_check(&self) -> Result<(), TtsError> {
        log::debug!("xAI TTS health check - validating API key");

        if self.api_key().trim().is_empty() {
            log::error!("xAI TTS health check failed - API key is missing");
            return Err(TtsError::Unavailable("xAI API key is missing".into()));
        }

        log::debug!("xAI TTS health check passed");
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "wav"
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        voice_id.to_lowercase()
    }
}

/// The `with_timestamps: true` response envelope: base64 audio plus
/// per-character timings over the echoed request text.
#[derive(Deserialize)]
struct TtsEnvelope {
    audio: String,
    #[serde(default)]
    audio_timestamps: Option<AudioTimestamps>,
}

#[derive(Deserialize)]
struct AudioTimestamps {
    graph_chars: Vec<String>,
    graph_times: Vec<GraphTime>,
}

/// xAI's own docs disagree on this field's shape: the REST reference's
/// response example carries objects (`{"start": 0.0, "end": 0.06}`) while the
/// guide text describes parallel `[start, end]` pairs. Accept both — the
/// untagged enum costs nothing and keeps captions alive on either wire form.
#[derive(Deserialize)]
#[serde(untagged)]
enum GraphTime {
    Object { start: f64, end: f64 },
    Pair([f64; 2]),
}

impl GraphTime {
    fn start(&self) -> f64 {
        match self {
            GraphTime::Object { start, .. } | GraphTime::Pair([start, _]) => *start,
        }
    }
    fn end(&self) -> f64 {
        match self {
            GraphTime::Object { end, .. } | GraphTime::Pair([_, end]) => *end,
        }
    }
}

/// Bucket per-character timings into word timings over the echoed text. Words
/// are the same alphanumeric runs `text_map::tokenize` produces for the
/// estimation path, so a highlighted unit matches the browser bridge.
/// Returns `None` when the arrays disagree or no word survives.
fn captions_from_timestamps(chars: &[String], times: &[GraphTime]) -> Option<CaptionAlignment> {
    if chars.len() != times.len() {
        log::warn!(
            "xAI timing arrays disagree: {} chars vs {} times; dropping captions",
            chars.len(),
            times.len()
        );
        return None;
    }
    let text = chars.concat();
    let (token_spans, _) = tokenize(&text);
    if token_spans.is_empty() {
        return None;
    }

    // Per-character UTF-16 spans, ascending and contiguous.
    let mut char_spans: Vec<(usize, usize)> = Vec::with_capacity(chars.len());
    let mut offset = 0usize;
    for character in chars {
        let next = offset + character.encode_utf16().count();
        if next == offset {
            return None; // zero-width entry would misalign every later char
        }
        char_spans.push((offset, next));
        offset = next;
    }

    let mut words: Vec<WordTiming> = Vec::with_capacity(token_spans.len());
    let mut char_index = 0usize;
    for (token_start, token_end) in token_spans {
        while char_index < char_spans.len() && char_spans[char_index].1 <= token_start {
            char_index += 1;
        }
        let first = char_index;
        let mut last = char_index;
        while last + 1 < char_spans.len() && char_spans[last + 1].0 < token_end {
            last += 1;
        }
        if first >= char_spans.len() || char_spans[last].0 >= token_end {
            continue; // token covers no timed character (should not happen)
        }
        words.push(WordTiming {
            text_start: token_start,
            text_end: token_end,
            start_ms: times[first].start() * 1000.0,
            end_ms: times[last].end() * 1000.0,
        });
    }

    let alignment = CaptionAlignment { text, words };
    if let Err(error) = alignment.validate() {
        log::warn!("xAI native captions failed validation: {error}");
        return None;
    }
    Some(alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(voice_id: &str) -> XaiTtsBackend {
        XaiTtsBackend::new(XaiConfig {
            voice_id: voice_id.into(),
            ..Default::default()
        })
    }

    #[test]
    fn body_pins_language_output_format_and_timestamps() {
        let body = backend("eve").request_body("hello");
        assert_eq!(body["text"], "hello");
        assert_eq!(body["language"], "auto");
        assert_eq!(body["with_timestamps"], true);
        assert_eq!(body["output_format"]["codec"], "wav");
        assert_eq!(body["output_format"]["sample_rate"], 24000);
    }

    #[test]
    fn body_omits_voice_id_for_the_default_voice() {
        // Empty voice = the API's own default (`eve`); sending it blank would
        // ask for a voice with an empty id.
        let body = backend("").request_body("hello");
        assert!(body.get("voice_id").is_none());
        let body = backend("ara").request_body("hello");
        assert_eq!(body["voice_id"], "ara");
    }

    #[test]
    fn speed_is_omitted_when_unset_and_clamped_when_set() {
        assert!(backend("eve").request_body("hi").get("speed").is_none());
        let body = backend("eve").request_body("hi");
        let backend = XaiTtsBackend::new(XaiConfig {
            speed: Some(2.5),
            ..Default::default()
        });
        assert_eq!(backend.request_body("hi")["speed"], serde_json::json!(1.5));
        let _ = body;
    }

    #[test]
    fn timestamps_bucket_into_ordered_valid_words() {
        let chars: Vec<String> = "Hello, world!".chars().map(String::from).collect();
        let times: Vec<GraphTime> = (0..chars.len())
            .map(|i| GraphTime::Object {
                start: i as f64 * 0.05,
                end: (i + 1) as f64 * 0.05,
            })
            .collect();
        let captions = captions_from_timestamps(&chars, &times).expect("valid alignment");
        captions.validate().unwrap();
        let words = ["Hello", "world"];
        assert_eq!(captions.words.len(), words.len());
        assert_eq!(captions.words[0].start_ms, 0.0);
        // Tokens are alphanumeric runs: "Hello" spans chars 0..=4 and ends
        // where the last covered char (o) ends; the comma is no token.
        assert!((captions.words[0].end_ms - 250.0).abs() < 1e-9);
        assert_eq!(captions.words[1].text_start, 7);
    }

    #[test]
    fn pair_form_of_graph_times_is_accepted() {
        let chars: Vec<String> = vec!["H".into(), "i".into()];
        let times = vec![GraphTime::Pair([0.0, 0.1]), GraphTime::Pair([0.1, 0.2])];
        let captions = captions_from_timestamps(&chars, &times).expect("pair form parses");
        // "Hi" is one token covering both chars: it ends where i ends.
        assert_eq!(captions.words[0].end_ms, 200.0);
    }

    #[test]
    fn disagreeing_arrays_yield_no_captions() {
        let chars = vec!["H".to_string()];
        let times: Vec<GraphTime> = Vec::new();
        assert!(captions_from_timestamps(&chars, &times).is_none());
    }

    #[test]
    fn echoed_emote_tags_are_stripped_from_native_captions() {
        // The API echoes speech tags — bracket inline tags and <whisper>-
        // style wraps — back in graph_chars. They are interpreted, never
        // spoken, so they must not be highlighted.
        let raw = "<whisper>[pause] quiet hello";
        let chars: Vec<String> = raw.chars().map(String::from).collect();
        let times: Vec<GraphTime> = (0..chars.len())
            .map(|i| GraphTime::Object {
                start: i as f64 * 0.1,
                end: (i + 1) as f64 * 0.1,
            })
            .collect();
        let captions = captions_from_timestamps(&chars, &times).expect("valid alignment");
        let stripped = captions.stripped_of_all_emote_tags();
        stripped.validate().unwrap();
        // Span removal keeps the surrounding text verbatim (offsets must stay
        // exact), so the space that preceded the tags remains — the ElevenLabs
        // decoder shows the same " Hi" shape.
        assert_eq!(stripped.text, " quiet hello");
        let units: Vec<u16> = stripped.text.encode_utf16().collect();
        for word in &stripped.words {
            let text = String::from_utf16_lossy(&units[word.text_start..word.text_end]);
            assert!(
                !text.contains('[') && !text.contains('<'),
                "tag fragment {text:?} leaked into captions"
            );
        }
    }

    #[test]
    fn envelope_without_timestamps_still_decodes_audio() {
        let wire = serde_json::json!({
            "audio": STANDARD.encode(b"RIFF----"),
            "content_type": "audio/wav",
            "duration": 0.5
        });
        let envelope: TtsEnvelope = serde_json::from_value(wire).unwrap();
        assert!(envelope.audio_timestamps.is_none());
        assert_eq!(STANDARD.decode(envelope.audio).unwrap(), b"RIFF----");
    }

    /// Live synthesis against api.x.ai on the default voice. Opt-in: needs
    /// `XAI_API_KEY` in the environment (e.g. exported from
    /// `target/debug/.env`); runs only via `cargo test xai_live -- --ignored`.
    /// Tiny text (~$0.001 at $15/1M chars).
    #[test]
    #[ignore = "live API test; needs XAI_API_KEY in the environment"]
    fn xai_live_synthesizes_with_native_captions() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(api_key) = std::env::var("XAI_API_KEY") else {
            eprintln!("XAI_API_KEY not set; skipping live synthesis test");
            return Ok(());
        };
        let backend = XaiTtsBackend::new(XaiConfig {
            api_key,
            ..Default::default()
        });
        let speech = backend
            .synthesize_with_captions("Hello from the CopySpeak live test.", "eve")
            .expect("live synthesis must succeed");
        assert!(
            speech.bytes.len() > 5_000,
            "suspiciously small audio: {}",
            speech.bytes.len()
        );
        assert_eq!(&speech.bytes[..4], b"RIFF", "expected a WAV container");
        let captions = speech
            .captions
            .expect("with_timestamps must produce native captions");
        captions.validate()?;
        assert!(!captions.words.is_empty());
        assert!(!captions.text.contains('['));
        Ok(())
    }
}
