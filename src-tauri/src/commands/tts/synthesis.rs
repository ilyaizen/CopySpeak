// TTS synthesis commands — speak_now, speak_queued, speak_history_entry.

use crate::audio::{self, AudioPlayer};
use crate::browser_bridge::{self, SpeakTrigger, TriggerSource};
use crate::config::{self, AppConfig, EffectId};
use crate::fragment_queue::FragmentQueue;
use crate::history::{self, HistoryLog};
use crate::hud;
use crate::pagination;
use crate::telemetry;
use crate::tts::captions::{read_sidecar, write_sidecar, CaptionAlignment, SpeechAudio};
use crate::tts::stream::{pcm_to_wav, AudioFormatMeta, ChunkItem, ChunkStream};
use crate::tts::TtsBackend;
use crate::AbortScope;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

use super::helpers::{
    active_engine, active_playback_speed, create_backend, create_backend_from_effective,
    engine_identifier, engine_str, post_process_gate_for_effective, resolve_effective,
    resolve_effective_for_profile, voice_display_name, SynthesisGuard,
};
use crate::commands::{
    AudioFragmentEvent, AudioStreamChunkEvent, CachedAudio, PaginationEvent,
    PlaybackStreamFailedEvent,
};

// ── Helper Functions ──────────────────────────────────────────────────────────

fn post_process_for_profile(
    config: &crate::config::PostProcessConfig,
    profile: &crate::config::ProfilePostProcess,
) -> crate::config::PostProcessConfig {
    let mut effective = config.clone();
    if let Some(id) = profile.active_template_id.as_ref() {
        if config.templates.iter().any(|template| &template.id == id) {
            effective.active_template_id = id.clone();
        }
    }
    effective
}

/// Truncate text for preview display
fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

/// Get text from parameter or clipboard
fn get_text_or_clipboard(text: Option<String>) -> Result<String, String> {
    match text {
        Some(t) => Ok(t),
        None => {
            crate::clipboard::get_clipboard_text().ok_or_else(|| "No text in clipboard".to_string())
        }
    }
}

/// Log debug info for TTS operations
fn log_tts_debug(tag: &str, backend: &str, text: &str) {
    if crate::logging::is_debug_mode() {
        let text_preview: String = text.chars().take(100).collect();
        log::debug!("[{}] called (Backend: {:?})", tag, backend);
        log::debug!("[{}] Text length: {} chars", tag, text.len());
        log::debug!(
            "[{}] Text preview: {:?}{}",
            tag,
            text_preview,
            if text.len() > 100 { "..." } else { "" }
        );
    }
}

/// Synthesize audio using spawn_blocking, retrying transient failures.
///
/// Bounded: two retries after the initial attempt, 500 ms / 1 s backoff, and
/// the abort scope is polled during backoff. The batch path never emits PCM
/// before returning, so a retry cannot duplicate audio the user already
/// heard; the streaming path (which does) is never retried.
async fn synthesize_async(
    backend: Arc<Box<dyn TtsBackend>>,
    text: String,
    voice: String,
    scope: AbortScope,
) -> Result<SpeechAudio, String> {
    let result = retry_synthesis(
        || {
            let backend = backend.clone();
            let text = text.clone();
            let voice = voice.clone();
            async move {
                tokio::task::spawn_blocking(move || backend.synthesize_with_captions(&text, &voice))
                    .await
                    .map_err(|e| format!("Task join error: {e}"))?
                    .map_err(|e| e.to_string())
            }
        },
        || scope.aborted(),
        // A backend that just told us the provider refused this request keeps
        // that verdict: retrying amplifies throttling rather than recovering.
        || !backend.failure_is_final(),
        Duration::from_millis(500),
    )
    .await;
    if scope.aborted() {
        // The abort paths downstream key on empty bytes plus the scope, so a
        // cancelled retry surfaces as "nothing synthesized", not an error.
        Ok(Vec::new().into())
    } else {
        result
    }
}

/// Wording for a section that could not be synthesized.
///
/// Pure so it can be asserted directly. The reading loop returns after this
/// error, so this section and every later one are never spoken; wording that
/// only describes the *already synthesized* fragments would read as though
/// the whole passage had been read.
fn section_failure_message(index: usize, total: usize, error: &str) -> String {
    if total <= 1 {
        format!("Reading failed: {error}")
    } else if index.saturating_add(1) >= total {
        // The final section failed, so nothing follows it; claiming "the rest
        // of the passage was not read" would describe text that does not exist.
        // `saturating_add` keeps a defensive index from panicking in here.
        format!(
            "Reading stopped: the final section ({} of {}) failed. {error}",
            index.saturating_add(1),
            total
        )
    } else {
        format!(
            "Reading stopped: section {} of {} failed, so the rest of the passage was not read. {error}",
            index + 1,
            total
        )
    }
}

/// Report a permanently failed reading section once, globally.
///
/// Emits `reading-failed` naming the failed section and returns the message
/// for the caller to propagate. Gated on the abort scope so an aborted
/// reading is never reported as a failure.
fn report_section_failure(
    app: &AppHandle,
    index: usize,
    total: usize,
    scope: AbortScope,
    error: String,
) -> String {
    let message = section_failure_message(index, total, &error);
    if !scope.aborted() {
        log::error!("[TTS] {message}");
        let _ = app.emit("reading-failed", &message);
    }
    message
}

fn transient_synthesis_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    // Every condition is evaluated; a permanent code must not short-circuit the
    // scan. edge-tts reports its real failure as
    // `NoAudioReceived("No audio was received...")` next to the `403` from the
    // refused handshake. A first-token-wins form would read that `403`,
    // return "permanent", and never reach the phrase list below — so the
    // transient no-audio failure would never be retried.
    let retryable_status =
        (message.contains("http") || message.contains("status") || message.contains("api error"))
            && message
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|token| matches!(token, "429" | "500" | "502" | "503" | "504"));
    if retryable_status {
        return true;
    }
    [
        "timeout",
        "timed out",
        "connection reset",
        "connection closed",
        "connection aborted",
        "connection refused",
        "no audio was received",
        "daemon closed its output",
        // reqwest's own transport failures. Its `Display` prints only the error
        // kind ("error sending request"), so a timeout or a cut connection is
        // recognised by the kind even when the reason chain is unavailable.
        "error sending request",
        "request or response body error",
    ]
    .iter()
    .any(|reason| message.contains(reason))
}

async fn retry_synthesis<T, Operation, Attempt, Cancelled, CanRetry>(
    mut operation: Operation,
    is_cancelled: Cancelled,
    can_retry: CanRetry,
    retry_delay: Duration,
) -> Result<T, String>
where
    Operation: FnMut() -> Attempt,
    Attempt: std::future::Future<Output = Result<T, String>>,
    Cancelled: Fn() -> bool,
    CanRetry: Fn() -> bool,
{
    for attempt in 0..3 {
        if is_cancelled() {
            return Err("Reading cancelled".into());
        }
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if is_cancelled() {
                    return Err("Reading cancelled".into());
                }
                if attempt == 2 || !can_retry() || !transient_synthesis_error(&error) {
                    return Err(error);
                }
                log::warn!(
                    "[TTS] Transient synthesis failure; retry {}/2: {}",
                    attempt + 1,
                    error
                );
                let deadline = tokio::time::Instant::now() + retry_delay * (attempt + 1);
                while tokio::time::Instant::now() < deadline {
                    if is_cancelled() {
                        return Err("Reading cancelled".into());
                    }
                    tokio::time::sleep(
                        Duration::from_millis(50)
                            .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
                    )
                    .await;
                }
            }
        }
    }
    unreachable!("retry_synthesis returns within 3 attempts")
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn missing_profile_template_uses_global_selection() {
        let mut config = crate::config::PostProcessConfig::default();
        config.active_template_id = "concise".into();
        let mut profile = crate::config::ProfilePostProcess::default();
        profile.active_template_id = Some("deleted".into());
        assert_eq!(
            post_process_for_profile(&config, &profile).active_template_id,
            "concise"
        );
        profile.active_template_id = Some("dramatize".into());
        assert_eq!(
            post_process_for_profile(&config, &profile).active_template_id,
            "dramatize"
        );
    }

    #[tokio::test]
    async fn retries_transient_errors_without_skipping_text() {
        let attempts = AtomicUsize::new(0);
        let result = retry_synthesis(
            || {
                std::future::ready(if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err("HTTP error: 503 Service Unavailable".into())
                } else {
                    Ok("complete section")
                })
            },
            || false,
            || true,
            Duration::ZERO,
        )
        .await;
        assert_eq!(result.unwrap(), "complete section");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn permanent_and_partially_played_failures_are_not_retried() {
        for (error, can_retry) in [("401 Unauthorized", true), ("HTTP 503", false)] {
            let attempts = AtomicUsize::new(0);
            let result: Result<(), String> = retry_synthesis(
                || {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    std::future::ready(Err(error.into()))
                },
                || false,
                || can_retry,
                Duration::ZERO,
            )
            .await;
            assert_eq!(result.unwrap_err(), error);
            assert_eq!(attempts.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn retries_are_bounded_and_cancellation_prevents_another_attempt() {
        let attempts = AtomicUsize::new(0);
        let result: Result<(), String> = retry_synthesis(
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                std::future::ready(Err("HTTP 429".into()))
            },
            || false,
            || true,
            Duration::ZERO,
        )
        .await;
        assert_eq!(result.unwrap_err(), "HTTP 429");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        let cancelled = AtomicBool::new(false);
        let result: Result<(), String> = retry_synthesis(
            || {
                cancelled.store(true, Ordering::SeqCst);
                std::future::ready(Err("HTTP 503".into()))
            },
            || cancelled.load(Ordering::SeqCst),
            || true,
            Duration::ZERO,
        )
        .await;
        assert_eq!(result.unwrap_err(), "Reading cancelled");
    }

    #[tokio::test]
    async fn cancellation_interrupts_backoff_before_another_request() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let cancel_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            signal.store(true, Ordering::SeqCst);
        });
        let attempts = AtomicUsize::new(0);
        let result: Result<(), String> = tokio::time::timeout(
            Duration::from_secs(1),
            retry_synthesis(
                || {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    std::future::ready(Err("HTTP 503".into()))
                },
                || cancelled.load(Ordering::SeqCst),
                || true,
                Duration::from_secs(60),
            ),
        )
        .await
        .unwrap();
        cancel_task.await.unwrap();
        assert_eq!(result.unwrap_err(), "Reading cancelled");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn status_classification_does_not_retry_permanent_or_incidental_numbers() {
        // A permanent HTTP status is not retried on its own.
        assert!(!transient_synthesis_error(
            "HTTP 401 Unauthorized: invalid API key"
        ));
        assert!(!transient_synthesis_error(
            "HTTP 403: voice model not found"
        ));
        assert!(!transient_synthesis_error(
            "voice model 50300 was not found"
        ));
        assert!(!transient_synthesis_error(
            "invalid transcript contains 503"
        ));
        assert!(transient_synthesis_error(
            "HTTP error: Google API error 503 Service Unavailable"
        ));
        assert!(transient_synthesis_error(
            "WSServerHandshakeError: 503 Invalid response status"
        ));
    }

    #[test]
    fn transient_signal_beats_a_coincidental_permanent_status() {
        // The exact shape of the fork user's Edge failure: the refused
        // handshake's 403 precedes the real no-audio error, and must not mask it.
        assert!(transient_synthesis_error(
            "edge-tts exit exit status: 1: NoAudioReceived(\"No audio was received. \
             Please verify that your parameters are correct.\") status=403"
        ));
        // A retryable status counts wherever it appears in the string.
        assert!(transient_synthesis_error(
            "status 403 handled; upstream returned 503"
        ));
        // A provider status error need not say "http" or "status".
        assert!(transient_synthesis_error(
            "ElevenLabs API error 503 Service Unavailable: overloaded"
        ));
        // reqwest reports transport failures as a bare error kind.
        assert!(transient_synthesis_error(
            "Request failed: error sending request"
        ));
        assert!(transient_synthesis_error(
            "Streaming synthesis failed mid-stream: request or response body error"
        ));
        assert!(transient_synthesis_error(
            "Request failed: error sending request: operation timed out"
        ));
        // A response-body read failure keeps its cause in the flattened chain.
        assert!(transient_synthesis_error(
            "Failed to read bytes: request or response body error: connection reset by peer"
        ));
        // A local CLI `exit status: 1` is not itself a transient signal.
        assert!(!transient_synthesis_error(
            "edge-tts exit exit status: 1: ERROR: voice model not found"
        ));
    }

    #[test]
    fn failure_message_states_that_the_rest_was_not_read() {
        let many = section_failure_message(3, 10, "HTTP 503 Service Unavailable");
        assert!(many.contains("section 4 of 10"));
        assert!(many.contains("the rest of the passage was not read"));
        // A single-section reading has no "section 1 of 1" to report.
        assert_eq!(
            section_failure_message(0, 1, "HTTP 503"),
            "Reading failed: HTTP 503"
        );
    }
}

/// Extract envelope with default fallback
fn extract_envelope_or_default(wav_bytes: &[u8]) -> audio::AmplitudeEnvelope {
    audio::extract_envelope(wav_bytes, 40).unwrap_or_else(|_| audio::AmplitudeEnvelope {
        values: vec![0.5; 40],
        duration_ms: 2000,
    })
}

/// Record telemetry sample
fn record_telemetry(
    telemetry_state: &State<'_, Mutex<telemetry::TelemetryLog>>,
    engine: &str,
    voice: &str,
    text_len: usize,
    duration_ms: u64,
) {
    telemetry::record_sample(telemetry_state, engine, voice, text_len, duration_ms);
}

/// Add history entry with synthesis metadata
fn add_history_with_metadata(
    history: &State<'_, Mutex<HistoryLog>>,
    text: &str,
    engine: &str,
    voice: &str,
    duration_ms: u64,
    path: Option<String>,
    batch_id: Option<String>,
    synthesis_ms: u64,
    extra_metadata: Option<HashMap<String, serde_json::Value>>,
) {
    let mut metadata = extra_metadata.unwrap_or_default();
    metadata.insert("synthesis_ms".to_string(), serde_json::json!(synthesis_ms));
    history::add_entry_with_batch(
        history,
        text,
        engine,
        voice,
        duration_ms,
        path,
        batch_id,
        metadata,
    );
}

/// Emit audio-ready event with base64 encoded audio
fn emit_audio_ready(
    app: &AppHandle,
    speech: &SpeechAudio,
    text: &str,
    session_id: Option<&str>,
    scope: AbortScope,
) {
    emit_audio_fragment(app, speech, 0, 1, text.to_owned(), session_id, scope);
}

/// Emit audio-fragment-ready event for streaming playback
fn emit_audio_fragment(
    app: &AppHandle,
    speech: &SpeechAudio,
    index: usize,
    total: usize,
    text: String,
    session_id: Option<&str>,
    scope: AbortScope,
) {
    let wav_bytes = &speech.bytes;
    if scope.aborted() {
        return;
    }
    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(wav_bytes);
    let is_final = index == total - 1;
    // Emit under speaker ownership: a scoped stop for this reading either
    // lands after this audio (and stops it) or before it (and this emit sees
    // the cancelled scope and sends nothing).
    crate::speak_requests::claim_playback(
        scope.request(),
        || scope.aborted(),
        || {
            if let Err(e) = app.emit(
                "audio-fragment-ready",
                AudioFragmentEvent {
                    audio_base64: encoded,
                    fragment_index: index,
                    fragment_total: total,
                    is_final,
                    text,
                    captions: speech.captions.clone(),
                    session_id: session_id.map(str::to_owned),
                },
            ) {
                log::warn!("Failed to emit audio-fragment-ready: {}", e);
            }
        },
    );
}

/// Emit one PCM chunk of streaming synthesis audio to the frontend.
/// A terminal zero-byte chunk with `is_final: true` marks end-of-stream.
fn emit_audio_stream_chunk(
    app: &AppHandle,
    meta: &AudioFormatMeta,
    pcm: &[u8],
    fragment_index: usize,
    is_final: bool,
    text: Option<&str>,
    fragment_duration_ms: Option<u64>,
    captions: Option<&CaptionAlignment>,
    session_id: Option<&str>,
) {
    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(pcm);
    let event = AudioStreamChunkEvent {
        audio_base64: encoded,
        sample_rate: meta.sample_rate,
        channels: meta.channels,
        bits_per_sample: meta.bits_per_sample,
        fragment_index,
        is_final,
        text: text.map(str::to_owned),
        fragment_duration_ms,
        captions: captions.cloned(),
        session_id: session_id.map(str::to_owned),
    };
    if let Err(e) = app.emit("audio-stream-chunk", event) {
        log::warn!("Failed to emit audio-stream-chunk: {}", e);
    }
}

/// Drain a [`ChunkStream`], forwarding each PCM chunk to `on_chunk` while
/// accumulating them into a single WAV container (for history/cache/HUD).
///
/// `on_chunk(chunk_index, pcm, is_final)` fires once per received chunk plus a
/// terminal `(n, &[], true)` end-of-stream marker. Returns the wrapped WAV
/// bytes and time-to-first-audio in milliseconds.
fn drain_chunk_stream(
    stream: ChunkStream,
    scope: AbortScope,
    mut on_chunk: impl FnMut(usize, &[u8], bool, Option<&CaptionAlignment>),
) -> Result<(SpeechAudio, u64), String> {
    let start = Instant::now();
    let meta = stream.meta.clone();
    let mut accumulated: Vec<u8> = Vec::new();
    let mut chunk_index = 0usize;
    let mut captions = None;
    let mut ttfa_ms: Option<u64> = None;

    loop {
        // Dropping the receiver stops each producer at its next send. A
        // producer blocked on a stalled upstream read only notices then.
        if scope.aborted() {
            log::info!(
                "[TTS][stream] aborted by user after {} bytes",
                accumulated.len()
            );
            drop(stream);
            return Ok((Vec::new().into(), ttfa_ms.unwrap_or(0)));
        }
        match stream.recv() {
            Some(ChunkItem::Pcm(bytes)) => {
                if ttfa_ms.is_none() {
                    ttfa_ms = Some(start.elapsed().as_millis() as u64);
                }
                accumulated.extend_from_slice(&bytes);
                on_chunk(chunk_index, &bytes, false, None);
                chunk_index += 1;
            }
            Some(ChunkItem::Captions(value)) if value.validate().is_ok() => {
                on_chunk(chunk_index, &[], false, Some(&value));
                captions = Some(value);
            }
            Some(ChunkItem::Captions(_)) | Some(ChunkItem::ClearCaptions) => {
                log::warn!("Clearing rejected live stream captions; continuing audio");
                if let Some(mut previous) = captions.take() {
                    // None means "no update" on the existing live wire. Send
                    // an empty valid snapshot to clear both live consumers,
                    // while retaining None for saved/cache metadata.
                    previous.words.clear();
                    on_chunk(chunk_index, &[], false, Some(&previous));
                }
            }
            Some(ChunkItem::Failed(reason)) => {
                log::error!(
                    "[TTS][stream] failed during receive: {} ({} bytes received)",
                    reason,
                    accumulated.len()
                );
                return Err(format!("Streaming synthesis failed mid-stream: {reason}"));
            }
            None => break,
        }
    }

    if accumulated.is_empty() {
        log::error!("[TTS][stream] failed during receive: stream ended with no audio");
        return Err("Streaming synthesis produced no audio".to_string());
    }

    // Terminal end-of-stream marker so the frontend knows no more chunks follow.
    on_chunk(chunk_index, &[], true, captions.as_ref());

    let total_ms = start.elapsed().as_millis() as u64;
    log::info!(
        "[TTS][stream] ended: {} PCM bytes in {} chunks, {}ms total, TTFA {}ms",
        accumulated.len(),
        chunk_index,
        total_ms,
        ttfa_ms.unwrap_or(0)
    );
    Ok((
        SpeechAudio {
            bytes: pcm_to_wav(&accumulated, &meta),
            captions,
        },
        ttfa_ms.unwrap_or(0),
    ))
}

/// Whether `effect` has a live implementation on the webview's PCM streaming
/// path, so streaming keeps it. Only walkie-talkie streams: it mixes its
/// clicks and static into arriving audio. The others are whole-buffer
/// transforms that need the decoded fragment, so they keep the batch path,
/// where `buildPlaybackUrl` runs `effect.process` before playback.
fn effect_streams_live(effect: &EffectId) -> bool {
    matches!(effect, EffectId::None | EffectId::WalkieTalkie)
}

/// Whether a fresh synthesis may play as live `audio-stream-chunk` events.
/// The frontend PCM scheduler wires straight into `ctx.destination`, so an
/// effect that it cannot apply live must force the fragment path. A cold
/// backend (daemon still loading) takes the batch path until it is ready.
/// A non-default speed also takes the batch path: the PCM scheduler renders
/// speed with the webview's SoundTouch stretcher, which sounds rougher than
/// the `<audio>` element's pitch-preserving `playbackRate` the batch path uses.
/// Pitch alone stays on the stream path, where it is rendered by the same
/// stretcher the batch path uses, so streaming keeps its first-audio win.
fn allows_streaming_playback(
    streaming_enabled: bool,
    backend_streams: bool,
    runtime_ready: bool,
    effects_enabled: bool,
    active_effect: &EffectId,
    playback_speed: f32,
) -> bool {
    streaming_enabled
        && backend_streams
        && runtime_ready
        && (!effects_enabled || effect_streams_live(active_effect))
        && (playback_speed - 1.0).abs() <= 1e-3
}

/// Profile effects count only while Experimental is on. The saved effect stays
/// in config, so re-enabling Experimental restores it without a rewrite here.
fn effects_active(experimental_enabled: bool, profile_effects_enabled: bool) -> bool {
    experimental_enabled && profile_effects_enabled
}

/// Whether this fragment is the last one in its reading. Only that fragment may
/// send a final marker.
fn is_last_fragment(index: usize, total: usize) -> bool {
    index + 1 == total
}

/// A chunk carries the final marker only when it is the stream's last chunk
/// and its fragment is the reading's last one. The player arms its completion
/// timer on the marker, so an early one cuts the reading short.
fn final_marker(chunk_is_final: bool, fragment_is_last: bool) -> bool {
    chunk_is_final && fragment_is_last
}

/// Synthesize via the backend's streaming path, forwarding each PCM chunk to
/// the frontend as an `audio-stream-chunk` event while accumulating the full
/// payload into a WAV container for history/cache/envelope handling.
/// Returns the audio and its time-to-first-audio: the JIT pagination gate uses
/// the first-chunk latency as the next fragment's lead.
/// `rearm` is the reading's once-only stream-gate re-arm: the first chunk
/// this reading emits re-arms the webview's `_streamStopped` gate under the
/// speaker-ownership lock, healing a scoped stop of the previous owner that
/// landed after this reading's start signal (see
/// [`crate::speak_requests::StreamRearm`]).
async fn synthesize_streaming_and_emit(
    app: &AppHandle,
    backend: Arc<Box<dyn TtsBackend>>,
    text: String,
    voice: String,
    fragment_index: usize,
    emit_final: bool,
    session_id: Option<String>,
    scope: AbortScope,
    rearm: &std::sync::Arc<crate::speak_requests::StreamRearm>,
) -> Result<(SpeechAudio, u64), String> {
    let worker_app = app.clone();
    let failed_session = session_id.clone();
    let rearm = std::sync::Arc::clone(rearm);
    let result = tokio::task::spawn_blocking(move || {
        let opened_at = Instant::now();
        log::info!(
            "[TTS][stream] opening streaming synthesis ({} chars, voice '{}', fragment {})",
            text.len(),
            voice,
            fragment_index
        );

        let stream = match backend.synthesize_streaming(&text, &voice) {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    "[TTS][stream] failed during opening: {} (0 bytes received)",
                    e
                );
                return Err(e.to_string());
            }
        };

        let meta = stream.meta.clone();
        let mut pcm_bytes = 0u64;
        drain_chunk_stream(stream, scope, |chunk_idx, pcm, is_final, captions| {
            if scope.aborted() {
                return;
            }
            pcm_bytes += pcm.len() as u64;
            if !is_final && chunk_idx == 0 {
                log::info!(
                    "[TTS][stream] first chunk playing after {}ms",
                    opened_at.elapsed().as_millis()
                );
            }
            // The player treats a final marker as "no more audio is coming" and
            // arms its completion timer, so only the last fragment sends one.
            let bytes_per_second = u64::from(meta.sample_rate)
                * u64::from(meta.channels)
                * u64::from(meta.bits_per_sample / 8);
            let duration_ms = if is_final && bytes_per_second > 0 {
                Some(pcm_bytes * 1000 / bytes_per_second)
            } else {
                None
            };
            // Same ownership rule as emit_audio_fragment. The rearm runs
            // inside the claim: under the speaker-ownership lock, and ordered
            // on the wire before this chunk, so a scoped stop for the
            // previous owner can never orphan this reading's stream gate.
            crate::speak_requests::claim_playback(
                scope.request(),
                || scope.aborted(),
                || {
                    rearm.rearm(|| {
                        if let Err(e) = worker_app.emit("synthesis-state-change", true) {
                            log::warn!("Failed to emit synthesis-state-change: {}", e);
                        }
                    });
                    emit_audio_stream_chunk(
                        &worker_app,
                        &meta,
                        pcm,
                        fragment_index,
                        final_marker(is_final, emit_final),
                        (chunk_idx == 0).then_some(text.as_str()),
                        duration_ms,
                        captions,
                        session_id.as_deref(),
                    );
                },
            );
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))
    .and_then(|worker_result| worker_result);
    // An abort already stopped this reading's playback. Its late failure must
    // not emit, even when a newer reading has started since.
    if result.is_err() && !scope.aborted() {
        // The webview scheduler finishes only on a final marker, which a failed
        // stream never sends. The store stops playback only when this session
        // owns it, and finishes when nothing plays, so playback-finished fires
        // either way and restores ducked audio.
        let event = PlaybackStreamFailedEvent {
            session_id: failed_session,
        };
        if let Err(e) = app.emit("playback-stream-failed", event) {
            log::warn!("Failed to emit playback-stream-failed: {}", e);
        }
    }
    result
}

/// Save audio to history storage and return path
fn save_to_history_storage(
    config: &State<'_, Mutex<AppConfig>>,
    speech: &SpeechAudio,
    engine_id: &str,
    voice_name: &str,
    audio_ext: &str,
) -> Option<String> {
    let wav_bytes = &speech.bytes;
    let history_config = config.lock().unwrap().history.clone();
    let audio_ext = if wav_bytes.starts_with(b"RIFF") {
        "wav"
    } else {
        audio_ext
    };
    let path = crate::history::save_audio_to_storage(
        &history_config,
        wav_bytes,
        engine_id,
        voice_name,
        audio_ext,
    );
    if let Some(path) = &path {
        if let Some(captions) = &speech.captions {
            if let Err(error) = write_sidecar(path, captions) {
                log::warn!("Cannot save caption sidecar: {error}");
            }
        } else {
            crate::tts::captions::remove_sidecar(path);
        }
    }
    path
}

/// Cache audio for replay
fn cache_audio(app: &AppHandle, speech: &SpeechAudio, text: &str) {
    let wav_bytes = &speech.bytes;
    let cache = app.state::<Mutex<CachedAudio>>();
    let mut cache = cache.lock().unwrap();
    cache.wav_bytes = Some(wav_bytes.to_vec());
    cache.text = Some(text.to_string());
    cache.captions = speech.captions.clone();
}

/// Content-addressed speech cache: audio saved under a hash of the sanitized
/// text, engine and voice, so a repeated message replays without a provider
/// request. It is independent of the history log: history auto-delete can evict
/// its entry, and this copy still answers the repeat click. Its own byte budget
/// bounds disk use; the oldest files go first.
const SPEECH_CACHE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

fn speech_cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("CopySpeak")
        .join("speech-cache")
}

/// ponytail: 64-bit DefaultHasher key, not stable across Rust releases. A
/// collision would replay the wrong audio; a release change only misses.
/// Upgrade to a stored-key check if either bites.
fn speech_cache_file(
    dir: &std::path::Path,
    engine: &str,
    voice: &str,
    text: &str,
) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (engine, voice, text).hash(&mut hasher);
    dir.join(format!("{:016x}.audio", hasher.finish()))
}

fn save_speech_cache(
    dir: &std::path::Path,
    speech: &SpeechAudio,
    engine: &str,
    voice: &str,
    text: &str,
) {
    let path = speech_cache_file(dir, engine, voice, text);
    let path_str = path.to_string_lossy();
    let written = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, &speech.bytes));
    if let Err(e) = written {
        log::warn!("[TTS] Cannot cache audio: {e}");
        return;
    }
    match &speech.captions {
        Some(captions) => {
            if let Err(error) = write_sidecar(&path_str, captions) {
                log::warn!("Cannot save caption sidecar: {error}");
            }
        }
        None => crate::tts::captions::remove_sidecar(&path_str),
    }
    evict_speech_cache(dir, SPEECH_CACHE_BUDGET_BYTES);
}

/// Bytes one cached entry occupies: its audio plus the caption sidecar.
fn cached_entry_bytes(path: &std::path::Path) -> u64 {
    let audio = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let sidecar = std::fs::metadata(format!("{}.captions.json", path.to_string_lossy()))
        .map(|meta| meta.len())
        .unwrap_or(0);
    audio + sidecar
}

/// Drop the oldest cached audio, with its caption sidecars, until the cache
/// fits `budget` bytes. The newest entry always stays: it belongs to the reading
/// that was just saved, and evicting it would free nothing useful.
fn evict_speech_cache(dir: &std::path::Path, budget: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "audio"))
        .filter_map(|path| Some((std::fs::metadata(&path).ok()?.modified().ok()?, path)))
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|(_, path)| cached_entry_bytes(path)).sum();
    // Never evict the newest entry, even when it alone exceeds the budget.
    files.pop();
    for (_, path) in files {
        if total <= budget {
            break;
        }
        let freed = cached_entry_bytes(&path);
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("[TTS] Cannot evict cached audio: {e}");
            continue;
        }
        crate::tts::captions::remove_sidecar(&path.to_string_lossy());
        total = total.saturating_sub(freed);
    }
}

fn read_speech_cache(
    dir: &std::path::Path,
    engine: &str,
    voice: &str,
    text: &str,
) -> Option<SpeechAudio> {
    let path = speech_cache_file(dir, engine, voice, text);
    let bytes = std::fs::read(&path).ok()?;
    Some(SpeechAudio {
        bytes,
        captions: read_sidecar(&path.to_string_lossy()),
    })
}

/// Where a replayed reading came from, for the replay log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedSource {
    History,
    SpeechCache,
}

/// Newest successful history entry with this text, engine and voice whose audio
/// file still exists.
fn find_in_history(
    history: &Mutex<HistoryLog>,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<SpeechAudio> {
    let path = history
        .lock()
        .unwrap()
        .entries()
        .iter()
        .rev()
        .filter(|e| e.success && e.text == text && e.voice == voice && e.tts_engine == engine)
        .find_map(|e| {
            e.output_path
                .clone()
                .filter(|path| std::path::Path::new(path).exists())
        })?;
    match std::fs::read(&path) {
        Ok(bytes) => Some(SpeechAudio {
            bytes,
            captions: read_sidecar(&path),
        }),
        Err(e) => {
            log::warn!(
                "[TTS] Saved audio unreadable ({}): {}. Re-synthesizing.",
                path,
                e
            );
            None
        }
    }
}

/// Saved audio for this exact text, engine and voice. With history on the
/// newest history entry wins; the content-addressed cache is the fallback, so a
/// repeat still replays after history auto-delete evicted the entry. With
/// history off the cache is the only source. `None` when nothing matches or the
/// file is unreadable — the caller then synthesizes as usual.
fn find_cached_speech(
    history: &Mutex<HistoryLog>,
    history_on: bool,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<(SpeechAudio, CachedSource)> {
    find_cached_speech_in(
        &speech_cache_dir(),
        history,
        history_on,
        text,
        engine,
        voice,
    )
}

fn find_cached_speech_in(
    cache_dir: &std::path::Path,
    history: &Mutex<HistoryLog>,
    history_on: bool,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<(SpeechAudio, CachedSource)> {
    if history_on {
        if let Some(speech) = find_in_history(history, text, engine, voice) {
            return Some((speech, CachedSource::History));
        }
    }
    read_speech_cache(cache_dir, engine, voice, text)
        .map(|speech| (speech, CachedSource::SpeechCache))
}

/// Saved audio for a whole reading: one entry with this text, or else every
/// fragment `speak_queued` would split it into, joined in order. Reports the
/// source of the first fragment that came from the content-addressed cache.
fn find_cached_reading(
    history: &Mutex<HistoryLog>,
    history_on: bool,
    text: &str,
    pagination_config: &crate::config::PaginationConfig,
    engine: &str,
    voice: &str,
) -> Option<(SpeechAudio, CachedSource)> {
    if let Some(found) = find_cached_speech(history, history_on, text, engine, voice) {
        return Some(found);
    }
    let fragments = pagination::paginate_text(text, pagination_config);
    if fragments.len() < 2 {
        return None;
    }
    let mut from_cache = false;
    let parts = fragments
        .into_iter()
        .map(|f| {
            let (speech, source) = find_cached_speech(history, history_on, &f.text, engine, voice)?;
            if source == CachedSource::SpeechCache {
                from_cache = true;
            }
            Some((speech, f.text))
        })
        .collect::<Option<Vec<_>>>()?;
    let source = if from_cache {
        CachedSource::SpeechCache
    } else {
        CachedSource::History
    };
    crate::tts::captions::concat_speech(parts)
        .map(|speech| (speech, source))
        .map_err(|e| log::warn!("[TTS] Cannot join saved fragments: {e}. Re-synthesizing."))
        .ok()
}

/// Saved audio for one paginated fragment, or `None` when the fragment must be
/// synthesized. A fresh read never replays, and a streamed fragment has no
/// complete WAV to replay. A repeat click during an unfinished reading reaches
/// this for the fragments its early stop already paid for.
fn find_cached_fragment(
    history: &Mutex<HistoryLog>,
    history_on: bool,
    reuse_saved_audio: bool,
    streaming: bool,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<(SpeechAudio, CachedSource)> {
    find_cached_fragment_in(
        &speech_cache_dir(),
        history,
        history_on,
        reuse_saved_audio,
        streaming,
        text,
        engine,
        voice,
    )
}

fn find_cached_fragment_in(
    cache_dir: &std::path::Path,
    history: &Mutex<HistoryLog>,
    history_on: bool,
    reuse_saved_audio: bool,
    streaming: bool,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<(SpeechAudio, CachedSource)> {
    if !reuse_saved_audio || streaming {
        return None;
    }
    find_cached_speech_in(cache_dir, history, history_on, text, engine, voice)
}

/// Play audio that history already holds: no new entry, file or telemetry
/// sample, since nothing was synthesized.
fn replay_saved_audio(
    app: &AppHandle,
    speech: &SpeechAudio,
    text: &str,
    session_id: Option<&str>,
    scope: AbortScope,
) {
    cache_audio(app, speech, text);
    hud::show_hud(
        app,
        extract_envelope_or_default(&speech.bytes),
        Some(text.to_string()),
    );
    emit_audio_ready(app, speech, text, session_id, scope);
}

// ── speak_now ───────────────────────────────────────────────────────────────

/// Manually trigger TTS for the given text (or current clipboard if empty).
/// `trigger` marks a hotkey speak the browser companion may follow; the Play
/// page, tray and control server pass none.
#[tauri::command]
pub async fn speak_now(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    _player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
    post_process: Option<bool>,
    trigger: Option<TriggerSource>,
) -> Result<(), String> {
    let trigger = trigger.map(|source| (source, browser_bridge::new_trigger_id()));
    let trigger_id = trigger.as_ref().map(|(_, id)| id.clone());
    let result = speak_now_internal(
        app.clone(),
        config,
        _player,
        history,
        telemetry_state,
        text,
        None,
        true,
        post_process.unwrap_or(false),
        trigger,
        crate::speak_requests::NO_REQUEST,
    )
    .await;
    if let (Err(_), Some(id)) = (&result, trigger_id) {
        browser_bridge::fail_trigger(&app, &id);
    }
    result
}

/// `speak_now` on behalf of a control-server request: `request` is the
/// sequence `speak_requests::register` handed out for its `request_id`. A
/// scoped stop for that id ends this reading — before it starts, during
/// post-processing or synthesis, or while its audio plays — and nothing else.
pub async fn speak_now_for_request(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: String,
    post_process: bool,
    request: u64,
) -> Result<(), String> {
    speak_now_internal(
        app,
        config,
        player,
        history,
        telemetry_state,
        Some(text),
        None,
        true,
        post_process,
        None,
        request,
    )
    .await
}

/// Like `speak_now`, but always synthesizes fresh audio, even when history
/// already holds this text for the active engine and voice.
#[tauri::command]
pub async fn regenerate_now(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: String,
) -> Result<(), String> {
    speak_now_internal(
        app,
        config,
        player,
        history,
        telemetry_state,
        Some(text),
        None,
        false,
        false,
        None,
        crate::speak_requests::NO_REQUEST,
    )
    .await
}

#[tauri::command]
pub async fn speak_now_with_profile(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
    profile_id: Option<String>,
) -> Result<(), String> {
    speak_now_internal(
        app,
        config,
        player,
        history,
        telemetry_state,
        text,
        profile_id,
        true,
        false,
        None,
        crate::speak_requests::NO_REQUEST,
    )
    .await
}

/// Wait for the global synthesis queue (one reading at a time), giving up when
/// the reading is stopped first. The lock future is kept across polls, so the
/// wait keeps its place in line.
async fn wait_for_queue(
    lock: &tokio::sync::Mutex<()>,
    scope: AbortScope,
) -> Option<tokio::sync::MutexGuard<'_, ()>> {
    crate::speak_requests::until_cancelled_or_aborted(
        scope.request(),
        move || scope.aborted(),
        lock.lock(),
    )
    .await
}

#[cfg(test)]
mod queue_wait_tests {
    use super::*;

    #[tokio::test]
    async fn a_queued_reading_gives_up_when_its_request_is_stopped() {
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        let holder = lock.clone();
        let (acquired_tx, acquired_rx) = tokio::sync::oneshot::channel();
        let held = tokio::spawn(async move {
            let _guard = holder.lock().await;
            let _ = acquired_tx.send(());
            tokio::time::sleep(Duration::from_millis(300)).await;
        });
        acquired_rx.await.unwrap();

        let id = "queue-wait-scoped-stop";
        let crate::speak_requests::Registration::Proceed(seq) = crate::speak_requests::register(id)
        else {
            panic!("id must register");
        };
        let ticket = crate::speak_requests::Ticket::new(seq);
        let scope = AbortScope::begin_for_request(seq);
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            crate::speak_requests::cancel(id, || {});
        });

        let started = Instant::now();
        assert!(
            wait_for_queue(&lock, scope).await.is_none(),
            "a stopped queued reading must not wait for the lock"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        canceller.await.unwrap();
        drop(ticket);
        held.await.unwrap();
    }

    #[tokio::test]
    async fn a_queued_reading_takes_the_lock_after_the_holder_releases() {
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        let holder = lock.clone();
        let (acquired_tx, acquired_rx) = tokio::sync::oneshot::channel();
        let held = tokio::spawn(async move {
            let _guard = holder.lock().await;
            let _ = acquired_tx.send(());
            tokio::time::sleep(Duration::from_millis(60)).await;
        });
        acquired_rx.await.unwrap();

        let scope = AbortScope::begin_for_request(crate::speak_requests::NO_REQUEST);
        let guard = wait_for_queue(&lock, scope).await;
        assert!(
            guard.is_some(),
            "an unscoped reading must not abort by itself"
        );
        drop(guard);
        held.await.unwrap();
    }
}

async fn speak_now_internal(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    _player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
    _profile_id: Option<String>,
    reuse_saved_audio: bool,
    post_process_flag: bool,
    trigger: Option<(TriggerSource, String)>,
    request: u64,
) -> Result<(), String> {
    let text = get_text_or_clipboard(text)?;
    // The page matches its live selection against the text as copied.
    let trigger = trigger.map(|(source, id)| {
        let raw_text = text.clone();
        (SpeakTrigger { source, raw_text }, id)
    });
    let session_id = trigger.as_ref().map(|(_, id)| id.clone());
    // Play page, hotkey, tray, selection and control server all land here;
    // unsanitized web text (tabs, NBSPs) breaks engine tokenizers like Kokoro's.
    let text = {
        let cfg = config.lock().unwrap();
        crate::sanitize::sanitize_text(&text, &cfg.sanitization)
    };
    if text.trim().is_empty() {
        return Err("Nothing to speak".into());
    }

    let queue_lock_started = Instant::now();
    let lock_state = app.state::<tokio::sync::Mutex<()>>();
    // The abort scope begins before the wait: a reading that queues behind
    // another must drop when it is stopped while queued, not start once the
    // other reading ends. Unscoped reads (hotkey, tray, Play) carry no request
    // id, so the abort generation is their only stop signal.
    let scope = AbortScope::begin_for_request(request);
    let Some(_queue_lock) = wait_for_queue(&lock_state, scope).await else {
        log::info!("[TTS] Reading stopped while queued; nothing to speak");
        return Ok(());
    };
    let queue_lock_ms = queue_lock_started.elapsed().as_millis();
    if queue_lock_ms >= 20 {
        log::info!(
            "[TTS] Reading waited {} ms for the queue lock (request {})",
            queue_lock_ms,
            request
        );
    }

    // A stop that lands after this reading arrived, but before it reaches the
    // queue, still ends it. Stops from before its arrival do not.
    if scope.aborted() {
        log::info!("[TTS] Request cancelled before synthesis; nothing to speak");
        return Ok(());
    }

    // Enter critical section for TTS synthesis
    let _synthesis_guard = SynthesisGuard::new(&app);

    let (
        tts_config,
        output_config,
        pagination_config,
        post_process_config,
        sanitization,
        streaming_enabled,
        experimental_enabled,
    ) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.clone(),
            cfg.output.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
            cfg.sanitization.clone(),
            cfg.playback.streaming_enabled,
            cfg.general.experimental_enabled,
        )
    };
    let eff = resolve_effective_for_profile(&tts_config, _profile_id.as_deref())
        .unwrap_or_else(|_| resolve_effective(&tts_config));
    let gate = post_process_gate_for_effective(&eff, experimental_enabled);

    let effective_post_process_config =
        post_process_for_profile(&post_process_config, &eff.post_process);

    // Optional LLM post-processing (e.g., Groq) — best-effort; falls back to
    // the input text on any failure so synthesis is never blocked. Output
    // re-sanitization branches on the template class (tag-safe for preserve,
    // standard for rewrite; see try_process_for_profile).
    // Post-processing inserts an LLM round trip before any audio: show the
    // waiting HUD first so the delay is visible.
    if crate::post_process::post_process_skip_reason_for_profile(
        &effective_post_process_config,
        eff.post_process.enabled,
        post_process_flag,
        text.chars().count(),
        &gate,
    )
    .is_none()
    {
        hud::show_hud_synthesizing(&app, Some(text.clone()));
    }
    let text = crate::post_process::try_process_for_profile(
        text,
        &effective_post_process_config,
        eff.post_process.enabled,
        &sanitization,
        post_process_flag,
        gate,
    )
    .await;
    // The post-process round trip can take seconds; a stop issued during it
    // must not let synthesis start.
    if scope.aborted() {
        log::info!("[TTS] Aborted during post-processing; nothing to speak");
        return Ok(());
    }
    let _ = app.emit("reading-started", &text);
    // File output plays nothing, so there is no speak for a page to follow.
    if let Some((trigger, id)) = &trigger {
        if !(output_config.enabled && !output_config.directory.is_empty()) {
            browser_bridge::begin_trigger(&app, id, trigger, &text);
        }
    }

    log_tts_debug("TTS", &format!("{:?}", eff.engine), &text);

    let voice = eff.voice.clone();

    let backend: Box<dyn TtsBackend> = create_backend_from_effective(&eff, &tts_config);
    let engine_str_for_cache = engine_str(&eff.engine);

    let history_on = config.lock().unwrap().is_history_effective();
    let cache_lookup_started = Instant::now();
    let cached = if reuse_saved_audio {
        find_cached_reading(
            &history,
            history_on,
            &text,
            &pagination_config,
            &engine_str_for_cache,
            &voice,
        )
    } else {
        None
    };
    let cache_lookup_ms = cache_lookup_started.elapsed().as_millis();
    let cache_hit = cached.is_some();
    if reuse_saved_audio {
        log::info!(
            "[TTS] Saved-audio lookup {} after {} ms",
            if cache_hit { "hit" } else { "miss" },
            cache_lookup_ms
        );
    }

    let synthesis_start = Instant::now();

    // Get telemetry estimate for progress display
    let (estimated_ms, confidence) =
        telemetry::get_estimate(&telemetry_state, &engine_str_for_cache, &voice, text.len());

    // Emit initial synthesis progress
    hud::emit_synthesis_progress(
        &app,
        estimated_ms,
        0,
        0,
        1,
        false,
        confidence,
        truncate_preview(&text, 50),
        text.len(),
        0,
    );

    // Show HUD with synthesizing indicator
    hud::show_hud_synthesizing(&app, Some(text.clone()));

    // Wrap backend in Arc for sharing across pagination calls
    let backend_arc = Arc::new(backend);

    // The reading's once-only stream-gate re-arm, shared by every streamed
    // fragment so exactly the reading's first audible chunk re-arms.
    let stream_rearm = std::sync::Arc::new(crate::speak_requests::StreamRearm::new());

    // Streaming backends forward PCM chunks during synthesis in playback mode;
    // batch backends (and cache hits / file output / pagination) take the
    // untouched existing branches. An active effect streams only when the
    // webview implements it live; the others keep the batch path.
    let effects_on = effects_active(
        experimental_enabled && config.lock().unwrap().general.effects_enabled,
        eff.effects.enabled,
    );
    let playback_speed = active_playback_speed(&tts_config);
    let streaming_playback = allows_streaming_playback(
        streaming_enabled,
        backend_arc.supports_streaming(),
        backend_arc.runtime_ready(),
        effects_on,
        &eff.effects.active_effect,
        playback_speed,
    ) && !cache_hit
        && !output_config.enabled;
    let (wav_bytes, already_streamed) = if let Some((speech, source)) = cached {
        match source {
            CachedSource::History => {
                log::info!("[TTS] Replaying saved audio from history instead of synthesizing")
            }
            CachedSource::SpeechCache => log::info!(
                "[TTS] Replaying saved audio from the speech cache instead of synthesizing"
            ),
        }
        (speech, false)
    } else if pagination::should_paginate(&text, &pagination_config) && !output_config.enabled {
        // Pagination comes first even for streaming engines: bounded fragments
        // keep each request under the provider's input limit (OpenAI rejects
        // anything past 4096 characters). Each fragment then streams on its own,
        // so long text is both accepted and low-latency.
        let (wav, emitted_fragments) = synthesize_paginated(
            &app,
            backend_arc.clone(),
            &text,
            &voice,
            &eff.engine,
            &telemetry_state,
            &history,
            history_on,
            reuse_saved_audio,
            &synthesis_start,
            estimated_ms,
            confidence,
            streaming_playback,
            playback_speed,
            session_id.clone(),
            scope,
            &pagination_config,
            &stream_rearm,
        )
        .await?;
        (wav, streaming_playback || emitted_fragments)
    } else if streaming_playback {
        let (wav, _ttfa_ms) = synthesize_streaming_and_emit(
            &app,
            backend_arc.clone(),
            text.clone(),
            voice.clone(),
            0,
            true,
            session_id.clone(),
            scope,
            &stream_rearm,
        )
        .await?;
        (wav, true)
    } else {
        // Simple synthesis
        let wav = synthesize_async(backend_arc.clone(), text.clone(), voice.clone(), scope)
            .await
            .map_err(|e| report_section_failure(&app, 0, 1, scope, e))?;
        (wav, false)
    };

    // Both the paginated and streaming paths return empty bytes when the user
    // aborts mid-synthesis. do_abort_synthesis has already stopped playback and
    // emitted synthesis-aborted, so there is nothing to play or save here.
    if scope.aborted() {
        log::info!("[TTS] Synthesis aborted; nothing to play or save");
        return Ok(());
    }

    let synthesis_duration = synthesis_start.elapsed();
    let synthesis_ms = synthesis_duration.as_millis() as u64;

    // A replay took no synthesis time; recording it would skew estimates.
    if !cache_hit {
        record_telemetry(
            &telemetry_state,
            &engine_str(&eff.engine),
            &voice,
            text.len(),
            synthesis_ms,
        );
    }

    if crate::logging::is_debug_mode() {
        log::debug!("[TTS] Synthesis completed in {:?}", synthesis_duration);
        log::debug!("[TTS] Audio size: {} bytes", wav_bytes.bytes.len());
    }

    // Handle file output mode (a replay still writes the requested file)
    if output_config.enabled && !output_config.directory.is_empty() {
        return handle_file_output(
            &app,
            &config,
            &history,
            &wav_bytes,
            &text,
            &voice,
            &eff.engine,
            &output_config,
            synthesis_ms,
        );
    }

    if cache_hit {
        let replay_started = Instant::now();
        replay_saved_audio(&app, &wav_bytes, &text, session_id.as_deref(), scope);
        log::info!(
            "[TTS] Cached replay emitted after {} ms (lookup {} ms)",
            replay_started.elapsed().as_millis(),
            cache_lookup_ms
        );
        return Ok(());
    }

    // Normal playback mode
    handle_playback_output(
        &app,
        &config,
        &history,
        &wav_bytes,
        &text,
        &voice,
        &eff.engine,
        &tts_config,
        backend_arc,
        synthesis_ms,
        already_streamed,
        eff.voice_label.as_deref(),
        session_id.as_deref(),
        scope,
    )
}

/// How often the JIT wait re-reads the webview's playback progress. The
/// webview reports its caption every ~100 ms, so a faster poll only re-reads
/// the same snapshot.
const JIT_POLL_INTERVAL: Duration = Duration::from_millis(120);

/// Duration of a WAV payload from its header, without decoding its samples.
/// The JIT gate needs the Rust-side fragment duration as the fallback for a
/// caption that carries none.
fn wav_duration_ms(bytes: &[u8]) -> Option<u64> {
    let info = crate::audio::wav::parse_wav_header(bytes).ok()?;
    let bytes_per_second = u64::from(info.sample_rate)
        * u64::from(info.channels)
        * u64::from(info.bits_per_sample / 8);
    if bytes_per_second == 0 {
        return None;
    }
    Some(info.data_size as u64 * 1000 / bytes_per_second)
}

/// Hold the next fragment's submission until the live playback of `target` is
/// within `lead` of its end, polling the webview's caption. Returns `false`
/// when the request was aborted while waiting: then nothing more is submitted.
async fn wait_for_jit_window(
    scope: AbortScope,
    target: super::jit::JitTarget,
    lead: Duration,
    playback_speed: f32,
) -> bool {
    let started = Instant::now();
    let mut waiter = super::jit::JitWaiter::new(target, lead, playback_speed, started);
    log::info!(
        "[TTS] JIT: holding fragment {} until fragment {} playback enters its last {} ms",
        target.fragment_index + 2,
        target.fragment_index + 1,
        lead.as_millis()
    );
    loop {
        if scope.aborted() {
            log::info!(
                "[TTS] JIT: stopped while fragment {} played; fragment {} is never submitted",
                target.fragment_index + 1,
                target.fragment_index + 2
            );
            return false;
        }
        let progress = crate::playback_signal::current_progress();
        if waiter.observe(progress, Instant::now()).synthesize_now() {
            log::info!(
                "[TTS] JIT: fragment {} reached its lead window after {} ms; synthesizing fragment {}",
                target.fragment_index + 1,
                started.elapsed().as_millis(),
                target.fragment_index + 2
            );
            return true;
        }
        // Sleep one poll interval, but wake at once on a scoped stop: the
        // previous reading's `/speak` handler releases its wait lock only after
        // this future returns, so a slow poll would delay the next click.
        let poll = tokio::time::sleep(JIT_POLL_INTERVAL);
        if crate::speak_requests::until_cancelled(scope.request(), poll)
            .await
            .is_none()
        {
            log::info!(
                "[TTS] JIT: stopped while fragment {} played; fragment {} is never submitted",
                target.fragment_index + 1,
                target.fragment_index + 2
            );
            return false;
        }
    }
}

/// Synthesize paginated text with progress updates.
///
/// Fragments are not generated up front. After fragment i is submitted and
/// playing, the loop waits until its playback is within a small lead of its end
/// before submitting fragment i+1 ([`wait_for_jit_window`]). An early stop
/// therefore pays only for the fragment already in flight, and a pause freezes
/// the playback position, which naturally stops further generation until the
/// reading resumes.
async fn synthesize_paginated(
    app: &AppHandle,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    text: &str,
    voice: &str,
    engine: &crate::config::TtsEngine,
    telemetry_state: &State<'_, Mutex<telemetry::TelemetryLog>>,
    history: &State<'_, Mutex<HistoryLog>>,
    history_on: bool,
    reuse_saved_audio: bool,
    synthesis_start: &Instant,
    _total_estimate: Option<u64>,
    _avg_confidence: f32,
    streaming: bool,
    playback_speed: f32,
    session_id: Option<String>,
    scope: AbortScope,
    pagination_config: &crate::config::PaginationConfig,
    rearm: &std::sync::Arc<crate::speak_requests::StreamRearm>,
) -> Result<(SpeechAudio, bool), String> {
    let fragments = pagination::paginate_text(text, pagination_config);

    if fragments.len() <= 1 {
        // Only one fragment — fall back to normal synthesis
        let speech = if streaming {
            synthesize_streaming_and_emit(
                app,
                backend_arc.clone(),
                text.to_string(),
                voice.to_string(),
                0,
                true,
                session_id,
                scope,
                rearm,
            )
            .await
            .map(|(speech, _ttfa)| speech)
            .map_err(|e| report_section_failure(app, 0, 1, scope, e))
        } else {
            synthesize_async(
                backend_arc.clone(),
                text.to_string(),
                voice.to_string(),
                scope,
            )
            .await
            .map_err(|e| report_section_failure(app, 0, 1, scope, e))
        }?;
        return Ok((speech, false));
    }

    log::info!(
        "[TTS] Paginating text ({} chars) into {} fragments",
        text.len(),
        fragments.len()
    );

    let char_counts: Vec<usize> = fragments.iter().map(|f| f.text.len()).collect();
    let engine_str_val = engine_str(engine);
    let (total_estimate, avg_confidence, _) =
        telemetry::get_estimate_paginated(telemetry_state, &engine_str_val, voice, &char_counts);

    log::info!(
        "[TTS] JIT pagination: {} fragments, first submitted now, later ones on demand",
        fragments.len()
    );

    let mut fragment_audio = Vec::new();
    // The Rust-side duration of each fragment submitted so far: the JIT gate
    // matches the webview's caption against it when the caption carries no
    // fragment index.
    let mut fragment_durations_ms: Vec<u64> = Vec::new();
    // How long the last fragment took to start producing audio. It sets the
    // next fragment's lead, so a slow provider never opens an audible gap.
    let mut previous_cost = Duration::ZERO;
    for (i, fragment) in fragments.iter().enumerate() {
        // Check for abort
        if scope.aborted() {
            log::info!("[TTS] Pagination aborted by user");
            return Ok((Vec::new().into(), false));
        }

        // Just-in-time: submit fragment i only once fragment i-1 is about to
        // finish playing. Fragment 0 is always submitted immediately.
        if i > 0 {
            let target = super::jit::JitTarget {
                fragment_index: i - 1,
                duration_ms: fragment_durations_ms[i - 1],
            };
            let lead = super::jit::lead_after(previous_cost);
            if !wait_for_jit_window(scope, target, lead, playback_speed).await {
                return Ok((Vec::new().into(), false));
            }
        }

        // Emit progress update
        let elapsed = synthesis_start.elapsed().as_millis() as u64;
        let processed_chars: usize = fragments.iter().take(i).map(|f| f.text.len()).sum();
        hud::emit_synthesis_progress(
            app,
            total_estimate,
            elapsed,
            i,
            fragments.len(),
            true,
            avg_confidence,
            truncate_preview(&fragment.text, 50),
            text.len(),
            processed_chars,
        );

        // Reuse a fragment this reading, or an earlier stopped one, already paid
        // for. The whole-reading lookup in `speak_now_internal` needs every
        // fragment, so a repeat click after an early stop lands here instead.
        let cached_fragment = find_cached_fragment(
            history,
            history_on,
            reuse_saved_audio,
            streaming,
            &fragment.text,
            &engine_str_val,
            voice,
        );
        let fragment_from_cache = cached_fragment.is_some();
        if !fragment_from_cache {
            log::info!(
                "[TTS] Synthesizing fragment {}/{} ({} chars)",
                i + 1,
                fragments.len(),
                fragment.text.len()
            );
        }

        let generation_started = Instant::now();
        let frag_wav = if let Some((speech, source)) = cached_fragment {
            match source {
                CachedSource::History => log::info!(
                    "[TTS] Fragment {}/{} replayed from history; no provider request",
                    i + 1,
                    fragments.len()
                ),
                CachedSource::SpeechCache => log::info!(
                    "[TTS] Fragment {}/{} replayed from the speech cache; no provider request",
                    i + 1,
                    fragments.len()
                ),
            }
            // A replay has no generation cost, so the next lead uses the minimum.
            previous_cost = Duration::ZERO;
            speech
        } else if streaming {
            let (speech, ttfa_ms) = synthesize_streaming_and_emit(
                app,
                backend_arc.clone(),
                fragment.text.clone(),
                voice.to_string(),
                i,
                i + 1 == fragments.len(),
                session_id.clone(),
                scope,
                rearm,
            )
            .await
            .map_err(|e| report_section_failure(app, i, fragments.len(), scope, e))?;
            // The streaming path only needs its first chunk to cover the gap.
            previous_cost = Duration::from_millis(ttfa_ms);
            speech
        } else {
            let speech = synthesize_async(
                backend_arc.clone(),
                fragment.text.clone(),
                voice.to_string(),
                scope,
            )
            .await
            .map_err(|e| report_section_failure(app, i, fragments.len(), scope, e))?;
            // The batch path has nothing to play until the whole fragment is
            // synthesized, so the next lead covers that full cost.
            previous_cost = generation_started.elapsed();
            speech
        };

        // Cache a fresh fragment as soon as it exists: an early stop must keep
        // the audio it already paid for, so the next repeat replays it.
        if !fragment_from_cache {
            save_speech_cache(
                &speech_cache_dir(),
                &frag_wav,
                &engine_str_val,
                voice,
                &fragment.text,
            );
        }

        if scope.aborted() {
            return Ok((Vec::new().into(), false));
        }
        if !streaming {
            if i == 0 {
                hud::show_hud(
                    app,
                    extract_envelope_or_default(&frag_wav.bytes),
                    Some(text.to_string()),
                );
            }
            emit_audio_fragment(
                app,
                &frag_wav,
                i,
                fragments.len(),
                fragment.text.clone(),
                session_id.as_deref(),
                scope,
            );
        }
        fragment_durations_ms.push(wav_duration_ms(&frag_wav.bytes).unwrap_or(0));
        fragment_audio.push((frag_wav, fragment.text.clone()));
    }

    crate::tts::captions::concat_speech(fragment_audio)
        .map(|speech| (speech, !streaming))
        // A concat failure lands after fragments were already emitted and may
        // be playing: surface it as a reading failure so the webview queue
        // stops waiting for the final fragment that will never arrive.
        .map_err(|e| {
            report_section_failure(
                app,
                0,
                1,
                scope,
                format!("Failed to concatenate audio fragments: {e}"),
            )
        })
}

/// Handle file output mode
fn handle_file_output(
    app: &AppHandle,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    speech: &SpeechAudio,
    text: &str,
    voice: &str,
    engine: &crate::config::TtsEngine,
    output_config: &crate::config::OutputConfig,
    synthesis_ms: u64,
) -> Result<(), String> {
    let wav_bytes = &speech.bytes;
    let output_dir = std::path::Path::new(&output_config.directory);
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {e}"))?;

    let mut filename =
        config::expand_filename_pattern(&output_config.filename_pattern, voice, text);

    let audio_bytes = if output_config.format_config.format != crate::config::AudioFormat::Wav {
        if filename.to_lowercase().ends_with(".wav") {
            let new_ext = output_config.format_config.format.default_extension();
            filename = format!(
                "{}.{}",
                filename.trim_end_matches(".wav").trim_end_matches(".WAV"),
                new_ext
            );
        }
        audio::convert_audio_format(wav_bytes, &output_config.format_config)
            .map_err(|e| format!("Failed to convert audio format: {e}"))?
    } else {
        wav_bytes.to_vec()
    };

    let output_path = output_dir.join(&filename);
    std::fs::write(&output_path, &audio_bytes)
        .map_err(|e| format!("Failed to write audio file: {e}"))?;

    crate::tts::captions::remove_sidecar(&output_path.to_string_lossy());
    if let Some(captions) = &speech.captions {
        if let Err(error) = write_sidecar(&output_path.to_string_lossy(), captions) {
            log::warn!("Cannot save caption sidecar: {error}");
        }
    }
    log::info!("Saved TTS audio to: {}", output_path.display());

    if crate::logging::is_debug_mode() {
        log::debug!("[TTS] Output file: {} bytes", audio_bytes.len());
    }

    let audio_duration_ms = audio::extract_envelope(wav_bytes, 1)
        .map(|e| e.duration_ms)
        .unwrap_or(0);

    let history_on = config.lock().unwrap().is_history_effective();
    if history_on {
        add_history_with_metadata(
            history,
            text,
            &engine_str(engine),
            voice,
            audio_duration_ms,
            Some(output_path.to_string_lossy().into_owned()),
            None,
            synthesis_ms,
            None,
        );
        let _ = app.emit("history-updated", ());
    }
    // A submitted reading is also a repeat candidate: keep the content-addressed
    // copy independent of what history does with its entry.
    save_speech_cache(
        &speech_cache_dir(),
        speech,
        &engine_str(engine),
        voice,
        text,
    );

    Ok(())
}

/// Handle normal playback output
fn handle_playback_output(
    app: &AppHandle,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    speech: &SpeechAudio,
    text: &str,
    voice: &str,
    engine: &crate::config::TtsEngine,
    tts_config: &crate::config::TtsConfig,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    synthesis_ms: u64,
    already_streamed: bool,
    voice_label: Option<&str>,
    session_id: Option<&str>,
    scope: AbortScope,
) -> Result<(), String> {
    let wav_bytes = &speech.bytes;
    let envelope = extract_envelope_or_default(wav_bytes);

    let engine_id = engine_identifier(engine);
    let voice_name = voice_display_name(engine, tts_config, voice, voice_label);
    let audio_ext = backend_arc.file_extension().to_string();

    let history_on = config.lock().unwrap().is_history_effective();
    if history_on {
        let history_path =
            save_to_history_storage(config, speech, &engine_id, &voice_name, &audio_ext);
        add_history_with_metadata(
            history,
            text,
            &engine_str(engine),
            voice,
            envelope.duration_ms,
            history_path,
            None,
            synthesis_ms,
            None,
        );
        let _ = app.emit("history-updated", ());
    }

    // Cache audio for replay
    cache_audio(app, speech, text);

    // Show HUD with waveform visualization
    hud::show_hud(app, envelope, Some(text.to_string()));

    // Emit audio to frontend for browser-native playback. Skipped when PCM
    // chunks were already forwarded by the streaming synthesis path.
    if !already_streamed {
        emit_audio_ready(app, speech, text, session_id, scope);
    }

    // Keep the content-addressed copy independent of history: history
    // auto-delete can evict its entry, and this copy still answers a repeat
    // click for free. Written after the emit so it cannot delay first audio.
    save_speech_cache(
        &speech_cache_dir(),
        speech,
        &engine_str(engine),
        voice,
        text,
    );

    Ok(())
}

// ── speak_queued ────────────────────────────────────────────────────────────

/// Speak text with fragment queue support for long texts.
/// Automatically paginates long texts and plays fragments sequentially.
/// `trigger` marks a double-copy speak the browser companion may follow; it
/// carries the text as copied because `text` arrives already sanitized.
#[tauri::command]
pub async fn speak_queued(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    _player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    queue: State<'_, Mutex<FragmentQueue>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
    trigger: Option<SpeakTrigger>,
    reading_id: Option<String>,
) -> Result<(), String> {
    let trigger = trigger.map(|trigger| (trigger, browser_bridge::new_trigger_id()));
    let trigger_id = trigger.as_ref().map(|(_, id)| id.clone());
    let session_id = trigger_id.clone().or(reading_id);
    let result = speak_queued_internal(
        app.clone(),
        config,
        history,
        queue,
        telemetry_state,
        text,
        trigger,
        session_id,
    )
    .await;
    if let (Err(_), Some(id)) = (&result, trigger_id) {
        browser_bridge::fail_trigger(&app, &id);
    }
    result
}

async fn speak_queued_internal(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    history: State<'_, Mutex<HistoryLog>>,
    queue: State<'_, Mutex<FragmentQueue>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
    trigger: Option<(SpeakTrigger, String)>,
    session_id: Option<String>,
) -> Result<(), String> {
    let text = get_text_or_clipboard(text)?;
    if text.trim().is_empty() {
        return Err("Nothing to speak".into());
    }

    let (
        tts_config,
        pagination_config,
        post_process_config,
        sanitization,
        streaming_enabled,
        experimental_enabled,
    ) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
            cfg.sanitization.clone(),
            cfg.playback.streaming_enabled,
            cfg.general.experimental_enabled,
        )
    };
    let eff = resolve_effective(&tts_config);
    let gate = post_process_gate_for_effective(&eff, experimental_enabled);

    let effective_post_process_config =
        post_process_for_profile(&post_process_config, &eff.post_process);

    // Aborts issued before this run starts do not reach it. speak_queued takes
    // no queue lock, so a shared flag cleared here could also un-abort a run
    // still draining from before.
    let scope = AbortScope::begin();

    // Optional LLM post-processing — best-effort; falls back on failure.
    // Double-copy processes in automatic mode (request flag false) when the
    // profile allows it; output re-sanitization matches speak_now's class
    // branch. Post-processing inserts an LLM round trip before any audio:
    // show the waiting HUD first so the delay is visible.
    let post_processing = crate::post_process::post_process_skip_reason_for_profile(
        &effective_post_process_config,
        eff.post_process.enabled,
        false,
        text.chars().count(),
        &gate,
    )
    .is_none();
    if post_processing {
        hud::show_hud_synthesizing(&app, Some(text.clone()));
    }
    let text = crate::post_process::try_process_for_profile(
        text,
        &effective_post_process_config,
        eff.post_process.enabled,
        &sanitization,
        false,
        gate,
    )
    .await;
    let _ = app.emit("reading-started", &text);
    if let Some((trigger, id)) = &trigger {
        browser_bridge::begin_trigger(&app, id, trigger, &text);
    }

    log_tts_debug("Queue", &format!("{:?}", eff.engine), &text);

    if !post_processing {
        // Keep the previous HUD timing when no LLM delay precedes synthesis.
        hud::show_hud_synthesizing(&app, Some(text.clone()));
    }

    // Paginate text
    let fragments = pagination::paginate_text(&text, &pagination_config);

    if crate::logging::is_debug_mode() {
        log::debug!("[Queue] Created {} fragments", fragments.len());
    }

    let voice = eff.voice.clone();

    // Get telemetry estimates
    let char_counts: Vec<usize> = fragments.iter().map(|f| f.text.len()).collect();
    let (total_estimate, avg_confidence, _) = telemetry::get_estimate_paginated(
        &telemetry_state,
        &engine_str(&eff.engine),
        &voice,
        &char_counts,
    );

    let synthesis_start = Instant::now();
    let total_chars = text.len();

    // Emit initial synthesis progress
    hud::emit_synthesis_progress(
        &app,
        total_estimate,
        0,
        0,
        fragments.len(),
        fragments.len() > 1,
        avg_confidence,
        truncate_preview(&text, 50),
        total_chars,
        0,
    );

    // Clear previous queue and add new fragments
    {
        let q = queue.lock().unwrap();
        q.clear();
        q.add_fragments(fragments.clone());
    }

    // Emit pagination started event
    let _ = app.emit(
        "pagination:started",
        PaginationEvent {
            total: fragments.len(),
            current_index: 0,
            is_paginated: fragments.len() > 1,
        },
    );

    // Enter synthesis state
    let _synthesis_guard = SynthesisGuard::new(&app);

    // Generate batch ID for grouping history entries
    let total = fragments.len();
    let batch_id = if total > 1 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Some(format!("batch_{}", ts))
    } else {
        None
    };

    let engine_str_val = engine_str(&eff.engine);
    let engine_id_val = engine_identifier(&eff.engine);

    // Repeated text replays saved audio — only when the saved-audio store holds
    // every fragment, so a reading never mixes saved and fresh parts nor leaves
    // a partial batch in history.
    let history_on = config.lock().unwrap().is_history_effective();
    let mut saved: Option<std::vec::IntoIter<SpeechAudio>> = fragments
        .iter()
        .map(|f| {
            find_cached_speech(&history, history_on, &f.text, &engine_str_val, &voice)
                .map(|(speech, _source)| speech)
        })
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_iter);
    let replaying = saved.is_some();
    if replaying {
        log::info!("[Queue] Replaying saved audio for all {} fragments", total);
    }

    // Synthesize and play each fragment
    // The reading's once-only stream-gate re-arm, shared by every streamed
    // fragment so exactly the reading's first audible chunk re-arms.
    let stream_rearm = std::sync::Arc::new(crate::speak_requests::StreamRearm::new());
    for (index, fragment) in fragments.iter().enumerate() {
        // Check if we should stop
        {
            let q = queue.lock().unwrap();
            if q.should_stop() {
                log::info!("[Queue] Playback stopped by user");
                q.clear();
                let _ = app.emit(
                    "pagination:stopped",
                    PaginationEvent {
                        total,
                        current_index: index,
                        is_paginated: total > 1,
                    },
                );
                return Ok(());
            }
        }

        log::debug!("[Queue] Synthesizing fragment {} of {}", index + 1, total);

        // Emit fragment started event
        let _ = app.emit(
            "pagination:fragment-started",
            PaginationEvent {
                total,
                current_index: index,
                is_paginated: total > 1,
            },
        );

        // Emit progress update
        let elapsed = synthesis_start.elapsed().as_millis() as u64;
        let processed_chars: usize = fragments.iter().take(index).map(|f| f.text.len()).sum();
        hud::emit_synthesis_progress(
            &app,
            total_estimate,
            elapsed,
            index,
            total,
            true,
            avg_confidence,
            truncate_preview(&fragment.text, 50),
            total_chars,
            processed_chars,
        );

        // Set current index in queue
        {
            let q = queue.lock().unwrap();
            q.set_current_index(index);
        }

        // Create backend for this fragment from the resolved profile so engine
        // options (model, format, etc.) are honored, not just the global config.
        let backend: Box<dyn TtsBackend> = create_backend_from_effective(&eff, &tts_config);
        let backend_arc = Arc::new(backend);

        // Synthesize fragment — streaming-capable backends forward PCM chunks
        // as they arrive; batch backends take the untouched synthesize path.
        let fragment_start = Instant::now();
        let effects_on = effects_active(
            experimental_enabled && config.lock().unwrap().general.effects_enabled,
            eff.effects.enabled,
        );
        let streamed = !replaying
            && allows_streaming_playback(
                streaming_enabled,
                backend_arc.supports_streaming(),
                backend_arc.runtime_ready(),
                effects_on,
                &eff.effects.active_effect,
                active_playback_speed(&tts_config),
            );
        let wav_bytes = if let Some(speech) = saved.as_mut().and_then(Iterator::next) {
            speech
        } else if streamed {
            synthesize_streaming_and_emit(
                &app,
                backend_arc.clone(),
                fragment.text.clone(),
                voice.clone(),
                index,
                is_last_fragment(index, total),
                session_id.clone(),
                scope,
                &stream_rearm,
            )
            .await
            .map(|(speech, _ttfa_ms)| speech)
            .map_err(|e| report_section_failure(&app, index, total, scope, e))?
        } else {
            synthesize_async(
                backend_arc.clone(),
                fragment.text.clone(),
                voice.clone(),
                scope,
            )
            .await
            .map_err(|e| report_section_failure(&app, index, total, scope, e))?
        };
        let fragment_duration = fragment_start.elapsed();
        if scope.aborted() {
            return Ok(());
        }

        if !replaying {
            record_telemetry(
                &telemetry_state,
                &engine_str_val,
                &voice,
                fragment.text.len(),
                fragment_duration.as_millis() as u64,
            );
        }

        // Store audio in queue
        {
            let q = queue.lock().unwrap();
            q.set_audio(index, wav_bytes.bytes.clone());
        }

        log::debug!(
            "[Queue] Synthesized fragment {} ({} bytes)",
            index + 1,
            wav_bytes.bytes.len()
        );

        // Extract envelope
        let envelope = extract_envelope_or_default(&wav_bytes.bytes);

        // Transition HUD from synthesizing → playing on first fragment
        if index == 0 {
            hud::show_hud(&app, envelope.clone(), Some(text.clone()));
        }

        // Save to the content-addressed cache on every fresh fragment (a replay
        // is already there), independent of history: the cache answers repeat
        // readings after the history entry is evicted.
        if !replaying {
            save_speech_cache(
                &speech_cache_dir(),
                &wav_bytes,
                &engine_str_val,
                &voice,
                &fragment.text,
            );
        }
        if !replaying && history_on {
            let audio_ext = backend_arc.file_extension().to_string();
            let voice_name =
                voice_display_name(&eff.engine, &tts_config, &voice, eff.voice_label.as_deref());
            let history_path = save_to_history_storage(
                &config,
                &wav_bytes,
                &engine_id_val,
                &voice_name,
                &audio_ext,
            );

            // Build metadata
            let mut metadata = HashMap::new();
            if total > 1 {
                metadata.insert("fragment_index".to_string(), serde_json::json!(index));
                metadata.insert("fragment_total".to_string(), serde_json::json!(total));
            }

            add_history_with_metadata(
                &history,
                &fragment.text,
                &engine_str_val,
                &voice,
                envelope.duration_ms,
                history_path,
                batch_id.clone(),
                fragment_duration.as_millis() as u64,
                Some(metadata),
            );
            let _ = app.emit("history-updated", ());
        }

        // Emit audio fragment for streaming playback. Skipped when the PCM
        // chunks were already forwarded by the streaming path above.
        if !streamed {
            emit_audio_fragment(
                &app,
                &wav_bytes,
                index,
                total,
                fragment.text.clone(),
                session_id.as_deref(),
                scope,
            );
        }

        // Emit fragment events
        let _ = app.emit(
            "pagination:fragment-ready",
            PaginationEvent {
                total,
                current_index: index,
                is_paginated: total > 1,
            },
        );
        let _ = app.emit(
            "pagination:fragment-complete",
            PaginationEvent {
                total,
                current_index: index,
                is_paginated: total > 1,
            },
        );
    }

    // Clear queue (frontend handles streaming)
    {
        let q = queue.lock().unwrap();
        q.clear();
    }

    log::info!("[Queue] All {} fragments synthesized and streamed", total);

    // Emit pagination complete event
    let _ = app.emit(
        "pagination:complete",
        PaginationEvent {
            total,
            current_index: total - 1,
            is_paginated: total > 1,
        },
    );

    Ok(())
}

// ── speak_history_entry ─────────────────────────────────────────────────────

/// Re-synthesize and play a history entry.
/// This creates new TTS audio using the current config and plays it.
#[tauri::command]
pub async fn speak_history_entry(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    entry_id: String,
) -> Result<(), String> {
    let scope = AbortScope::begin();
    let _synthesis_guard = SynthesisGuard::new(&app);
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] speak_history_entry called (id: {})", entry_id);
    }

    // Get text and voice from history
    let (text, original_voice) = {
        let hist = history.lock().unwrap();
        let entry = hist
            .get_by_id(&entry_id)
            .ok_or_else(|| format!("History entry not found: {}", entry_id))?;
        (entry.text.clone(), entry.voice.clone())
    };

    if text.trim().is_empty() {
        return Err("Nothing to speak".to_string());
    }

    let (tts_config, history_on) = {
        let cfg = config.lock().unwrap();
        (cfg.tts.clone(), cfg.is_history_effective())
    };

    let active_backend = active_engine(&tts_config);

    let backend: Box<dyn TtsBackend> = create_backend(&active_backend, &tts_config);
    let voice = original_voice;
    let engine_str_val = engine_str(&active_backend);
    let engine_id = engine_identifier(&active_backend);
    let voice_name = voice_display_name(&active_backend, &tts_config, &voice, None);
    let audio_ext = backend.file_extension().to_string();

    // Get telemetry estimate
    let (estimated_ms, confidence) =
        telemetry::get_estimate(&telemetry_state, &engine_str_val, &voice, text.len());

    // Emit progress event
    hud::emit_synthesis_progress(
        &app,
        estimated_ms,
        0,
        0,
        1,
        false,
        confidence,
        truncate_preview(&text, 50),
        text.len(),
        0,
    );

    // Show HUD with synthesizing indicator
    hud::show_hud_synthesizing(&app, Some(text.clone()));

    let backend_arc = Arc::new(backend);
    let synthesis_start = Instant::now();

    // Synthesize
    let wav_bytes = synthesize_async(backend_arc.clone(), text.clone(), voice.clone(), scope)
        .await
        .map_err(|e| report_section_failure(&app, 0, 1, scope, e))?;
    if scope.aborted() {
        return Ok(());
    }
    let synthesis_ms = synthesis_start.elapsed().as_millis() as u64;

    // Record telemetry
    record_telemetry(
        &telemetry_state,
        &engine_str_val,
        &voice,
        text.len(),
        synthesis_ms,
    );

    // Extract envelope
    let envelope = extract_envelope_or_default(&wav_bytes.bytes);

    if history_on {
        // Save to history
        let history_path =
            save_to_history_storage(&config, &wav_bytes, &engine_id, &voice_name, &audio_ext);
        add_history_with_metadata(
            &history,
            &text,
            &engine_str_val,
            &voice,
            envelope.duration_ms,
            history_path,
            None,
            synthesis_ms,
            None,
        );
        let _ = app.emit("history-updated", ());
    }
    // Re-spoken audio is also a repeat candidate; keep the content-addressed
    // copy independent of what history does with its entry.
    save_speech_cache(
        &speech_cache_dir(),
        &wav_bytes,
        &engine_str_val,
        &voice,
        &text,
    );

    // Show HUD with waveform visualization
    hud::show_hud(&app, envelope, Some(text.clone()));

    // Emit audio to frontend
    emit_audio_ready(&app, &wav_bytes, &text, None, scope);

    log::info!("Re-spoke history entry: {}", entry_id);
    Ok(())
}

#[cfg(test)]
mod streaming_tests {
    use super::*;
    use std::sync::mpsc;

    fn meta(sample_rate: u32) -> AudioFormatMeta {
        AudioFormatMeta {
            sample_rate,
            channels: 1,
            bits_per_sample: 16,
        }
    }

    fn stream_from_items(items: Vec<ChunkItem>, sample_rate: u32) -> ChunkStream {
        let (tx, rx) = mpsc::channel();
        for item in items {
            tx.send(item).unwrap();
        }
        drop(tx);
        ChunkStream::new(meta(sample_rate), rx)
    }

    #[test]
    fn multi_chunk_order_preserved_and_pcm_wrapped_into_valid_wav() {
        let a = vec![1u8, 2, 3, 4];
        let b = vec![5u8, 6, 7, 8];
        let stream = stream_from_items(
            vec![ChunkItem::Pcm(a.clone()), ChunkItem::Pcm(b.clone())],
            44100,
        );

        let mut seen: Vec<(usize, Vec<u8>, bool)> = Vec::new();
        let (wav, _ttfa) =
            drain_chunk_stream(stream, AbortScope::begin(), |idx, pcm, is_final, _| {
                seen.push((idx, pcm.to_vec(), is_final));
            })
            .expect("healthy stream drains");

        assert_eq!(seen.len(), 3, "two data chunks plus the terminal marker");
        assert_eq!(seen[0], (0, a.clone(), false));
        assert_eq!(seen[1], (1, b.clone(), false));
        assert!(seen[2].2, "terminal event must be marked final");
        assert!(seen[2].1.is_empty());

        // Wrapped WAV parses back to the concatenated PCM with the stream meta.
        let info = crate::audio::wav::parse_wav_header(&wav.bytes).expect("wrapped WAV valid");
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.data_size, 8);
        let expected = [a, b].concat();
        assert_eq!(
            &wav.bytes[info.data_offset..info.data_offset + info.data_size],
            &expected
        );
    }

    #[test]
    fn streaming_gate_keeps_only_live_effects_on_the_pcm_path() {
        // Streaming on, warm streaming backend, default speed: no effect and
        // the live walkie-talkie stream play as chunks arrive.
        assert!(allows_streaming_playback(
            true,
            true,
            true,
            false,
            &EffectId::None,
            1.0
        ));
        assert!(allows_streaming_playback(
            true,
            true,
            true,
            true,
            &EffectId::WalkieTalkie,
            1.0
        ));
        // Whole-buffer effects cannot run in the PCM scheduler: keep the batch
        // path, where buildPlaybackUrl applies them to the decoded fragment.
        assert!(!allows_streaming_playback(
            true,
            true,
            true,
            true,
            &EffectId::GameBoy,
            1.0
        ));
        assert!(!allows_streaming_playback(
            true,
            true,
            true,
            true,
            &EffectId::Cyberpunk,
            1.0
        ));
    }

    #[test]
    fn non_default_speed_keeps_the_batch_path() {
        // The PCM scheduler renders speed with the webview's SoundTouch
        // stretcher; the batch path keeps the audio element's pitch-preserving
        // playbackRate, which sounds cleaner. Any non-default speed must take
        // the batch path, fast or slow, whatever the effect.
        assert!(!allows_streaming_playback(
            true,
            true,
            true,
            false,
            &EffectId::None,
            1.65
        ));
        assert!(!allows_streaming_playback(
            true,
            true,
            true,
            false,
            &EffectId::None,
            0.75
        ));
        assert!(!allows_streaming_playback(
            true,
            true,
            true,
            true,
            &EffectId::WalkieTalkie,
            1.4
        ));
        // Default speed keeps the stream path and its first-audio win.
        assert!(allows_streaming_playback(
            true,
            true,
            true,
            true,
            &EffectId::WalkieTalkie,
            1.0
        ));
    }

    #[test]
    fn effect_streams_live_only_for_the_pcm_implemented_effect() {
        assert!(effect_streams_live(&EffectId::None));
        assert!(effect_streams_live(&EffectId::WalkieTalkie));
        assert!(!effect_streams_live(&EffectId::GameBoy));
        assert!(!effect_streams_live(&EffectId::Cyberpunk));
    }

    #[test]
    fn kill_switches_force_the_batch_path() {
        // Any kill switch routes the reading through audio-fragment-ready.
        assert!(!allows_streaming_playback(
            false,
            true,
            true,
            false,
            &EffectId::None,
            1.0
        ));
        assert!(!allows_streaming_playback(
            true,
            false,
            true,
            false,
            &EffectId::None,
            1.0
        ));
    }

    #[test]
    fn saved_effects_are_inert_while_experimental_is_off() {
        // Experimental off must let a saved effect stream again, whatever it is.
        assert!(effects_active(true, true));
        assert!(!effects_active(false, true));
        assert!(!effects_active(true, false));
        assert!(allows_streaming_playback(
            true,
            true,
            true,
            effects_active(false, true),
            &EffectId::GameBoy,
            1.0
        ));
    }

    #[test]
    fn cold_backend_takes_batch_path_until_ready() {
        // A static streaming claim alone must not stream: a cold CLI daemon
        // would otherwise run a one-shot command through the stream branch.
        assert!(!allows_streaming_playback(
            true,
            true,
            false,
            false,
            &EffectId::None,
            1.0
        ));
        assert!(allows_streaming_playback(
            true,
            true,
            true,
            false,
            &EffectId::None,
            1.0
        ));
    }

    #[test]
    fn only_the_last_fragments_last_chunk_is_marked_final() {
        // Pins the rule the helpers encode: only the reading's last fragment's
        // final chunk carries the marker. The call site passing
        // is_last_fragment(...) is not exercised here.
        let total = 3;
        let marks: Vec<bool> = (0..total)
            .map(|index| final_marker(true, is_last_fragment(index, total)))
            .collect();
        assert_eq!(marks, [false, false, true]);
        assert!(!final_marker(false, is_last_fragment(total - 1, total)));
    }

    #[test]
    fn invalid_captions_do_not_interrupt_live_audio_or_survive_in_history() {
        let valid = CaptionAlignment {
            text: "Hi".into(),
            words: vec![crate::tts::captions::WordTiming {
                text_start: 0,
                text_end: 2,
                start_ms: 0.0,
                end_ms: 0.1,
            }],
        };
        let mut invalid = valid.clone();
        invalid.words[0].text_end = 9;
        let cleared = CaptionAlignment {
            text: valid.text.clone(),
            words: Vec::new(),
        };
        for rejection in [ChunkItem::Captions(invalid), ChunkItem::ClearCaptions] {
            let stream = stream_from_items(
                vec![
                    ChunkItem::Captions(valid.clone()),
                    ChunkItem::Pcm(vec![1, 2]),
                    rejection,
                    ChunkItem::Pcm(vec![3, 4]),
                ],
                8000,
            );
            let mut seen = Vec::new();
            let (speech, _) = drain_chunk_stream(
                stream,
                AbortScope::begin(),
                |idx, pcm, is_final, captions| {
                    seen.push((idx, pcm.to_vec(), is_final, captions.cloned()));
                },
            )
            .unwrap();
            assert_eq!(speech.bytes, pcm_to_wav(&[1, 2, 3, 4], &meta(8000)));
            assert_eq!(speech.captions, None);
            assert_eq!(
                seen,
                vec![
                    (0, vec![], false, Some(valid.clone())),
                    (0, vec![1, 2], false, None),
                    (1, vec![], false, Some(cleared.clone())),
                    (1, vec![3, 4], false, None),
                    (2, vec![], true, None),
                ]
            );
            // The existing frontend accepts this as a replacement snapshot;
            // absent metadata on ordinary PCM events is not a clear command.
            assert!(cleared.validate().is_ok());
        }
        let next = stream_from_items(
            vec![
                ChunkItem::Captions(valid.clone()),
                ChunkItem::Pcm(vec![5, 6]),
            ],
            8000,
        );
        assert_eq!(
            drain_chunk_stream(next, AbortScope::begin(), |_, _, _, _| {})
                .unwrap()
                .0
                .captions,
            Some(valid)
        );
    }

    #[test]
    fn failed_item_mid_stream_is_an_error() {
        let stream = stream_from_items(
            vec![
                ChunkItem::Pcm(vec![1, 2]),
                ChunkItem::Failed("upstream connection reset".into()),
            ],
            16000,
        );
        let result = drain_chunk_stream(stream, AbortScope::begin(), |_, _, _, _| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("connection reset"));
    }

    #[test]
    fn empty_stream_is_an_error_not_silence() {
        let stream = stream_from_items(Vec::new(), 22050);
        assert!(drain_chunk_stream(stream, AbortScope::begin(), |_, _, _, _| {}).is_err());
    }

    #[test]
    fn terminal_marker_always_emitted_even_for_single_chunk() {
        let stream = stream_from_items(vec![ChunkItem::Pcm(vec![9, 9, 9])], 8000);
        let mut finals = 0;
        let (wav, _) = drain_chunk_stream(stream, AbortScope::begin(), |_, _, is_final, _| {
            if is_final {
                finals += 1;
            }
        })
        .unwrap();
        assert_eq!(finals, 1);
        assert_eq!(wav.bytes.len(), 44 + 3);
    }

    /// The JIT gate reads the fragment duration off the WAV header, never by
    /// decoding the samples: this is the Rust-side fallback when the caption
    /// carries no duration.
    #[test]
    fn jit_fragment_duration_comes_from_the_wav_header() {
        let meta = meta(24_000);
        // One second of 24 kHz mono 16-bit PCM.
        let wav = pcm_to_wav(&vec![0u8; 48_000], &meta);
        assert_eq!(wav_duration_ms(&wav), Some(1_000));
        assert_eq!(wav_duration_ms(b"not a wav payload"), None);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::history::HistoryEntry;

    fn saved(history: &Mutex<HistoryLog>, text: &str, voice: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Unique per call: tests share one process and run in parallel, so an
        // entries-length suffix can collide across tests.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "copyspeak-cache-test-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        std::fs::write(&path, pcm_to_wav(&[0, 0, 1, 0], &meta)).unwrap();
        history.lock().unwrap().add(HistoryEntry {
            id: text.to_string(),
            timestamp: chrono::Utc::now(),
            text: text.to_string(),
            text_length: text.len() as u32,
            tts_engine: "edge".into(),
            voice: voice.into(),
            speed: 1.0,
            output_format: None,
            output_path: Some(path.to_string_lossy().into_owned()),
            duration_ms: 0,
            batch_id: None,
            app_name: None,
            source: None,
            filters_applied: Vec::new(),
            success: true,
            error_message: None,
            attempts: 1,
            tags: Vec::new(),
            metadata: HashMap::new(),
        });
        path
    }

    #[test]
    fn replays_a_reading_only_when_every_fragment_is_saved() {
        let pagination = crate::config::PaginationConfig {
            enabled: true,
            fragment_size: 30,
        };
        let text = "First sentence goes here. Second sentence goes here.";
        let fragments = pagination::paginate_text(text, &pagination);
        assert_eq!(fragments.len(), 2);
        let history = Mutex::new(HistoryLog::new());
        let first = saved(&history, &fragments[0].text, "ava");
        assert!(find_cached_reading(&history, true, text, &pagination, "edge", "ava").is_none());

        let second = saved(&history, &fragments[1].text, "ava");
        let (joined, source) =
            find_cached_reading(&history, true, text, &pagination, "edge", "ava").unwrap();
        assert_eq!(source, CachedSource::History);
        assert_eq!(
            crate::audio::wav::parse_wav_header(&joined.bytes)
                .unwrap()
                .data_size,
            8,
            "both fragments' samples, in order"
        );
        assert!(find_cached_reading(&history, true, text, &pagination, "edge", "other").is_none());

        std::fs::remove_file(&second).unwrap();
        assert!(find_cached_reading(&history, true, text, &pagination, "edge", "ava").is_none());
        std::fs::remove_file(&first).unwrap();
    }

    #[test]
    fn speech_cache_evicts_oldest_entries_past_its_byte_budget() {
        let dir = std::env::temp_dir().join(format!(
            "copyspeak-speech-cache-budget-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..3 {
            let speech = SpeechAudio {
                bytes: vec![i as u8; 4],
                captions: None,
            };
            save_speech_cache(&dir, &speech, "edge", "ava", &format!("text {i}"));
            // Coarse mtime clocks can tie; keep write order observable.
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let cached = |i: usize| read_speech_cache(&dir, "edge", "ava", &format!("text {i}"));
        // Three 4-byte entries; an 8-byte budget keeps the two newest.
        evict_speech_cache(&dir, 8);
        assert!(cached(0).is_none(), "oldest entry is evicted");
        assert_eq!(cached(1).unwrap().bytes, vec![1u8; 4]);
        assert_eq!(cached(2).unwrap().bytes, vec![2u8; 4]);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

        // A budget below the newest entry keeps it: evicting it frees nothing.
        evict_speech_cache(&dir, 1);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert_eq!(cached(2).unwrap().bytes, vec![2u8; 4]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// History auto-delete can evict a reading's entry, but a repeat click must
    /// still replay from the content-addressed cache; while the history entry
    /// and its file exist, history wins.
    #[test]
    fn history_on_falls_back_to_the_content_addressed_cache() {
        let dir = std::env::temp_dir().join(format!(
            "copyspeak-speech-cache-fallback-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let history = Mutex::new(HistoryLog::new());
        let text = "A message read twice";
        let speech = SpeechAudio {
            bytes: b"saved audio".to_vec(),
            captions: None,
        };
        save_speech_cache(&dir, &speech, "elevenlabs", "matilda", text);

        // History is on but holds no entry: the cache answers the repeat.
        let (found, source) =
            find_cached_speech_in(&dir, &history, true, text, "elevenlabs", "matilda")
                .expect("cache replay when history has no entry");
        assert_eq!(found.bytes, b"saved audio");
        assert_eq!(source, CachedSource::SpeechCache);

        // While the history entry and its file exist, history wins.
        let history_engine = "edge";
        let history_voice = "ava";
        let path = saved(&history, text, history_voice);
        let (found, source) =
            find_cached_speech_in(&dir, &history, true, text, history_engine, history_voice)
                .expect("history replay when its entry exists");
        assert_eq!(source, CachedSource::History);
        assert!(found.bytes.starts_with(b"RIFF"));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cache_lookup_matches_profile_resolved_engine_not_active_backend() {
        use crate::config::{ProfileEngineOptions, TtsConfig, TtsEngine, VoiceProfile};

        let history = Mutex::new(HistoryLog::new());
        let text = "Profile-addressed sentence";
        let voice = "en_US-lessac";

        let mut tts = TtsConfig::default();
        tts.active_backend = TtsEngine::Kokoro;
        tts.profiles.push(VoiceProfile {
            id: "piper-prof".into(),
            name: "Piper Voice".into(),
            description: None,
            engine: TtsEngine::Piper,
            voice: voice.into(),
            voice_label: Some("Lessac".into()),
            speed: 1.0,
            pitch: 1.0,
            effects: Default::default(),
            post_process: Default::default(),
            text_processing: Default::default(),
            engine_options: ProfileEngineOptions::default_for(&TtsEngine::Piper),
        });

        // 1. Explicit profile addressing (speak_now_with_profile path)
        let eff_profile = resolve_effective_for_profile(&tts, Some("piper-prof")).unwrap();
        assert_eq!(eff_profile.engine, TtsEngine::Piper);
        let profile_engine_str = engine_str(&eff_profile.engine);
        let active_engine_str = engine_str(&tts.active_backend);
        assert_ne!(profile_engine_str, active_engine_str);

        let path = std::env::temp_dir().join(format!(
            "copyspeak-prof-cache-test-{}.wav",
            std::process::id()
        ));
        let meta = AudioFormatMeta {
            sample_rate: 8000,
            channels: 1,
            bits_per_sample: 16,
        };
        std::fs::write(&path, pcm_to_wav(&[0, 0, 1, 0], &meta)).unwrap();
        history.lock().unwrap().add(HistoryEntry {
            id: text.to_string(),
            timestamp: chrono::Utc::now(),
            text: text.to_string(),
            text_length: text.len() as u32,
            tts_engine: profile_engine_str.to_string(),
            voice: voice.into(),
            speed: 1.0,
            output_format: None,
            output_path: Some(path.to_string_lossy().into_owned()),
            duration_ms: 0,
            batch_id: None,
            app_name: None,
            source: None,
            filters_applied: Vec::new(),
            success: true,
            error_message: None,
            attempts: 1,
            tags: Vec::new(),
            metadata: HashMap::new(),
        });

        // Cache hit using resolved profile engine
        let found = find_cached_speech(&history, true, text, profile_engine_str, voice);
        assert!(
            found.is_some(),
            "Must hit cache when queried with resolved profile engine"
        );

        // Cache miss if queried with active backend engine
        let miss = find_cached_speech(&history, true, text, active_engine_str, voice);
        assert!(
            miss.is_none(),
            "Must not hit cache if queried with differing active backend engine"
        );

        // 2. Active profile resolution path (speak_queued path)
        tts.active_profile_id = "piper-prof".into();
        let eff_queued = resolve_effective(&tts);
        assert_eq!(eff_queued.engine, TtsEngine::Piper);
        let queued_engine_str = engine_str(&eff_queued.engine);
        let found_queued = find_cached_speech(&history, true, text, queued_engine_str, voice);
        assert!(
            found_queued.is_some(),
            "speak_queued path must hit cache using resolved profile engine"
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn same_text_cache_keys_on_resolved_profile_engine() {
        let dir = std::env::temp_dir().join(format!(
            "copyspeak-prof-speech-cache-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let speech = SpeechAudio {
            bytes: vec![1, 2, 3, 4],
            captions: None,
        };

        save_speech_cache(&dir, &speech, "piper", "en_US-lessac", "Hello world");
        assert!(read_speech_cache(&dir, "piper", "en_US-lessac", "Hello world").is_some());
        assert!(read_speech_cache(&dir, "kokoro", "en_US-lessac", "Hello world").is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A repeat reading must replay a fragment an earlier early stop saved. A
    /// fresh read (`regenerate_now`) must synthesize instead, and a streamed
    /// fragment has no complete WAV to replay.
    #[test]
    fn a_paginated_fragment_replays_only_for_a_saved_audio_repeat() {
        let dir =
            std::env::temp_dir().join(format!("copyspeak-fragment-replay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let history = Mutex::new(HistoryLog::new());
        let speech = SpeechAudio {
            bytes: b"saved fragment".to_vec(),
            captions: None,
        };
        save_speech_cache(&dir, &speech, "edge", "ava", "first fragment");
        let read = |reuse_saved_audio: bool, streaming: bool| {
            find_cached_fragment_in(
                &dir,
                &history,
                false,
                reuse_saved_audio,
                streaming,
                "first fragment",
                "edge",
                "ava",
            )
        };

        assert_eq!(
            read(true, false)
                .expect("a repeat replays the saved fragment")
                .0
                .bytes,
            b"saved fragment".to_vec()
        );
        assert!(
            read(false, false).is_none(),
            "a fresh read must synthesize the fragment"
        );
        assert!(
            read(true, true).is_none(),
            "a streamed fragment has no complete WAV to replay"
        );
        assert!(
            find_cached_fragment_in(
                &dir,
                &history,
                false,
                true,
                false,
                "first fragment",
                "edge",
                "other voice",
            )
            .is_none(),
            "a voice change must not replay the fragment"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
