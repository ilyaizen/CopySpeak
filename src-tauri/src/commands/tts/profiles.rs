use crate::config::{self, sync_active_backend_mirror, AppConfig};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn set_active_profile(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    id: String,
) -> Result<(), String> {
    let mut cfg = config.lock().map_err(|e| e.to_string())?;
    if !cfg.tts.profiles.iter().any(|profile| profile.id == id) {
        return Err(format!("unknown profile: {}", id));
    }
    cfg.tts.active_profile_id = id;
    sync_active_backend_mirror(&mut cfg.tts);
    config::save(&cfg)?;
    let tts = cfg.tts.clone();
    drop(cfg);
    // The newly selected engine may be cold; prewarm it off-thread so the
    // first reading after a profile switch skips the model load.
    crate::commands::spawn_background_prewarm(tts);
    let _ = app.emit("config-changed", ());
    Ok(())
}
