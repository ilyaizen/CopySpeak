// Commands for running the KittenTTS installer.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

const INSTALLER_SCRIPT: &str = include_str!("../../../install-kittentts.ps1");
const CLI_SCRIPT: &str = include_str!("../../../kittentts-cli.py");

fn write_to_temp(content: &str, temp_dir: &PathBuf, filename: &str) -> io::Result<PathBuf> {
    let dest = temp_dir.join(filename);
    fs::write(&dest, content)?;
    Ok(dest)
}

#[tauri::command]
pub fn get_installer_script_path() -> String {
    let temp_dir = std::env::temp_dir().join("copyspeak-installer");
    temp_dir.join("install-kittentts.ps1").display().to_string()
}

#[tauri::command]
pub fn run_kittentts_installer() -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join("copyspeak-installer");
    fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp directory: {}", e))?;

    let script_path = write_to_temp(INSTALLER_SCRIPT, &temp_dir, "install-kittentts.ps1")
        .map_err(|e| format!("Failed to write installer script: {}", e))?;

    let _ = write_to_temp(CLI_SCRIPT, &temp_dir, "kittentts-cli.py");

    let script_path_str = script_path.display().to_string();

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("pwsh.exe");
        let script_wrapper = format!(
            r#"& '{}'; if ($LASTEXITCODE -eq 0) {{ Write-Host ""; Write-Host "Press any key to exit..." -ForegroundColor Cyan; $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") }} else {{ Write-Host ""; Write-Host "Installation failed with exit code: $LASTEXITCODE" -ForegroundColor Red; Write-Host ""; Write-Host "Press any key to exit..." -ForegroundColor Cyan; $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") }}"#,
            script_path_str
        );

        cmd.args([
            "-ExecutionPolicy",
            "Bypass",
            "-NoExit",
            "-WindowStyle",
            "Normal",
            "-Command",
            &script_wrapper,
        ]);

        match cmd.spawn() {
            Ok(_) => {
                log::info!("Opened KittenTTS installer in new PowerShell 7 window");
            }
            Err(_) => {
                log::warn!("pwsh.exe not found, falling back to powershell.exe");
                let mut cmd_fallback = Command::new("powershell.exe");
                cmd_fallback.args([
                    "-ExecutionPolicy",
                    "Bypass",
                    "-NoExit",
                    "-WindowStyle",
                    "Normal",
                    "-Command",
                    &script_wrapper,
                ]);

                cmd_fallback
                    .spawn()
                    .map_err(|e| format!("Failed to launch installer: {}", e))?;
                log::info!("Opened KittenTTS installer in new Windows PowerShell window");
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        return Err("Installer only available on Windows".into());
    }

    Ok(())
}
