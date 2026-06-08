// TTS synthesis commands — speak_now, speak_queued, speak_history_entry.

use crate::audio::{self, AudioPlayer};
use crate::config::{self, AppConfig};
use crate::fragment_queue::FragmentQueue;
use crate::history::{self, HistoryLog};
use crate::hud;
use crate::pagination;
use crate::telemetry;
use crate::tts::TtsBackend;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

use super::helpers::{
    create_backend, engine_identifier, engine_str, voice_display_name, voice_for_backend,
    SynthesisGuard,
};
use crate::commands::{AudioFragmentEvent, CachedAudio, PaginationEvent};

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
    tokio::task::spawn_blocking(move || backend.synthesize(&text, &voice, 1.0))
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

/// Emit audio-ready event with base64 encoded audio.
/// Encoding runs on blocking thread pool to avoid stalling the async worker.
async fn emit_audio_ready(app: &AppHandle, wav_bytes: &[u8]) {
    use base64::{engine::general_purpose, Engine as _};
    let wav = wav_bytes.to_vec();
    let encoded = tokio::task::spawn_blocking(move || general_purpose::STANDARD.encode(&wav))
        .await
        .unwrap_or_default();
    if let Err(e) = app.emit("audio-ready", encoded) {
        log::warn!("Failed to emit audio-ready: {}", e);
    }
}

/// Emit audio-fragment-ready event for streaming playback.
/// Encoding runs on blocking thread pool to avoid stalling the async worker.
#[inline]
async fn emit_audio_fragment(
    app: &AppHandle,
    wav_bytes: &[u8],
    index: usize,
    total: usize,
    text: String,
) {
    use base64::{engine::general_purpose, Engine as _};
    let wav = wav_bytes.to_vec();
    let encoded = tokio::task::spawn_blocking(move || general_purpose::STANDARD.encode(&wav))
        .await
        .unwrap_or_default();
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

/// Spawn fragment emit in a background task so synthesis of the next fragment
/// can overlap with base64 encoding. Returns a JoinHandle that must be polled
/// before the loop advances to the next fragment to preserve emit order.
fn spawn_fragment_emit(
    app: &AppHandle,
    wav_bytes: &Vec<u8>,
    index: usize,
    total: usize,
    text: String,
) -> tokio::task::JoinHandle<()> {
    use base64::{engine::general_purpose, Engine as _};
    let wav = wav_bytes.clone();
    let app = app.clone();
    tokio::task::spawn(async move {
        let encoded = tokio::task::spawn_blocking(move || general_purpose::STANDARD.encode(&wav))
            .await
            .unwrap_or_default();
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
    })
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

    let backend: Box<dyn TtsBackend> = create_backend(&active_backend, &tts_config);
    let voice = voice_for_backend(&active_backend, &tts_config);
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

    let wav_bytes = if let Some(ref path) = cached_path {
        // Try to read cached audio
        match std::fs::read(path) {
            Ok(bytes) => {
                log::info!("[TTS] Reusing cached audio from history: {}", path);
                bytes
            }
            Err(e) => {
                log::warn!(
                    "[TTS] Found cached history entry but failed to read audio file: {}. Re-synthesizing.",
                    e
                );
                synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?
            }
        }
    } else if pagination::should_paginate(&text, &pagination_config) && !output_config.enabled {
        // Paginated synthesis for long text
        synthesize_paginated(
            &app,
            backend_arc.clone(),
            &text,
            &voice,
            &active_backend,
            &telemetry_state,
            &synthesis_start,
            &pagination_config,
            estimated_ms,
            confidence,
        )
        .await?
    } else {
        // Simple synthesis
        synthesize_async(backend_arc.clone(), text.clone(), voice.clone()).await?
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

    log::info!(
        "[TTS] Synthesis completed in {}ms, {} bytes",
        synthesis_ms, wav_bytes.len()
    );

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
    )
    .await
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
    pagination_config: &crate::config::PaginationConfig,
    _total_estimate: Option<u64>,
    _avg_confidence: f32,
) -> Result<Vec<u8>, String> {
    let fragments = pagination::paginate_text(text, pagination_config);

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
async fn handle_playback_output(
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
) -> Result<(), String> {
    let t_post = std::time::Instant::now();

    let t_env = std::time::Instant::now();
    let envelope = extract_envelope_or_default(wav_bytes);
    let env_ms = t_env.elapsed().as_millis() as u64;

    let engine_id = engine_identifier(active_backend, tts_config);
    let voice_name = voice_display_name(active_backend, tts_config, voice);
    let audio_ext = backend_arc.file_extension().to_string();

    let t_hist = std::time::Instant::now();
    let history_path =
        save_to_history_storage(config, wav_bytes, &engine_id, &voice_name, &audio_ext);
    let hist_ms = t_hist.elapsed().as_millis() as u64;

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

    // Emit audio to frontend for browser-native playback
    let t_emit = std::time::Instant::now();
    emit_audio_ready(app, wav_bytes).await;
    let emit_ms = t_emit.elapsed().as_millis() as u64;

    let post_ms = t_post.elapsed().as_millis() as u64;
    log::info!(
        "[TTS] Pipeline — synth:{synth_ms}ms env:{env_ms}ms hist:{hist_ms}ms emit:{emit_ms}ms total_post:{post_ms}ms",
        synth_ms = synthesis_ms,
        env_ms = env_ms,
        hist_ms = hist_ms,
        emit_ms = emit_ms,
        post_ms = post_ms
    );

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

    let (active_backend, tts_config, mut pagination_config, post_process_config) = {
        let cfg = config.lock().unwrap();
        (
            cfg.tts.active_backend.clone(),
            cfg.tts.clone(),
            cfg.pagination.clone(),
            cfg.post_process.clone(),
        )
    };

    // Adaptive fragment sizing: use telemetry to determine optimal fragment size.
    // Fast engines get larger fragments to reduce API call count.
    {
        let telemetry = telemetry_state.lock().unwrap();
        let engine_key = engine_str(&active_backend);
        let voice = voice_for_backend(&active_backend, &tts_config);
        // Use mid-size bucket estimate for sizing decision
        let bucket = telemetry::get_bucket_index(500);
        let timing_key = telemetry::TimingKey {
            backend: engine_key.to_string(),
            voice,
            bucket,
        };
        if let Some(stats) = telemetry.stats.get(&timing_key) {
            if stats.sample_count >= 3 {
                let adjusted = pagination::adaptive_fragment_size(
                    &pagination_config,
                    stats.chars_per_ms,
                );
                if adjusted != pagination_config.fragment_size as usize {
                    log::info!(
                        "[Queue] Adaptive fragment size: {} → {} (chars/ms: {:.1})",
                        pagination_config.fragment_size,
                        adjusted,
                        stats.chars_per_ms
                    );
                    pagination_config.fragment_size = adjusted as u32;
                }
            }
        }
    }

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

    let voice = voice_for_backend(&active_backend, &tts_config);

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
    let engine_id_val = engine_identifier(&active_backend, &tts_config);

    // Create backend once for all fragments (reuses HTTP connection pool / Piper server)
    let backend: Box<dyn TtsBackend> = create_backend(&active_backend, &tts_config);
    let backend_arc = Arc::new(backend);

    let is_parallel_capable = matches!(
        active_backend,
        crate::config::TtsEngine::OpenAI
            | crate::config::TtsEngine::ElevenLabs
            | crate::config::TtsEngine::Cartesia
    ) || (matches!(active_backend, crate::config::TtsEngine::Local)
        && tts_config.preset == "piper");

    // Parallel synthesis for cloud backends or Piper with multiple fragments
    if is_parallel_capable && fragments.len() > 1 {
        synthesize_queued_parallel(
            &app,
            backend_arc,
            &fragments,
            &voice,
            total,
            &config,
            &history,
            &telemetry_state,
            &engine_str_val,
            &engine_id_val,
            &tts_config,
            &active_backend,
            &batch_id,
        )
        .await;
    } else {
        // Sequential synthesis for CLI backends or single fragments
        synthesize_queued_sequential(
            &app,
            backend_arc,
            &fragments,
            &voice,
            total,
            &queue,
            &config,
            &history,
            &telemetry_state,
            &engine_str_val,
            &engine_id_val,
            &tts_config,
            &active_backend,
            &batch_id,
            synthesis_start,
            total_estimate,
            avg_confidence,
            total_chars,
        )
        .await?;
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

// ── Queued Synthesis Helpers ─────────────────────────────────────────────────

/// Sequential synthesis for CLI backends or single fragments.
async fn synthesize_queued_sequential(
    app: &AppHandle,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    fragments: &[pagination::TextFragment],
    voice: &str,
    total: usize,
    queue: &State<'_, Mutex<FragmentQueue>>,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    telemetry_state: &State<'_, Mutex<telemetry::TelemetryLog>>,
    engine_str_val: &str,
    engine_id_val: &str,
    tts_config: &crate::config::TtsConfig,
    active_backend: &crate::config::TtsEngine,
    batch_id: &Option<String>,
    synthesis_start: Instant,
    total_estimate: Option<u64>,
    avg_confidence: f32,
    total_chars: usize,
) -> Result<(), String> {
    let mut pending_emit: Option<tokio::task::JoinHandle<()>> = None;

    for (index, fragment) in fragments.iter().enumerate() {
        {
            let q = queue.lock().unwrap();
            if q.should_stop() {
                log::info!("[Queue] Playback stopped by user");
                q.clear();
                let _ = app.emit(
                    "pagination:stopped",
                    PaginationEvent { total, current_index: index, is_paginated: total > 1 },
                );
                return Ok(());
            }
        }

        log::debug!("[Queue] Synthesizing fragment {} of {}", index + 1, total);

        let _ = app.emit(
            "pagination:fragment-started",
            PaginationEvent { total, current_index: index, is_paginated: total > 1 },
        );

        let elapsed = synthesis_start.elapsed().as_millis() as u64;
        let processed_chars: usize = fragments.iter().take(index).map(|f| f.text.len()).sum();
        hud::emit_synthesis_progress(app, total_estimate, elapsed, index, total, true, avg_confidence, truncate_preview(&fragment.text, 50), total_chars, processed_chars);

        {
            let q = queue.lock().unwrap();
            q.set_current_index(index);
        }

        let fragment_start = Instant::now();
        let wav_bytes =
            synthesize_async(backend_arc.clone(), fragment.text.clone(), voice.to_string()).await?;
        let fragment_duration = fragment_start.elapsed();

        record_telemetry(telemetry_state, engine_str_val, voice, fragment.text.len(), fragment_duration.as_millis() as u64);

        {
            let q = queue.lock().unwrap();
            q.set_audio(index, wav_bytes.clone());
        }

        let envelope = extract_envelope_or_default(&wav_bytes);

        if index == 0 {
            hud::show_hud(app, envelope.clone(), None);
        }

        let audio_ext = backend_arc.file_extension().to_string();
        let voice_name = voice_display_name(active_backend, tts_config, voice);
        let history_path =
            save_to_history_storage(config, &wav_bytes, engine_id_val, &voice_name, &audio_ext);

        let mut metadata = HashMap::new();
        if total > 1 {
            metadata.insert("fragment_index".to_string(), serde_json::json!(index));
            metadata.insert("fragment_total".to_string(), serde_json::json!(total));
        }

        add_history_with_metadata(history, &fragment.text, engine_str_val, voice, envelope.duration_ms, history_path, batch_id.clone(), fragment_duration.as_millis() as u64, Some(metadata));

        // Await previous fragment's emit to preserve order, then spawn current emit.
        // This overlaps current synthesis with previous emit's base64 encoding.
        if let Some(handle) = pending_emit.take() {
            let _ = handle.await;
        }
        pending_emit = Some(spawn_fragment_emit(app, &wav_bytes, index, total, fragment.text.clone()));

        let _ = app.emit(
            "pagination:fragment-ready",
            PaginationEvent { total, current_index: index, is_paginated: total > 1 },
        );
        let _ = app.emit(
            "pagination:fragment-complete",
            PaginationEvent { total, current_index: index, is_paginated: total > 1 },
        );
    }

    // Await final emit
    if let Some(handle) = pending_emit.take() {
        let _ = handle.await;
    }

    let _ = app.emit("history-updated", ());
    Ok(())
}

/// Parallel synthesis for cloud backends or Piper with multiple fragments.
/// Processes all fragments with a concurrency cap, collecting results sorted
/// by index, then emits in order for correct sequential playback.
async fn synthesize_queued_parallel(
    app: &AppHandle,
    backend_arc: Arc<Box<dyn TtsBackend>>,
    fragments: &[pagination::TextFragment],
    voice: &str,
    total: usize,
    config: &State<'_, Mutex<AppConfig>>,
    history: &State<'_, Mutex<HistoryLog>>,
    _telemetry_state: &State<'_, Mutex<telemetry::TelemetryLog>>,
    engine_str_val: &str,
    engine_id_val: &str,
    tts_config: &crate::config::TtsConfig,
    active_backend: &crate::config::TtsEngine,
    batch_id: &Option<String>,
) {
    use tokio::task::JoinSet;
    const MAX_CONCURRENT: usize = 3;

    let mut join_set = JoinSet::new();
    let total_frags = fragments.len();
    let mut per_fragment_wavs: Vec<Option<Vec<u8>>> = vec![None; total_frags];
    let mut next_idx = 0usize;

    loop {
        while join_set.len() < MAX_CONCURRENT && next_idx < total_frags {
            let idx = next_idx;
            let backend = backend_arc.clone();
            let text = fragments[idx].text.clone();
            let voice = voice.to_string();
            let fragment = fragments[idx].clone();

            join_set.spawn(async move {
                let wav_bytes = synthesize_async(backend, text, voice).await?;
                Ok::<_, String>((idx as u32, fragment, wav_bytes))
            });
            next_idx += 1;
        }

        match join_set.join_next().await {
            Some(Ok(Ok((idx, _fragment, wav_bytes)))) => {
                let i = idx as usize;
                if i < per_fragment_wavs.len() {
                    per_fragment_wavs[i] = Some(wav_bytes);
                }
            }
            Some(Ok(Err(e))) => {
                log::error!("[Queue] Parallel fragment synthesis failed: {}", e);
            }
            Some(Err(e)) => {
                log::error!("[Queue] JoinSet task panicked: {}", e);
            }
            None => break,
        }
    }

    // Emit fragments in index order for correct sequential playback
    for idx in 0..total_frags {
        let wav_bytes = match &per_fragment_wavs[idx] {
            Some(w) => w,
            None => continue,
        };
        let fragment = &fragments[idx];
        let envelope = extract_envelope_or_default(wav_bytes);

        if idx == 0 {
            hud::show_hud(app, envelope.clone(), None);
        }

        let audio_ext = backend_arc.file_extension().to_string();
        let voice_name = voice_display_name(active_backend, tts_config, voice);
        let history_path = save_to_history_storage(
            config,
            wav_bytes,
            engine_id_val,
            &voice_name,
            &audio_ext,
        );

        let mut metadata = HashMap::new();
        if total > 1 {
            metadata.insert("fragment_index".to_string(), serde_json::json!(idx));
            metadata.insert("fragment_total".to_string(), serde_json::json!(total));
        }

        add_history_with_metadata(
            history,
            &fragment.text,
            engine_str_val,
            voice,
            envelope.duration_ms,
            history_path,
            batch_id.clone(),
            0,
            Some(metadata),
        );

        emit_audio_fragment(app, wav_bytes, idx, total, fragment.text.clone()).await;

        let _ = app.emit(
            "pagination:fragment-ready",
            PaginationEvent {
                total,
                current_index: idx,
                is_paginated: total > 1,
            },
        );
        let _ = app.emit(
            "pagination:fragment-complete",
            PaginationEvent {
                total,
                current_index: idx,
                is_paginated: total > 1,
            },
        );
    }

    let _ = app.emit("history-updated", ());
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

    let (active_backend, tts_config) = {
        let cfg = config.lock().unwrap();
        (cfg.tts.active_backend.clone(), cfg.tts.clone())
    };

    let backend: Box<dyn TtsBackend> = create_backend(&active_backend, &tts_config);
    let voice = original_voice;
    let engine_str_val = engine_str(&active_backend);
    let engine_id = engine_identifier(&active_backend, &tts_config);
    let voice_name = voice_display_name(&active_backend, &tts_config, &voice);
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
    emit_audio_ready(&app, &wav_bytes).await;

    log::info!("Re-spoke history entry: {}", entry_id);
    Ok(())
}
