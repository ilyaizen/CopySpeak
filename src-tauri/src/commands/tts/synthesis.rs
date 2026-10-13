// TTS synthesis commands — speak_now, speak_queued, speak_history_entry.

use crate::audio::{self, AudioPlayer};
use crate::browser_bridge::{self, SpeakTrigger, TriggerSource};
use crate::config::{self, AppConfig};
use crate::fragment_queue::FragmentQueue;
use crate::history::{self, HistoryLog};
use crate::hud;
use crate::pagination;
use crate::telemetry;
use crate::tts::captions::{read_sidecar, write_sidecar, CaptionAlignment, SpeechAudio};
use crate::tts::stream::{pcm_to_wav, AudioFormatMeta, ChunkItem, ChunkStream};
use crate::tts::TtsBackend;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

use super::helpers::{
    active_engine, create_backend, create_backend_from_effective, engine_identifier, engine_str,
    resolve_effective, voice_display_name, SynthesisGuard,
};
use crate::commands::{AudioFragmentEvent, AudioStreamChunkEvent, CachedAudio, PaginationEvent};

// ── Helper Functions ──────────────────────────────────────────────────────────

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

/// Synthesize audio using spawn_blocking
async fn synthesize_async(
    backend: Arc<Box<dyn TtsBackend>>,
    text: String,
    voice: String,
) -> Result<SpeechAudio, String> {
    tokio::task::spawn_blocking(move || backend.synthesize_with_captions(&text, &voice))
        .await
        .map_err(|e| format!("Task join error: {e}"))?
        .map_err(|e| e.to_string())
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
fn emit_audio_ready(app: &AppHandle, speech: &SpeechAudio, text: &str, session_id: Option<&str>) {
    emit_audio_fragment(app, speech, 0, 1, text.to_owned(), session_id);
}

/// Emit audio-fragment-ready event for streaming playback
fn emit_audio_fragment(
    app: &AppHandle,
    speech: &SpeechAudio,
    index: usize,
    total: usize,
    text: String,
    session_id: Option<&str>,
) {
    let wav_bytes = &speech.bytes;
    if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
        return;
    }
    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(wav_bytes);
    let is_final = index == total - 1;
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
    if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
        return;
    }
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
    mut on_chunk: impl FnMut(usize, &[u8], bool, Option<&CaptionAlignment>),
) -> Result<(SpeechAudio, u64), String> {
    let start = Instant::now();
    let meta = stream.meta.clone();
    let mut accumulated: Vec<u8> = Vec::new();
    let mut chunk_index = 0usize;
    let mut captions = None;
    let mut ttfa_ms: Option<u64> = None;

    loop {
        // Dropping the receiver is what actually cancels a streaming request:
        // every producer stops pulling its source once its send fails.
        if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
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

/// Whether a fresh synthesis may play as live `audio-stream-chunk` events.
/// The frontend PCM scheduler wires straight into `ctx.destination` with no
/// effect chain, so an enabled profile effect must force the fragment path,
/// where `buildPlaybackUrl` runs `effect.process` before playback.
/// A cold backend (daemon still loading) takes the batch path until it is ready.
fn allows_streaming_playback(
    streaming_enabled: bool,
    backend_streams: bool,
    runtime_ready: bool,
    effects_enabled: bool,
) -> bool {
    streaming_enabled && backend_streams && runtime_ready && !effects_enabled
}

/// Synthesize via the backend's streaming path, forwarding each PCM chunk to
/// the frontend as an `audio-stream-chunk` event while accumulating the full
/// payload into a WAV container for history/cache/envelope handling.
async fn synthesize_streaming_and_emit(
    app: &AppHandle,
    backend: Arc<Box<dyn TtsBackend>>,
    text: String,
    voice: String,
    fragment_index: usize,
    emit_final: bool,
    session_id: Option<String>,
) -> Result<SpeechAudio, String> {
    let app = app.clone();
    tokio::task::spawn_blocking(move || {
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
        drain_chunk_stream(stream, |chunk_idx, pcm, is_final, captions| {
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
            emit_audio_stream_chunk(
                &app,
                &meta,
                pcm,
                fragment_index,
                is_final && emit_final,
                (chunk_idx == 0).then_some(text.as_str()),
                duration_ms,
                captions,
                session_id.as_deref(),
            );
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
    .map(|(wav, _ttfa_ms)| wav)
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

/// Same-text cache used while history is off: audio files in their own folder,
/// never logged. The newest `CACHE_LIMIT` files survive.
const CACHE_LIMIT: usize = 15;

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
    evict_speech_cache(dir);
}

/// Drop the oldest cached audio, and its captions, past `CACHE_LIMIT`.
fn evict_speech_cache(dir: &std::path::Path) {
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
    let excess = files.len().saturating_sub(CACHE_LIMIT);
    for (_, path) in files.into_iter().take(excess) {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("[TTS] Cannot evict cached audio: {e}");
        }
        crate::tts::captions::remove_sidecar(&path.to_string_lossy());
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

/// Saved audio (and caption sidecar) for this exact text, engine and voice.
/// With history on, that is the newest successful history entry; with history
/// off, the same-text cache. `None` when nothing matches or the file is gone
/// or unreadable — the caller then synthesizes as usual.
fn find_cached_speech(
    history: &Mutex<HistoryLog>,
    history_on: bool,
    text: &str,
    engine: &str,
    voice: &str,
) -> Option<SpeechAudio> {
    if !history_on {
        return read_speech_cache(&speech_cache_dir(), engine, voice, text);
    }
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

/// Saved audio for a whole reading: one entry with this text, or else every
/// fragment `speak_queued` would split it into, joined in order.
fn find_cached_reading(
    history: &Mutex<HistoryLog>,
    history_on: bool,
    text: &str,
    pagination_config: &crate::config::PaginationConfig,
    engine: &str,
    voice: &str,
) -> Option<SpeechAudio> {
    if let Some(speech) = find_cached_speech(history, history_on, text, engine, voice) {
        return Some(speech);
    }
    let fragments = pagination::paginate_text(text, pagination_config);
    if fragments.len() < 2 {
        return None;
    }
    let parts = fragments
        .into_iter()
        .map(|f| {
            let speech = find_cached_speech(history, history_on, &f.text, engine, voice)?;
            Some((speech, f.text))
        })
        .collect::<Option<Vec<_>>>()?;
    crate::tts::captions::concat_speech(parts)
        .map_err(|e| log::warn!("[TTS] Cannot join saved fragments: {e}. Re-synthesizing."))
        .ok()
}

/// Play audio that history already holds: no new entry, file or telemetry
/// sample, since nothing was synthesized.
fn replay_saved_audio(app: &AppHandle, speech: &SpeechAudio, text: &str, session_id: Option<&str>) {
    cache_audio(app, speech, text);
    hud::show_hud(
        app,
        extract_envelope_or_default(&speech.bytes),
        Some(text.to_string()),
    );
    emit_audio_ready(app, speech, text, session_id);
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
    condense: Option<bool>,
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
        condense.unwrap_or(false),
        trigger,
    )
    .await;
    if let (Err(_), Some(id)) = (&result, trigger_id) {
        browser_bridge::fail_trigger(&app, &id);
    }
    result
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
    )
    .await
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
    condense: bool,
    trigger: Option<(TriggerSource, String)>,
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

    let lock_state = app.state::<tokio::sync::Mutex<()>>();
    let _queue_lock = lock_state.lock().await;

    // Clear any previous abort request
    crate::ABORT_REQUESTED.store(false, Ordering::Relaxed);

    // Enter critical section for TTS synthesis
    let _synthesis_guard = SynthesisGuard::new(&app);

    let (
        active_backend,
        tts_config,
        output_config,
        pagination_config,
        post_process_config,
        sanitization,
        streaming_enabled,
    ) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.active_backend.clone(),
            cfg.tts.clone(),
            cfg.output.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
            cfg.sanitization.clone(),
            cfg.playback.streaming_enabled,
        )
    };

    // Optional LLM post-processing (e.g., Groq) — best-effort; falls back to
    // the input text on any failure so synthesis is never blocked. Re-runs
    // sanitization on the condenser OUTPUT (see try_process).
    let text =
        crate::post_process::try_process(text, &post_process_config, &sanitization, condense).await;
    let _ = app.emit("reading-started", &text);
    // File output plays nothing, so there is no speak for a page to follow.
    if let Some((trigger, id)) = &trigger {
        if !(output_config.enabled && !output_config.directory.is_empty()) {
            browser_bridge::begin_trigger(&app, id, trigger, &text);
        }
    }

    log_tts_debug("TTS", &format!("{:?}", active_backend), &text);

    let eff = resolve_effective(&tts_config);
    let voice = eff.voice.clone();

    let backend: Box<dyn TtsBackend> = create_backend_from_effective(&eff, &tts_config);
    let engine_str_for_cache = engine_str(&active_backend);

    let history_on = config.lock().unwrap().history.enabled;
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
    let cache_hit = cached.is_some();

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

    // Streaming backends forward PCM chunks during synthesis in playback mode;
    // batch backends (and cache hits / file output / pagination) take the
    // untouched existing branches.
    let streaming_playback = allows_streaming_playback(
        streaming_enabled,
        backend_arc.supports_streaming(),
        backend_arc.runtime_ready(),
        eff.effects.enabled,
    ) && !cache_hit
        && !output_config.enabled;
    let (wav_bytes, already_streamed) = if let Some(speech) = cached {
        log::info!("[TTS] Replaying saved audio from history instead of synthesizing");
        (speech, false)
    } else if pagination::should_paginate(&text, &pagination_config) && !output_config.enabled {
        // Pagination comes first even for streaming engines: bounded fragments
        // keep each request under the provider's input limit (OpenAI rejects
        // anything past 4096 characters). Each fragment then streams on its own,
        // so long text is both accepted and low-latency.
        let wav = synthesize_paginated(
            &app,
            backend_arc.clone(),
            &text,
            &voice,
            &active_backend,
            &telemetry_state,
            &synthesis_start,
            estimated_ms,
            confidence,
            streaming_playback,
            session_id.clone(),
        )
        .await?;
        (wav, streaming_playback)
    } else if streaming_playback {
        let wav = synthesize_streaming_and_emit(
            &app,
            backend_arc.clone(),
            text.clone(),
            voice.clone(),
            0,
            true,
            session_id.clone(),
        )
        .await?;
        (wav, true)
    } else {
        // Simple synthesis
        let wav = synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?;
        (wav, false)
    };

    // Both the paginated and streaming paths return empty bytes when the user
    // aborts mid-synthesis. do_abort_synthesis has already stopped playback and
    // emitted synthesis-aborted, so there is nothing to save or play here.
    if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
        log::info!("[TTS] Synthesis aborted; nothing to play or save");
        return Ok(());
    }

    let synthesis_duration = synthesis_start.elapsed();
    let synthesis_ms = synthesis_duration.as_millis() as u64;

    // A replay took no synthesis time; recording it would skew estimates.
    if !cache_hit {
        record_telemetry(
            &telemetry_state,
            &engine_str(&active_backend),
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
            &active_backend,
            &output_config,
            synthesis_ms,
        );
    }

    if cache_hit {
        replay_saved_audio(&app, &wav_bytes, &text, session_id.as_deref());
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
        &active_backend,
        &tts_config,
        backend_arc,
        synthesis_ms,
        already_streamed,
        eff.voice_label.as_deref(),
        session_id.as_deref(),
    )
}

/// Synthesize paginated text with progress updates
async fn synthesize_paginated(
    app: &AppHandle,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    text: &str,
    voice: &str,
    active_backend: &crate::config::TtsEngine,
    telemetry_state: &State<'_, Mutex<telemetry::TelemetryLog>>,
    synthesis_start: &Instant,
    _total_estimate: Option<u64>,
    _avg_confidence: f32,
    streaming: bool,
    session_id: Option<String>,
) -> Result<SpeechAudio, String> {
    let pagination_config = crate::config::PaginationConfig::default();
    let fragments = pagination::paginate_text(text, &pagination_config);

    if fragments.len() <= 1 {
        // Only one fragment — fall back to normal synthesis
        return if streaming {
            synthesize_streaming_and_emit(
                app,
                backend_arc.clone(),
                text.to_string(),
                voice.to_string(),
                0,
                true,
                session_id,
            )
            .await
        } else {
            synthesize_async(backend_arc.clone(), text.to_string(), voice.to_string()).await
        };
    }

    log::info!(
        "[TTS] Paginating text ({} chars) into {} fragments",
        text.len(),
        fragments.len()
    );

    let char_counts: Vec<usize> = fragments.iter().map(|f| f.text.len()).collect();
    let (total_estimate, avg_confidence, _) = telemetry::get_estimate_paginated(
        telemetry_state,
        &engine_str(active_backend),
        voice,
        &char_counts,
    );

    let mut fragment_audio = Vec::new();
    for (i, fragment) in fragments.iter().enumerate() {
        // Check for abort
        if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
            log::info!("[TTS] Pagination aborted by user");
            return Ok(Vec::new().into());
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

        log::info!(
            "[TTS] Synthesizing fragment {}/{} ({} chars)",
            i + 1,
            fragments.len(),
            fragment.text.len()
        );

        let frag_wav = if streaming {
            synthesize_streaming_and_emit(
                app,
                backend_arc.clone(),
                fragment.text.clone(),
                voice.to_string(),
                i,
                i + 1 == fragments.len(),
                session_id.clone(),
            )
            .await
        } else {
            synthesize_async(
                backend_arc.clone(),
                fragment.text.clone(),
                voice.to_string(),
            )
            .await
        }
        .map_err(|e| format!("Fragment {} synthesis failed: {}", i + 1, e))?;

        if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
            return Ok(Vec::new().into());
        }
        fragment_audio.push((frag_wav, fragment.text.clone()));
    }

    crate::tts::captions::concat_speech(fragment_audio)
        .map_err(|e| format!("Failed to concatenate audio fragments: {e}"))
}

/// Handle file output mode
fn handle_file_output(
    app: &AppHandle,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    speech: &SpeechAudio,
    text: &str,
    voice: &str,
    active_backend: &crate::config::TtsEngine,
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

    let history_on = config.lock().unwrap().history.enabled;
    if history_on {
        add_history_with_metadata(
            history,
            text,
            &engine_str(active_backend),
            voice,
            audio_duration_ms,
            Some(output_path.to_string_lossy().into_owned()),
            None,
            synthesis_ms,
            None,
        );
        let _ = app.emit("history-updated", ());
    } else {
        save_speech_cache(
            &speech_cache_dir(),
            speech,
            &engine_str(active_backend),
            voice,
            text,
        );
    }

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
    active_backend: &crate::config::TtsEngine,
    tts_config: &crate::config::TtsConfig,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    synthesis_ms: u64,
    already_streamed: bool,
    voice_label: Option<&str>,
    session_id: Option<&str>,
) -> Result<(), String> {
    let wav_bytes = &speech.bytes;
    let envelope = extract_envelope_or_default(wav_bytes);

    let engine_id = engine_identifier(active_backend);
    let voice_name = voice_display_name(active_backend, tts_config, voice, voice_label);
    let audio_ext = backend_arc.file_extension().to_string();

    let history_on = config.lock().unwrap().history.enabled;
    if history_on {
        let history_path =
            save_to_history_storage(config, speech, &engine_id, &voice_name, &audio_ext);
        add_history_with_metadata(
            history,
            text,
            &engine_str(active_backend),
            voice,
            envelope.duration_ms,
            history_path,
            None,
            synthesis_ms,
            None,
        );
        let _ = app.emit("history-updated", ());
    } else {
        save_speech_cache(
            &speech_cache_dir(),
            speech,
            &engine_str(active_backend),
            voice,
            text,
        );
    }

    // Cache audio for replay
    cache_audio(app, speech, text);

    // Show HUD with waveform visualization
    hud::show_hud(app, envelope, Some(text.to_string()));

    // Emit audio to frontend for browser-native playback. Skipped when PCM
    // chunks were already forwarded by the streaming synthesis path.
    if !already_streamed {
        emit_audio_ready(app, speech, text, session_id);
    }

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
        active_backend,
        tts_config,
        pagination_config,
        post_process_config,
        sanitization,
        streaming_enabled,
    ) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.active_backend.clone(),
            cfg.tts.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
            cfg.sanitization.clone(),
            cfg.playback.streaming_enabled,
        )
    };

    // Clear any previous abort request, the same way speak_now_internal does.
    // Both commands feed drain_chunk_stream, which now honours the flag, so a
    // stale `true` from an earlier abort would kill this run's first stream.
    crate::ABORT_REQUESTED.store(false, Ordering::Relaxed);

    // Optional LLM post-processing — best-effort; falls back on failure.
    // speak_queued callers (double-copy, browser readings) never request
    // condensing; re-sanitizes condenser output like speak_now does.
    let text =
        crate::post_process::try_process(text, &post_process_config, &sanitization, false).await;
    let _ = app.emit("reading-started", &text);
    if let Some((trigger, id)) = &trigger {
        browser_bridge::begin_trigger(&app, id, trigger, &text);
    }

    log_tts_debug("Queue", &format!("{:?}", active_backend), &text);

    // Show HUD with synthesizing indicator
    hud::show_hud_synthesizing(&app, Some(text.clone()));

    // Paginate text
    let fragments = pagination::paginate_text(&text, &pagination_config);

    if crate::logging::is_debug_mode() {
        log::debug!("[Queue] Created {} fragments", fragments.len());
    }

    let eff = resolve_effective(&tts_config);
    let voice = eff.voice.clone();

    // Get telemetry estimates
    let char_counts: Vec<usize> = fragments.iter().map(|f| f.text.len()).collect();
    let (total_estimate, avg_confidence, _) = telemetry::get_estimate_paginated(
        &telemetry_state,
        &engine_str(&active_backend),
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

    let engine_str_val = engine_str(&active_backend);
    let engine_id_val = engine_identifier(&active_backend);

    // Repeated text replays saved audio — only when the saved-audio store holds
    // every fragment, so a reading never mixes saved and fresh parts nor leaves
    // a partial batch in history.
    let history_on = config.lock().unwrap().history.enabled;
    let mut saved: Option<std::vec::IntoIter<SpeechAudio>> = fragments
        .iter()
        .map(|f| find_cached_speech(&history, history_on, &f.text, &engine_str_val, &voice))
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_iter);
    let replaying = saved.is_some();
    if replaying {
        log::info!("[Queue] Replaying saved audio for all {} fragments", total);
    }

    // Synthesize and play each fragment
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
        let streamed = !replaying
            && allows_streaming_playback(
                streaming_enabled,
                backend_arc.supports_streaming(),
                backend_arc.runtime_ready(),
                eff.effects.enabled,
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
                index + 1 == total,
                session_id.clone(),
            )
            .await?
        } else {
            synthesize_async(backend_arc.clone(), fragment.text.clone(), voice.clone()).await?
        };
        let fragment_duration = fragment_start.elapsed();
        if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
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

        // Save to history (a replay is already there). With history off the
        // audio goes to the same-text cache instead, and nothing is logged.
        if !replaying && !history_on {
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
            let voice_name = voice_display_name(
                &active_backend,
                &tts_config,
                &voice,
                eff.voice_label.as_deref(),
            );
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
    crate::ABORT_REQUESTED.store(false, Ordering::Relaxed);
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
        (cfg.tts.clone(), cfg.history.enabled)
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
    let wav_bytes = synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?;
    if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
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
    } else {
        save_speech_cache(
            &speech_cache_dir(),
            &wav_bytes,
            &engine_str_val,
            &voice,
            &text,
        );
    }

    // Show HUD with waveform visualization
    hud::show_hud(&app, envelope, Some(text.clone()));

    // Emit audio to frontend
    emit_audio_ready(&app, &wav_bytes, &text, None);

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
        let (wav, _ttfa) = drain_chunk_stream(stream, |idx, pcm, is_final, _| {
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
    fn enabled_effect_forces_fragment_path_over_stream_chunks() {
        // Streaming on, warm streaming backend, no effect: live chunks.
        assert!(allows_streaming_playback(true, true, true, false));
        // Any kill switch routes the reading through audio-fragment-ready,
        // the only path where buildPlaybackUrl applies the effect.
        assert!(!allows_streaming_playback(false, true, true, false));
        assert!(!allows_streaming_playback(true, false, true, false));
        assert!(!allows_streaming_playback(true, true, true, true));
    }

    #[test]
    fn cold_backend_takes_batch_path_until_ready() {
        // A static streaming claim alone must not stream: a cold CLI daemon
        // would otherwise run a one-shot command through the stream branch.
        assert!(!allows_streaming_playback(true, true, false, false));
        assert!(allows_streaming_playback(true, true, true, false));
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
            let (speech, _) = drain_chunk_stream(stream, |idx, pcm, is_final, captions| {
                seen.push((idx, pcm.to_vec(), is_final, captions.cloned()));
            })
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
            drain_chunk_stream(next, |_, _, _, _| {})
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
        let result = drain_chunk_stream(stream, |_, _, _, _| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("connection reset"));
    }

    #[test]
    fn empty_stream_is_an_error_not_silence() {
        let stream = stream_from_items(Vec::new(), 22050);
        assert!(drain_chunk_stream(stream, |_, _, _, _| {}).is_err());
    }

    #[test]
    fn terminal_marker_always_emitted_even_for_single_chunk() {
        let stream = stream_from_items(vec![ChunkItem::Pcm(vec![9, 9, 9])], 8000);
        let mut finals = 0;
        let (wav, _) = drain_chunk_stream(stream, |_, _, is_final, _| {
            if is_final {
                finals += 1;
            }
        })
        .unwrap();
        assert_eq!(finals, 1);
        assert_eq!(wav.bytes.len(), 44 + 3);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::history::HistoryEntry;

    fn saved(history: &Mutex<HistoryLog>, text: &str, voice: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "copyspeak-cache-test-{}-{}.wav",
            std::process::id(),
            history.lock().unwrap().entries().len()
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
        let joined = find_cached_reading(&history, true, text, &pagination, "edge", "ava").unwrap();
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
    fn same_text_cache_keeps_only_the_newest_entries() {
        let dir =
            std::env::temp_dir().join(format!("copyspeak-speech-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..=CACHE_LIMIT {
            let speech = SpeechAudio {
                bytes: vec![i as u8; 4],
                captions: None,
            };
            save_speech_cache(&dir, &speech, "edge", "ava", &format!("text {i}"));
            // Coarse mtime clocks can tie; keep write order observable.
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let cached = |i: usize| read_speech_cache(&dir, "edge", "ava", &format!("text {i}"));
        assert!(cached(0).is_none(), "oldest entry is evicted");
        assert_eq!(
            cached(CACHE_LIMIT).unwrap().bytes,
            vec![CACHE_LIMIT as u8; 4]
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), CACHE_LIMIT);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
