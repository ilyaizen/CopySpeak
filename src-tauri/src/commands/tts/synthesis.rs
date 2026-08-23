// TTS synthesis commands — speak_now, speak_queued, speak_history_entry.

use crate::audio::{self, AudioPlayer};
use crate::config::{self, AppConfig};
use crate::fragment_queue::FragmentQueue;
use crate::history::{self, HistoryLog};
use crate::hud;
use crate::pagination;
use crate::telemetry;
use crate::tts::TtsBackend;
use crate::tts::stream::{pcm_to_wav, AudioFormatMeta, ChunkItem, ChunkStream};
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
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || backend.synthesize(&text, &voice))
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
fn emit_audio_ready(app: &AppHandle, wav_bytes: &[u8]) {
    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(wav_bytes);
    if let Err(e) = app.emit("audio-ready", encoded) {
        log::warn!("Failed to emit audio-ready: {}", e);
    }
}

/// Emit audio-fragment-ready event for streaming playback
fn emit_audio_fragment(
    app: &AppHandle,
    wav_bytes: &[u8],
    index: usize,
    total: usize,
    text: String,
) {
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
    mut on_chunk: impl FnMut(usize, &[u8], bool),
) -> Result<(Vec<u8>, u64), String> {
    let start = Instant::now();
    let meta = stream.meta.clone();
    let mut accumulated: Vec<u8> = Vec::new();
    let mut chunk_index = 0usize;
    let mut ttfa_ms: Option<u64> = None;

    loop {
        match stream.recv() {
            Some(ChunkItem::Pcm(bytes)) => {
                if ttfa_ms.is_none() {
                    ttfa_ms = Some(start.elapsed().as_millis() as u64);
                }
                accumulated.extend_from_slice(&bytes);
                on_chunk(chunk_index, &bytes, false);
                chunk_index += 1;
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
    on_chunk(chunk_index, &[], true);

    let total_ms = start.elapsed().as_millis() as u64;
    log::info!(
        "[TTS][stream] ended: {} PCM bytes in {} chunks, {}ms total, TTFA {}ms",
        accumulated.len(),
        chunk_index,
        total_ms,
        ttfa_ms.unwrap_or(0)
    );
    Ok((pcm_to_wav(&accumulated, &meta), ttfa_ms.unwrap_or(0)))
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
) -> Result<Vec<u8>, String> {
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
                log::error!("[TTS][stream] failed during opening: {} (0 bytes received)", e);
                return Err(e.to_string());
            }
        };

        let meta = stream.meta.clone();
        drain_chunk_stream(stream, |chunk_idx, pcm, is_final| {
            if !is_final && chunk_idx == 0 {
                log::info!(
                    "[TTS][stream] first chunk playing after {}ms",
                    opened_at.elapsed().as_millis()
                );
            }
            emit_audio_stream_chunk(&app, &meta, pcm, fragment_index, is_final);
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
    .map(|(wav, _ttfa_ms)| wav)
}

/// Save audio to history storage and return path
fn save_to_history_storage(
    config: &State<'_, Mutex<AppConfig>>,
    wav_bytes: &[u8],
    engine_id: &str,
    voice_name: &str,
    audio_ext: &str,
) -> Option<String> {
    let history_config = config.lock().unwrap().history.clone();
    crate::history::save_audio_to_storage(
        &history_config,
        wav_bytes,
        engine_id,
        voice_name,
        audio_ext,
    )
}

/// Cache audio for replay
fn cache_audio(app: &AppHandle, wav_bytes: &[u8], text: &str) {
    let cache = app.state::<Mutex<CachedAudio>>();
    let mut cache = cache.lock().unwrap();
    cache.wav_bytes = Some(wav_bytes.to_vec());
    cache.text = Some(text.to_string());
}

// ── speak_now ───────────────────────────────────────────────────────────────

/// Manually trigger TTS for the given text (or current clipboard if empty).
#[tauri::command]
pub async fn speak_now(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    _player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
) -> Result<(), String> {
    speak_now_internal(app, config, _player, history, telemetry_state, text, None).await
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
) -> Result<(), String> {
    let text = get_text_or_clipboard(text)?;
    if text.trim().is_empty() {
        return Err("Nothing to speak".into());
    }

    let lock_state = app.state::<tokio::sync::Mutex<()>>();
    let _queue_lock = lock_state.lock().await;

    // Clear any previous abort request
    crate::ABORT_REQUESTED.store(false, Ordering::Relaxed);

    // Enter critical section for TTS synthesis
    let _synthesis_guard = SynthesisGuard::new(&app);

    let (active_backend, tts_config, output_config, pagination_config, post_process_config) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.active_backend.clone(),
            cfg.tts.clone(),
            cfg.output.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
        )
    };

    // Optional LLM post-processing (e.g., Groq) — best-effort; falls back to
    // the input text on any failure so synthesis is never blocked.
    let text = crate::post_process::try_process(text, &post_process_config).await;

    log_tts_debug("TTS", &format!("{:?}", active_backend), &text);

    let eff = resolve_effective(&tts_config);
    let voice = eff.voice.clone();

    let backend: Box<dyn TtsBackend> = create_backend_from_effective(&eff, &tts_config);
    let engine_str_for_cache = engine_str(&active_backend);

    // Check for cached audio in history
    let cached_path = {
        let hist = history.lock().unwrap();
        hist.entries().iter().rev().find_map(|e| {
            if e.text == text
                && e.voice == voice
                && e.tts_engine == engine_str_for_cache
                && e.success
            {
                e.output_path.as_ref().and_then(|path| {
                    if std::path::Path::new(path).exists() {
                        Some(path.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
    };

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
    let streaming_playback =
        backend_arc.supports_streaming() && cached_path.is_none() && !output_config.enabled;
    let (wav_bytes, already_streamed) = if let Some(ref path) = cached_path {
        // Try to read cached audio
        match std::fs::read(path) {
            Ok(bytes) => {
                log::info!("[TTS] Reusing cached audio from history: {}", path);
                (bytes, false)
            }
            Err(e) => {
                log::warn!(
                    "[TTS] Found cached history entry but failed to read audio file: {}. Re-synthesizing.",
                    e
                );
                let bytes =
                    synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?;
                (bytes, false)
            }
        }
    } else if streaming_playback {
        let wav =
            synthesize_streaming_and_emit(&app, backend_arc.clone(), text.clone(), voice.clone(), 0)
                .await?;
        (wav, true)
    } else if pagination::should_paginate(&text, &pagination_config) && !output_config.enabled {
        // Paginated synthesis for long text
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
        )
        .await?;
        (wav, false)
    } else {
        // Simple synthesis
        let wav = synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?;
        (wav, false)
    };

    let synthesis_duration = synthesis_start.elapsed();
    let synthesis_ms = synthesis_duration.as_millis() as u64;

    // Record telemetry
    record_telemetry(
        &telemetry_state,
        &engine_str(&active_backend),
        &voice,
        text.len(),
        synthesis_ms,
    );

    if crate::logging::is_debug_mode() {
        log::debug!("[TTS] Synthesis completed in {:?}", synthesis_duration);
        log::debug!("[TTS] Audio size: {} bytes", wav_bytes.len());
    }

    // Handle file output mode
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
) -> Result<Vec<u8>, String> {
    let pagination_config = crate::config::PaginationConfig::default();
    let fragments = pagination::paginate_text(text, &pagination_config);

    if fragments.len() <= 1 {
        // Only one fragment — fall back to normal synthesis
        return synthesize_async(backend_arc.clone(), text.to_string(), voice.to_string()).await;
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

    let mut fragment_wavs: Vec<Vec<u8>> = Vec::new();
    for (i, fragment) in fragments.iter().enumerate() {
        // Check for abort
        if crate::ABORT_REQUESTED.load(Ordering::Relaxed) {
            log::info!("[TTS] Pagination aborted by user");
            return Ok(Vec::new());
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

        let frag_wav = synthesize_async(
            backend_arc.clone(),
            fragment.text.clone(),
            voice.to_string(),
        )
        .await
        .map_err(|e| format!("Fragment {} synthesis failed: {}", i + 1, e))?;

        fragment_wavs.push(frag_wav);
    }

    audio::concat_wav_files(fragment_wavs)
        .map_err(|e| format!("Failed to concatenate audio fragments: {e}"))
}

/// Handle file output mode
fn handle_file_output(
    app: &AppHandle,
    _config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    wav_bytes: &[u8],
    text: &str,
    voice: &str,
    active_backend: &crate::config::TtsEngine,
    output_config: &crate::config::OutputConfig,
    synthesis_ms: u64,
) -> Result<(), String> {
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

    log::info!("Saved TTS audio to: {}", output_path.display());

    if crate::logging::is_debug_mode() {
        log::debug!("[TTS] Output file: {} bytes", audio_bytes.len());
    }

    let audio_duration_ms = audio::extract_envelope(wav_bytes, 1)
        .map(|e| e.duration_ms)
        .unwrap_or(0);

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

    Ok(())
}

/// Handle normal playback output
fn handle_playback_output(
    app: &AppHandle,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    wav_bytes: &[u8],
    text: &str,
    voice: &str,
    active_backend: &crate::config::TtsEngine,
    tts_config: &crate::config::TtsConfig,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    synthesis_ms: u64,
    already_streamed: bool,
    voice_label: Option<&str>,
) -> Result<(), String> {
    let envelope = extract_envelope_or_default(wav_bytes);

    let engine_id = engine_identifier(active_backend);
    let voice_name = voice_display_name(active_backend, tts_config, voice, voice_label);
    let audio_ext = backend_arc.file_extension().to_string();

    let history_path =
        save_to_history_storage(config, wav_bytes, &engine_id, &voice_name, &audio_ext);

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

    // Cache audio for replay
    cache_audio(app, wav_bytes, text);

    // Show HUD with waveform visualization
    hud::show_hud(app, envelope, Some(text.to_string()));

    // Emit audio to frontend for browser-native playback. Skipped when PCM
    // chunks were already forwarded by the streaming synthesis path.
    if !already_streamed {
        emit_audio_ready(app, wav_bytes);
    }

    Ok(())
}

// ── speak_queued ────────────────────────────────────────────────────────────

/// Speak text with fragment queue support for long texts.
/// Automatically paginates long texts and plays fragments sequentially.
#[tauri::command]
pub async fn speak_queued(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    _player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    queue: State<'_, Mutex<FragmentQueue>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
    text: Option<String>,
) -> Result<(), String> {
    let text = get_text_or_clipboard(text)?;
    if text.trim().is_empty() {
        return Err("Nothing to speak".into());
    }

    let (active_backend, tts_config, pagination_config, post_process_config) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.active_backend.clone(),
            cfg.tts.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
        )
    };

    // Optional LLM post-processing — best-effort; falls back on failure.
    let text = crate::post_process::try_process(text, &post_process_config).await;

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
        let streamed = backend_arc.supports_streaming();
        let wav_bytes = if streamed {
            synthesize_streaming_and_emit(
                &app,
                backend_arc.clone(),
                fragment.text.clone(),
                voice.clone(),
                index,
            )
            .await?
        } else {
            synthesize_async(backend_arc.clone(), fragment.text.clone(), voice.clone()).await?
        };
        let fragment_duration = fragment_start.elapsed();

        // Record telemetry
        record_telemetry(
            &telemetry_state,
            &engine_str_val,
            &voice,
            fragment.text.len(),
            fragment_duration.as_millis() as u64,
        );

        // Store audio in queue
        {
            let q = queue.lock().unwrap();
            q.set_audio(index, wav_bytes.clone());
        }

        log::debug!(
            "[Queue] Synthesized fragment {} ({} bytes)",
            index + 1,
            wav_bytes.len()
        );

        // Extract envelope
        let envelope = extract_envelope_or_default(&wav_bytes);

        // Transition HUD from synthesizing → playing on first fragment
        if index == 0 {
            hud::show_hud(&app, envelope.clone(), Some(text.clone()));
        }

        // Save to history
        let audio_ext = backend_arc.file_extension().to_string();
        let voice_name = voice_display_name(
            &active_backend,
            &tts_config,
            &voice,
            eff.voice_label.as_deref(),
        );
        let history_path =
            save_to_history_storage(&config, &wav_bytes, &engine_id_val, &voice_name, &audio_ext);

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

        // Emit audio fragment for streaming playback. Skipped when the PCM
        // chunks were already forwarded by the streaming path above.
        if !streamed {
            emit_audio_fragment(&app, &wav_bytes, index, total, fragment.text.clone());
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

    let tts_config = {
        let cfg = config.lock().unwrap();
        cfg.tts.clone()
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
    let envelope = extract_envelope_or_default(&wav_bytes);

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

    // Show HUD with waveform visualization
    hud::show_hud(&app, envelope, Some(text.clone()));

    // Emit audio to frontend
    emit_audio_ready(&app, &wav_bytes);

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
            drain_chunk_stream(stream, |idx, pcm, is_final| {
                seen.push((idx, pcm.to_vec(), is_final));
            })
            .expect("healthy stream drains");

        assert_eq!(seen.len(), 3, "two data chunks plus the terminal marker");
        assert_eq!(seen[0], (0, a.clone(), false));
        assert_eq!(seen[1], (1, b.clone(), false));
        assert!(seen[2].2, "terminal event must be marked final");
        assert!(seen[2].1.is_empty());

        // Wrapped WAV parses back to the concatenated PCM with the stream meta.
        let info = crate::audio::wav::parse_wav_header(&wav).expect("wrapped WAV valid");
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.data_size, 8);
        let expected = [a, b].concat();
        assert_eq!(&wav[info.data_offset..info.data_offset + info.data_size], &expected);
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
        let result = drain_chunk_stream(stream, |_, _, _| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("connection reset"));
    }

    #[test]
    fn empty_stream_is_an_error_not_silence() {
        let stream = stream_from_items(Vec::new(), 22050);
        assert!(drain_chunk_stream(stream, |_, _, _| {}).is_err());
    }

    #[test]
    fn terminal_marker_always_emitted_even_for_single_chunk() {
        let stream = stream_from_items(vec![ChunkItem::Pcm(vec![9, 9, 9])], 8000);
        let mut finals = 0;
        let (wav, _) = drain_chunk_stream(stream, |_, _, is_final| {
            if is_final {
                finals += 1;
            }
        })
        .unwrap();
        assert_eq!(finals, 1);
        assert_eq!(wav.len(), 44 + 3);
    }
}
