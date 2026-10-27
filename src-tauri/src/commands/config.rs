// Configuration management commands — get, set, reset, validate.
// Also includes general app state commands (listening, debug mode, clipboard).

use crate::audio::AudioPlayer;
use crate::config::{self, AppConfig};
use crate::secrets;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Reset configuration to factory defaults
/// Deletes the config file and returns default settings
#[tauri::command]
pub fn reset_config(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    is_listening: State<'_, Arc<AtomicBool>>,
) -> Result<AppConfig, String> {
    log::info!("[IPC] reset_config called - resetting to factory defaults");

    // Create default config
    let default_config = AppConfig::default();

    let path = config::config_path();
    let mut cfg = config.lock().unwrap();
    let was_duck_effective = cfg.is_duck_effective();
    let old_show_tray_icon = cfg.general.show_tray_icon;
    if !old_show_tray_icon {
        crate::tray::sync_visibility(&app, true)?;
    }
    let persisted = crate::autostart::persist_with_autostart(
        cfg.general.launch_on_login,
        default_config.general.launch_on_login,
        || match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Failed to delete config file: {error}")),
        },
    );
    if let Err(error) = persisted {
        if !old_show_tray_icon {
            if let Err(rollback) = crate::tray::sync_visibility(&app, false) {
                return Err(format!(
                    "{error}; failed to restore tray visibility: {rollback}"
                ));
            }
        }
        return Err(error);
    }
    *cfg = default_config.clone();
    drop(cfg);
    is_listening.store(default_config.trigger.listen_enabled, Ordering::Relaxed);
    crate::update_tray_icon(&app);
    if let Err(error) = crate::tray_labels::apply(&app) {
        log::warn!("[Tray] Could not reset labels: {error}");
    }

    // Factory defaults leave Duck off: un-duck a reading in progress.
    if was_duck_effective {
        crate::audio_duck::restore();
    }

    // Apply runtime changes
    // Update audio settings
    {
        let cfg = config.lock().unwrap();
        let mut p = player.lock().unwrap();
        p.set_mode(cfg.playback.on_retrigger.clone());
        p.set_volume(cfg.playback.volume);
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
    let mut cfg = config.lock().unwrap().clone();
    // API keys are deliberately NOT hydrated from .env here: env-provided keys
    // win at synthesis time (secrets::resolve is env-first), but the raw
    // secret never leaves the backend. The frontend renders a localized
    // "from .env" sentinel from get_env_key_sources instead, so a typed key
    // that env precedence would shadow is a visible, warned choice — not a
    // silent no-op. Env vars are loaded by secrets::load_dotenv() at startup
    // and never written back to config.json.
    // The Microsoft endpoint is not a secret; hydrate it so the field shows
    // the effective value.
    cfg.tts.microsoft.endpoint =
        secrets::resolve(&cfg.tts.microsoft.endpoint, &["MICROSOFT_ENDPOINT"]);
    cfg
}

/// Canonical engine → env-var lists for cloud API keys. ONE table shared by
/// `set_config`'s strip pass and `get_env_key_sources`. Order within a list is
/// precedence: first non-empty wins, matching `secrets::resolve`. The Microsoft
/// endpoint is hydrated/stripped separately (not a secret, not in this table).
const ENGINE_API_KEY_ENV_VARS: &[(&str, &[&str])] = &[
    ("openai", &["OPENAI_API_KEY"]),
    ("elevenlabs", &["ELEVENLABS_API_KEY"]),
    ("cartesia", &["CARTESIA_API_KEY"]),
    ("fish", &["FISH_API_KEY"]),
    ("xai", &["XAI_API_KEY"]),
    ("mistral", &["MISTRAL_API_KEY"]),
    ("google", &["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
    ("microsoft", &["MICROSOFT_API_KEY", "AZURE_API_KEY"]),
];

/// Mutable accessor for a cloud engine's persisted `api_key`, keyed by the
/// engine names used in `ENGINE_API_KEY_ENV_VARS`. `None` for unknown engines.
fn tts_api_key_mut<'a>(config: &'a mut AppConfig, engine: &str) -> Option<&'a mut String> {
    match engine {
        "openai" => Some(&mut config.tts.openai.api_key),
        "elevenlabs" => Some(&mut config.tts.elevenlabs.api_key),
        "cartesia" => Some(&mut config.tts.cartesia.api_key),
        "fish" => Some(&mut config.tts.fish.api_key),
        "xai" => Some(&mut config.tts.xai.api_key),
        "mistral" => Some(&mut config.tts.mistral.api_key),
        "google" => Some(&mut config.tts.google.api_key),
        "microsoft" => Some(&mut config.tts.microsoft.api_key),
        _ => None,
    }
}

/// First non-empty env var among `env_names` — the same precedence
/// `secrets::resolve` applies when choosing the active key. `None` when none
/// of the listed vars is set to a non-empty value.
fn winning_env_var(env_names: &[&str]) -> Option<String> {
    env_names
        .iter()
        .find(|name| {
            std::env::var(name)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
        })
        .map(|name| name.to_string())
}

/// Engines whose API key is currently provided by the environment, mapped to
/// the env var that wins. The frontend renders a localized "from .env"
/// sentinel from this map instead of ever receiving the raw secret, and
/// confirms before saving a typed key that env precedence would keep shadowing.
#[tauri::command]
pub fn get_env_key_sources() -> HashMap<String, String> {
    ENGINE_API_KEY_ENV_VARS
        .iter()
        .filter_map(|(engine, env_names)| {
            winning_env_var(env_names).map(|var| (engine.to_string(), var))
        })
        .collect()
}

#[tauri::command]
pub async fn list_post_process_models(
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let endpoint = format!(
        "{}/models",
        base_url
            .trim()
            .trim_end_matches("/chat/completions")
            .trim_end_matches("/responses")
            .trim_end_matches('/')
    );

    let client = reqwest::Client::new();
    let mut builder = client.get(&endpoint);
    let api_key = crate::secrets::resolve(&api_key, &["POST_PROCESS_API_KEY"]);
    if !api_key.trim().is_empty() {
        builder = builder.header(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
                .map_err(|e| format!("Invalid API key header: {e}"))?,
        );
    }

    let value: Value = builder
        .send()
        .await
        .map_err(|e| format!("Model refresh failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Model refresh API error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Model refresh response parse failed: {e}"))?;

    let mut models = value["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    models.sort();
    models.dedup();
    Ok(models)
}

/// The last global-hotkey registration failure, if the current hotkey did not
/// register (taken by another app). `None` when it registered or is disabled.
#[tauri::command]
pub fn get_hotkey_error() -> Option<String> {
    crate::hotkey_error()
}

#[tauri::command]
pub fn set_config(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    is_listening: State<'_, Arc<AtomicBool>>,
    mut new_config: AppConfig,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_config called");
    }

    if let Err(errors) = new_config.validate() {
        let error_messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(format!("Validation failed: {}", error_messages.join("; ")));
    }

    // `version` is owned by the app, not the caller (frontend or imported
    // file) — always persist the running version stamp.
    new_config.version = crate::config::config_version().into();

    // Strip API keys that match env vars so they aren't persisted to disk.
    // Env values are loaded via secrets::load_dotenv() and injected at read
    // time; saving them would leak secrets and override the env-first contract.
    fn strip_env(ui_val: &str, env_names: &[&str]) -> String {
        let env_val = secrets::resolve(&String::new(), env_names);
        if !env_val.is_empty() && ui_val.trim() == env_val {
            String::new()
        } else {
            ui_val.to_string()
        }
    }
    for (engine, env_names) in ENGINE_API_KEY_ENV_VARS {
        if let Some(key) = tts_api_key_mut(&mut new_config, engine) {
            let stripped = strip_env(key, env_names);
            *key = stripped;
        }
    }
    new_config.tts.microsoft.endpoint =
        strip_env(&new_config.tts.microsoft.endpoint, &["MICROSOFT_ENDPOINT"]);

    let (old_mode, old_volume, old_debug_mode, old_listen_enabled, old_hotkey) = {
        let cfg = config.lock().unwrap();
        (
            cfg.playback.on_retrigger.clone(),
            cfg.playback.volume,
            cfg.general.debug_mode,
            cfg.trigger.listen_enabled,
            cfg.hotkey.clone(),
        )
    };
    let old_duck_effective = {
        let cfg = config.lock().unwrap();
        cfg.is_duck_effective()
    };
    let old_daemon_idle_timeout = {
        let cfg = config.lock().unwrap();
        cfg.tts.daemon_idle_timeout_secs
    };
    let new_duck_effective = new_config.is_duck_effective();
    let mode_changed = old_mode != new_config.playback.on_retrigger;
    let volume_changed = old_volume != new_config.playback.volume;

    let debug_mode_changed = old_debug_mode != new_config.general.debug_mode;
    let listen_enabled_changed = old_listen_enabled != new_config.trigger.listen_enabled;
    let hotkey_changed = old_hotkey != new_config.hotkey;
    let daemon_idle_timeout_changed =
        old_daemon_idle_timeout != new_config.tts.daemon_idle_timeout_secs;

    if crate::logging::is_debug_mode() {
        log::debug!(
            "[IPC] Config changes - mode: {}, volume: {}, debug_mode: {}, listen_enabled: {}, hotkey: {}",
            mode_changed,
            volume_changed,

            debug_mode_changed,
            listen_enabled_changed,
            hotkey_changed
        );
    }

    let listen_enabled_value = new_config.trigger.listen_enabled;

    crate::config::sync_active_backend_mirror(&mut new_config.tts);

    let mut cfg = config.lock().unwrap();
    let old_show_tray_icon = cfg.general.show_tray_icon;
    let tray_visibility_changed = old_show_tray_icon != new_config.general.show_tray_icon;
    if tray_visibility_changed {
        crate::tray::sync_visibility(&app, new_config.general.show_tray_icon)?;
    }
    let persisted = crate::autostart::persist_with_autostart(
        cfg.general.launch_on_login,
        new_config.general.launch_on_login,
        || config::save(&new_config),
    );
    if let Err(error) = persisted {
        if tray_visibility_changed {
            if let Err(rollback) = crate::tray::sync_visibility(&app, old_show_tray_icon) {
                return Err(format!(
                    "{error}; failed to restore tray visibility: {rollback}"
                ));
            }
        }
        return Err(error);
    }
    *cfg = new_config;

    if debug_mode_changed {
        crate::logging::set_debug_mode(cfg.general.debug_mode);
    }

    drop(cfg);

    if mode_changed || volume_changed {
        let cfg = config.lock().unwrap();
        let mut p = player.lock().unwrap();
        if mode_changed {
            p.set_mode(cfg.playback.on_retrigger.clone());
        }
        if volume_changed {
            p.set_volume(cfg.playback.volume);
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
            let cfg = config.lock().unwrap();
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

    if daemon_idle_timeout_changed {
        let secs = {
            let cfg = config.lock().unwrap();
            cfg.tts.daemon_idle_timeout_secs
        };
        crate::tts::local_daemon::set_idle_timeout(secs);
        log::info!("[LocalDaemon] idle timeout set to {}s", secs);
    }

    // A save may have changed the active engine, its model, voice or device;
    // prewarm off-thread so the next reading skips the cold load. The daemon
    // keyed on the resolved command no-ops while it is already warm.
    {
        let tts = { config.lock().unwrap().tts.clone() };
        crate::commands::spawn_background_prewarm(tts);
    }

    // Duck is gated by Experimental: a save that turns either switch off while
    // a reading plays must un-duck immediately through the same worker
    // (restore is a no-op when nothing is ducked; playback-finished and crash
    // recovery still restore as before).
    if old_duck_effective && !new_duck_effective {
        log::info!("[Config] Duck became ineffective — restoring ducked apps");
        crate::audio_duck::restore();
    }

    // Tray labels follow the saved locale and the listening state.
    if let Err(e) = crate::tray_labels::apply(&app) {
        log::error!("Failed to update tray labels: {}", e);
    }
    crate::update_tray_icon(&app);

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
    is_listening: State<'_, Arc<AtomicBool>>,
    enabled: bool,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] set_listening called (enabled: {})", enabled);
    }
    is_listening.store(enabled, Ordering::Relaxed);
    crate::update_tray_icon(&app);
    if let Err(error) = crate::tray_labels::apply(&app) {
        log::warn!("[Tray] Could not update listening label: {error}");
    }
    log::info!("set_listening: {}", enabled);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Process env is global; serialize the env-mutating tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn winning_env_var_prefers_first_non_empty() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("COPYTEST_A", "   ");
        std::env::set_var("COPYTEST_B", "token");
        assert_eq!(
            winning_env_var(&["COPYTEST_A", "COPYTEST_B"]),
            Some("COPYTEST_B".to_string())
        );
        assert_eq!(
            winning_env_var(&["COPYTEST_B", "COPYTEST_A"]),
            Some("COPYTEST_B".to_string())
        );
        std::env::remove_var("COPYTEST_A");
        std::env::remove_var("COPYTEST_B");
    }

    #[test]
    fn winning_env_var_is_none_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("COPYTEST_UNSET");
        assert_eq!(winning_env_var(&["COPYTEST_UNSET"]), None);
    }

    #[test]
    fn engine_env_table_covers_every_cloud_engine() {
        let engines: Vec<&str> = ENGINE_API_KEY_ENV_VARS
            .iter()
            .map(|(engine, _)| *engine)
            .collect();
        assert_eq!(
            engines,
            [
                "openai",
                "elevenlabs",
                "cartesia",
                "fish",
                "xai",
                "mistral",
                "google",
                "microsoft"
            ]
        );
        for (_, env_names) in ENGINE_API_KEY_ENV_VARS {
            assert!(!env_names.is_empty());
        }
    }

    #[test]
    fn tts_api_key_mut_targets_every_listed_engine() {
        let mut config = AppConfig::default();
        for (engine, _) in ENGINE_API_KEY_ENV_VARS {
            assert!(tts_api_key_mut(&mut config, engine).is_some());
        }
        assert!(tts_api_key_mut(&mut config, "not-an-engine").is_none());
    }

    #[test]
    fn get_env_key_sources_maps_set_env_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("FISH_API_KEY", "fish-token");
        let sources = get_env_key_sources();
        assert_eq!(
            sources.get("fish").map(String::as_str),
            Some("FISH_API_KEY")
        );
        std::env::remove_var("FISH_API_KEY");
    }
}
