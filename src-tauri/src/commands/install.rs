// Commands for running the KittenTTS installer.

use std::process::Command;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn get_installer_script_path(app_handle: AppHandle) -> Result<String, String> {
    let resource_path = app_handle
        .path()
        .resolve(
            "install-kittentts.ps1",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| format!("Failed to resolve installer script path: {}", e))?;
    Ok(resource_path.display().to_string())
}

#[tauri::command]
pub fn run_kittentts_installer(app_handle: AppHandle) -> Result<(), String> {
    let script_path = get_installer_script_path(app_handle)?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-ExecutionPolicy",
            "Bypass",
            "-NoExit",
            "-File",
            &script_path,
        ])
        .creation_flags(0x08000000);

        cmd.spawn()
            .map_err(|e| format!("Failed to launch installer: {}", e))?;
        log::info!("Opened KittenTTS installer in new PowerShell window");
    }

    #[cfg(not(target_os = "windows"))]
    {
        return Err("Installer only available on Windows".into());
    }

    Ok(())
}
