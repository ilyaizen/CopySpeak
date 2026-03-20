// Playback control commands — stop, pause, skip, volume, speed, replay.

use crate::audio::{AmplitudeEnvelope, AudioPlayer, PlaybackState};
use crate::config::AppConfig;
use crate::history::HistoryLog;
use crate::hud;
use std::sync::Mutex;
use tauri::{Emitter, State};

use super::CachedAudio;

/// Abort any in-progress synthesis (kills CLI processes) and stop playback.
#[tauri::command]
pub fn abort_synthesis(app: tauri::AppHandle) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] abort_synthesis called");
    }
    crate::do_abort_synthesis(&app);
    Ok(())
}

/// Replay the most recently cached audio without re-synthesizing.
/// Emits "audio-ready" event to frontend with base64-encoded audio.
#[tauri::command]
pub fn replay_cached(
    app: tauri::AppHandle,
    cache: State<'_, Mutex<CachedAudio>>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] replay_cached called");
    }
    let wav_bytes = {
        let cache = cache.lock().unwrap();
        cache
            .wav_bytes
            .clone()
            .ok_or_else(|| "No cached audio to replay".to_string())?
    };

    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(&wav_bytes);
    if let Err(e) = app.emit("audio-ready", encoded) {
        log::warn!("Failed to emit audio-ready: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub fn stop_speaking(
    app: tauri::AppHandle,
    player: State<'_, Mutex<AudioPlayer>>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] stop_speaking called");
    }
    // Emit event so frontend audio element stops
    if let Err(e) = app.emit("playback-stop", ()) {
        log::warn!("Failed to emit playback-stop: {}", e);
    }
    // Keep Rodio stop as safety net
    let mut p = player.lock().unwrap();
    p.stop();
    p.set_playing_entry_id(None);
    Ok(())
}

#[tauri::command]
pub fn toggle_pause(
    app: tauri::AppHandle,
    player: State<'_, Mutex<AudioPlayer>>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] toggle_pause called");
    }
    // Emit event so frontend audio element toggles pause
    if let Err(e) = app.emit("playback-toggle-pause", ()) {
        log::warn!("Failed to emit playback-toggle-pause: {}", e);
    }
    // Keep Rodio toggle as safety net
    let mut p = player.lock().unwrap();
    p.toggle_pause();
    Ok(())
}

#[tauri::command]
pub fn skip_forward(
    player: State<'_, Mutex<AudioPlayer>>,
    seconds: Option<u64>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] skip_forward called (seconds: {:?})", seconds);
    }
    let mut p = player.lock().unwrap();
    p.skip_forward(seconds.unwrap_or(5));
    Ok(())
}

#[tauri::command]
pub fn skip_backward(
    player: State<'_, Mutex<AudioPlayer>>,
    seconds: Option<u64>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] skip_backward called (seconds: {:?})", seconds);
    }
    let mut p = player.lock().unwrap();
    p.skip_backward(seconds.unwrap_or(5));
    Ok(())
}

/// Set the playback speed (0.25–4.0). Saved to config; applied by frontend audio element.
#[tauri::command]
pub fn set_playback_speed(config: State<'_, Mutex<AppConfig>>, speed: f32) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_playback_speed called (speed: {})", speed);
    }
    let clamped = speed.clamp(0.25, 4.0);
    let mut cfg = config.lock().unwrap();
    cfg.playback.playback_speed = clamped;
    crate::config::save(&cfg)?;
    log::info!("Playback speed set to: {}", clamped);
    Ok(())
}

#[tauri::command]
pub fn get_playback_state(player: State<'_, Mutex<AudioPlayer>>) -> PlaybackState {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_playback_state called");
    }
    let p = player.lock().unwrap();
    p.get_state()
}

/// Set the playback volume (0-100). Saves to config; frontend applies to audio element.
#[tauri::command]
pub fn set_volume(config: State<'_, Mutex<AppConfig>>, volume: u8) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_volume called (volume: {})", volume);
    }

    let mut cfg = config.lock().unwrap();
    cfg.playback.volume = volume;
    crate::config::save(&cfg)?;

    log::info!("Volume set to: {}%", volume);
    Ok(())
}

/// Play audio from a history entry if it has an output file.
/// Emits "audio-ready" event to frontend with base64-encoded audio.
#[tauri::command]
pub fn play_history_entry(
    app: tauri::AppHandle,
    history: State<'_, Mutex<HistoryLog>>,
    entry_id: String,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] play_history_entry called (id: {})", entry_id);
    }

    let output_path = {
        let hist = history.lock().unwrap();
        let entry = hist
            .get_by_id(&entry_id)
            .ok_or_else(|| format!("History entry not found: {}", entry_id))?;

        entry
            .output_path
            .as_ref()
            .ok_or_else(|| "No audio file available for this entry".to_string())?
            .clone()
    };

    if !std::path::Path::new(&output_path).exists() {
        log::error!("[Audio] Audio file not found: {}", output_path);
        return Err(format!(
            "Audio file not found: {}. The file may have been deleted or moved.",
            output_path
        ));
    }

    let wav_bytes = std::fs::read(&output_path)
        .map_err(|e| {
            log::error!("[Audio] Failed to read audio file '{}': {}", output_path, e);
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    format!("Audio file not found: {}. The file may have been deleted.", output_path)
                }
                std::io::ErrorKind::PermissionDenied => {
                    format!("Permission denied reading audio file: {}. Check file permissions.", output_path)
                }
                _ => {
                    format!("Failed to read audio file '{}': {}. The file may be corrupted or inaccessible.", output_path, e)
                }
            }
        })?;

    if wav_bytes.is_empty() {
        log::error!("[Audio] Audio file is empty: {}", output_path);
        return Err(format!(
            "Audio file is empty: {}. The file may be corrupted.",
            output_path
        ));
    }

    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(&wav_bytes);
    if let Err(e) = app.emit("audio-ready", encoded) {
        log::warn!("Failed to emit audio-ready: {}", e);
    }

    log::info!("Playing history entry: {}", entry_id);
    Ok(())
}

/// Show the HUD window for playback of existing audio.
/// Called when playing history entries to display the HUD overlay.
#[tauri::command]
pub fn show_hud_for_playback(
    app: tauri::AppHandle,
    text: Option<String>,
    audio_duration_ms: Option<u64>,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] show_hud_for_playback called");
    }

    hud::show_hud_playback(&app, text, audio_duration_ms);
    Ok(())
}

/// Test command to show HUD with sample data.
/// Used in settings to preview the HUD overlay.
/// Automatically hides after 3 seconds since HUD is click-through.
#[tauri::command]
pub fn test_show_hud(app: tauri::AppHandle) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] test_show_hud called");
    }

    // Create sample amplitude envelope for testing
    let envelope = AmplitudeEnvelope {
        values: vec![
            0.1, 0.3, 0.5, 0.7, 0.9, 0.8, 0.6, 0.4, 0.2, 0.1, 0.2, 0.4, 0.6, 0.8, 0.9, 0.7, 0.5,
            0.3, 0.1, 0.2,
        ],
        duration_ms: 5000,
    };

    hud::show_hud(
        &app,
        envelope,
        Some("Test HUD - Sample text for preview".to_string()),
    );

    // Auto-hide after 3 seconds since HUD is click-through with no close button
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        hud::hide_hud(&app_clone);
    });

    Ok(())
}
