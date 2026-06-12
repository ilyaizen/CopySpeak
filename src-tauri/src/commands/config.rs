// Configuration management commands — get, set, reset, validate.
// Also includes general app state commands (listening, debug mode, clipboard).

use crate::audio::AudioPlayer;
use crate::config::{self, AppConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Reset configuration to factory defaults
/// Deletes the config file and returns default settings
#[tauri::command]
pub fn reset_config(
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
) -> Result<AppConfig, String> {
    log::info!("[IPC] reset_config called - resetting to factory defaults");

    // Create default config
    let default_config = AppConfig::default();

    // Delete config file from disk
    let path = config::config_path();
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            log::error!("Failed to delete config file: {}", e);
            return Err(format!("Failed to delete config file: {}", e));
        }
        log::info!("Deleted config file: {:?}", path);
    }

    // Update in-memory state
    let mut cfg = crate::lock_or_recover!(config);
    *cfg = default_config.clone();
    drop(cfg);

    // Apply runtime changes
    // Update audio settings
    {
        let cfg = crate::lock_or_recover!(config);
        let mut p = crate::lock_or_recover!(player);
        p.set_mode(cfg.playback.on_retrigger.clone());
        p.set_volume(cfg.playback.volume);
    }

    // Sync autostart setting
    if let Err(e) =
        crate::autostart::sync_autostart_with_config(default_config.general.start_with_windows)
    {
        log::error!("Failed to sync autostart after reset: {}", e);
    }

    // Update debug mode
    crate::logging::set_debug_mode(default_config.general.debug_mode);

    log::info!("Configuration reset to factory defaults");
    Ok(default_config)
}

#[tauri::command]
pub fn get_config(config: State<'_, Mutex<AppConfig>>) -> AppConfig {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_config called");
    }
    crate::lock_or_recover!(config).clone()
}

#[tauri::command]
pub fn set_config(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    is_listening: State<'_, Arc<AtomicBool>>,
    new_config: AppConfig,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_config called");
    }

    if let Err(errors) = new_config.validate() {
        let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(format!("Validation failed: {}", error_messages.join("; ")));
    }

    let (old_mode, old_volume, old_autostart, old_debug_mode, old_listen_enabled, old_hotkey, old_tts_config) = {
        let cfg = crate::lock_or_recover!(config);
        (
            cfg.playback.on_retrigger.clone(),
            cfg.playback.volume,
            cfg.general.start_with_windows,
            cfg.general.debug_mode,
            cfg.trigger.listen_enabled,
            cfg.hotkey.clone(),
            cfg.tts.clone(),
        )
    };
    let mode_changed = old_mode != new_config.playback.on_retrigger;
    let volume_changed = old_volume != new_config.playback.volume;
    let autostart_changed = old_autostart != new_config.general.start_with_windows;
    let debug_mode_changed = old_debug_mode != new_config.general.debug_mode;
    let listen_enabled_changed = old_listen_enabled != new_config.trigger.listen_enabled;
    let hotkey_changed = old_hotkey != new_config.hotkey;

    // R5(b): Restart is keyed only on command or cuda changes. Voice changes
    // don't need a restart — the Piper HTTP server lazy-loads voices per request.
    // Preset changes from piper→piper don't need a restart either.
    let piper_server_changed = old_tts_config.command != new_config.tts.command
        || old_tts_config.cuda != new_config.tts.cuda;

    // R5(a): Unload on switching away from Piper so the model doesn't linger in
    // RAM/VRAM. Toggling from piper to another engine releases the resources.
    let was_piper_active = old_tts_config.active_backend == crate::config::TtsEngine::Local
        && old_tts_config.preset == "piper";
    let is_piper_active = new_config.tts.active_backend == crate::config::TtsEngine::Local
        && new_config.tts.preset == "piper";
    let switched_away_from_piper = was_piper_active && !is_piper_active;

    if crate::logging::is_debug_mode() {
        log::debug!(
            "[IPC] Config changes - mode: {}, volume: {}, autostart: {}, debug_mode: {}, listen_enabled: {}, hotkey: {}",
            mode_changed,
            volume_changed,
            autostart_changed,
            debug_mode_changed,
            listen_enabled_changed,
            hotkey_changed
        );
    }

    let listen_enabled_value = new_config.trigger.listen_enabled;
    let tts_for_server = if piper_server_changed && is_piper_active {
        Some(new_config.tts.clone())
    } else {
        None
    };

    let new_config_clone = new_config.clone();

    let mut cfg = crate::lock_or_recover!(config);
    *cfg = new_config;

    config::save(&cfg)?;

    if debug_mode_changed {
        crate::logging::set_debug_mode(cfg.general.debug_mode);
    }

    drop(cfg);

    if mode_changed || volume_changed {
        let cfg = crate::lock_or_recover!(config);
        let mut p = crate::lock_or_recover!(player);
        if mode_changed {
            p.set_mode(cfg.playback.on_retrigger.clone());
        }
        if volume_changed {
            p.set_volume(cfg.playback.volume);
        }
    }

    if autostart_changed {
        let enabled = {
            let cfg = crate::lock_or_recover!(config);
            cfg.general.start_with_windows
        };
        if let Err(e) = crate::autostart::sync_autostart_with_config(enabled) {
            log::error!("Failed to sync autostart: {}", e);
        }
    }

    if listen_enabled_changed {
        is_listening.store(listen_enabled_value, Ordering::Relaxed);
        log::info!(
            "Listening state synced from config: {}",
            listen_enabled_value
        );
    }

    if hotkey_changed {
        let new_hotkey = {
            let cfg = crate::lock_or_recover!(config);
            log::info!(
                "[Config] Hotkey changed - enabled: {}, shortcut: {}",
                cfg.hotkey.enabled,
                cfg.hotkey.shortcut
            );
            cfg.hotkey.clone()
        };
        if let Err(e) = crate::register_hotkey(&app, &new_hotkey) {
            log::error!("[Config] Failed to re-register hotkey: {}", e);
        }
    }

    // Restart Piper server if TTS config changed
    if let Some(tts_cfg) = tts_for_server {
        let data_dir = crate::tts::cli::CliTtsBackend::data_dir();
        log::info!(
            "[Piper] Config change detected — restarting server (voice: {}, cuda: {})",
            tts_cfg.voice,
            tts_cfg.cuda
        );
        crate::tts::cli::restart_piper_server(
            tts_cfg.command,
            tts_cfg.voice,
            data_dir,
            tts_cfg.cuda,
        );
    }

    // R5(a): Unload Piper model when switching away (e.g., to OpenAI)
    // so hundreds of MB of RAM/VRAM aren't held for a backend that isn't active.
    if switched_away_from_piper {
        log::info!("[Piper] Switched away from Piper — unloading model");
        crate::tts::cli::unload_piper_model_internal();
    }

    // --- Local TTS server (Kokoro/Kitten/Pocket) lifecycle ---
    {
        let old_local = local_engine_from_preset(&old_tts_config.preset);
        let new_local = local_engine_from_preset(&new_config_clone.tts.preset);
        let is_local = new_config_clone.tts.active_backend == crate::config::TtsEngine::Local;

        // Unload if switching away from a local engine
        if let Some(ref old_engine) = old_local {
            let switched_away = !is_local || old_local != new_local;
            if switched_away {
                log::info!(
                    "[LocalServer] Switched away from {} — unloading model",
                    old_engine
                );
                crate::tts::cli::unload_local_server(old_engine);
            }
        }

        // Start new engine if active
        if is_local {
            if let Some(ref engine) = new_local {
                let command_changed = old_tts_config.command != new_config_clone.tts.command
                    || old_tts_config.args_template != new_config_clone.tts.args_template;
                let engine_switched = old_local != new_local;

                if engine_switched || command_changed {
                    log::info!(
                        "[LocalServer] Starting {} server (switched: {}, command_changed: {})",
                        engine,
                        engine_switched,
                        command_changed
                    );
                    let (script_args, cmd) = build_local_server_args(
                        engine,
                        &new_config_clone.tts.command,
                        &new_config_clone.tts.args_template,
                    );
                    if engine_switched {
                        crate::tts::cli::prewarm_local_server(engine.clone(), cmd, script_args);
                    } else {
                        crate::tts::cli::restart_local_server(engine.clone(), cmd, script_args);
                    }
                }
            }
        }
    }

    // Emit config-changed event so frontend can react
    let _ = app.emit("config-changed", ());

    Ok(())
}

/// Validate a config object without saving it.
/// Used for import validation before applying settings.
#[tauri::command]
pub fn validate_config(config: AppConfig) -> Result<(), String> {
    log::info!("[IPC] validate_config called");

    match config.validate() {
        Ok(()) => {
            log::info!("Config validation passed");
            Ok(())
        }
        Err(errors) => {
            let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            let error_msg = format!("Validation failed: {}", error_messages.join("; "));
            log::warn!("Config validation failed: {}", error_msg);
            Err(error_msg)
        }
    }
}

/// Returns the Piper voices directory path (e.g. C:\Users\<User>\piper-voices on Windows).
/// Used by the frontend to build CLI command previews with resolved paths.
#[tauri::command]
pub fn get_data_dir() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("piper-voices")
        .to_string_lossy()
        .into_owned()
}

/// Returns the user's home directory path.
/// Used by the frontend to resolve {home_dir} placeholder in CLI templates.
#[tauri::command]
pub fn get_home_dir() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .to_string_lossy()
        .into_owned()
}

/// Check if the config file exists on disk.
/// Used for first-run detection to show onboarding.
#[tauri::command]
pub fn config_exists() -> bool {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] config_exists called");
    }
    config::config_path().exists()
}

// ── General app state commands ──────────────────────────────────────────────

#[tauri::command]
pub fn set_listening(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    is_listening: State<'_, Arc<AtomicBool>>,
    enabled: bool,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_listening called (enabled: {})", enabled);
    }
    is_listening.store(enabled, Ordering::Relaxed);
    log::info!("set_listening: {}", enabled);

    // Update in-memory config and persist to disk
    {
        let mut cfg = config.lock().map_err(|e| e.to_string())?;
        cfg.trigger.listen_enabled = enabled;
        config::save(&cfg)?;
    }

    // Emit config-changed event to sync the frontend and tray icon
    let _ = app.emit("config-changed", ());

    Ok(())
}

#[tauri::command]
pub fn get_listening(is_listening: State<'_, Arc<AtomicBool>>) -> bool {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_listening called");
    }
    is_listening.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn get_clipboard_content() -> Option<String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_clipboard_content called");
    }
    crate::clipboard::get_clipboard_text()
}

#[tauri::command]
pub fn set_debug_mode(enabled: bool) -> Result<(), String> {
    log::info!("[IPC] set_debug_mode called (enabled: {})", enabled);
    crate::logging::set_debug_mode(enabled);
    log::info!("Debug mode set to: {}", enabled);
    Ok(())
}

// ── Local TTS server helpers ────────────────────────────────────────────────

fn local_engine_from_preset(preset: &str) -> Option<String> {
    match preset {
        "kokoro-tts" => Some("kokoro".into()),
        "kitten-tts" => Some("kitten".into()),
        "pocket-tts" => Some("pocket".into()),
        _ => None,
    }
}

fn build_local_server_args(
    engine: &str,
    command: &str,
    args_template: &[String],
) -> (Vec<String>, String) {
    let backend = crate::tts::cli::CliTtsBackend::new(
        command.into(),
        args_template.to_vec(),
        false,
        match engine {
            "kokoro" => "kokoro-tts",
            "kitten" => "kitten-tts",
            "pocket" => "pocket-tts",
            _ => engine,
        }
        .into(),
    );

    let args = match engine {
        "kokoro" => backend.kokoro_model_args(),
        "kitten" => backend.kitten_model_args(),
        _ => Vec::new(),
    };

    (args, command.into())
}
