// Auto-updater commands: trigger update checks from frontend.

use tauri::{AppHandle, Emitter};

/// Trigger an update check by emitting an event that the frontend listens for.
/// Respects the update_checks_enabled config setting.
#[tauri::command]
pub fn trigger_update_check(app: AppHandle) -> Result<(), String> {
    let config = crate::config::load_or_default();
    if !config.general.update_checks_enabled {
        log::info!("Update checks disabled in config, skipping");
        return Ok(());
    }

    log::info!("Triggering update check");
    app.emit("check-for-updates", ())
        .map_err(|e| format!("Failed to emit update check event: {}", e))?;

    Ok(())
}
