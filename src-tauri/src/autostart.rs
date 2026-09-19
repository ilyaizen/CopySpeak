// Launch-on-login persistence.
//
// Windows: HKCU Run registry value. Linux (systemd-based sessions, which
// includes uwsm-wrapped Hyprland): an XDG autostart `.desktop` entry in
// `$XDG_CONFIG_HOME/autostart`, started by `xdg-desktop-autostart.target`.

use std::path::PathBuf;

#[cfg(not(target_os = "windows"))]
const DESKTOP_ENTRY: &str = "\
[Desktop Entry]
Type=Application
Name=CopySpeak
Exec=copyspeak
Terminal=false
X-GNOME-Autostart-enabled=true
";

fn get_current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))
}

#[cfg(target_os = "windows")]
mod windows_imp {
    use super::*;

    use winreg::enums::*;
    use winreg::RegKey;

    const APP_NAME: &str = "CopySpeak";
    const REGISTRY_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn get_run_key() -> Result<RegKey, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(REGISTRY_KEY, KEY_SET_VALUE | KEY_READ)
            .map_err(|e| format!("Failed to open registry key: {}", e))
    }

    pub fn enable_autostart() -> Result<(), String> {
        let exe_path = get_current_exe_path()?;
        let exe_path_str = exe_path.to_string_lossy();

        let quoted_path = format!("\"{}\"", exe_path_str);

        let run_key = get_run_key()?;
        run_key
            .set_value(APP_NAME, &quoted_path)
            .map_err(|e| format!("Failed to set registry value: {}", e))?;

        log::info!("Enabled auto-start: {}", quoted_path);
        Ok(())
    }

    pub fn disable_autostart() -> Result<(), String> {
        let run_key = get_run_key()?;

        match run_key.delete_value(APP_NAME) {
            Ok(()) => {
                log::info!("Disabled auto-start");
                Ok(())
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    log::info!("Auto-start was already disabled (registry value not found)");
                    Ok(())
                } else {
                    Err(format!("Failed to remove registry value: {}", e))
                }
            }
        }
    }

    pub fn is_autostart_enabled() -> Result<bool, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(REGISTRY_KEY, KEY_READ)
            .map_err(|e| format!("Failed to open registry key for reading: {}", e))?;

        match run_key.get_value::<String, _>(APP_NAME) {
            Ok(stored_path) => {
                let current_exe = get_current_exe_path()?;
                let current_exe_str = current_exe.to_string_lossy();

                let is_match = stored_path.trim_matches('"') == current_exe_str;
                log::debug!(
                    "Auto-start check: stored='{}', current='{}', match={}",
                    stored_path,
                    current_exe_str,
                    is_match
                );
                Ok(is_match)
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(format!("Failed to read registry value: {}", e))
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod linux_imp {
    use super::*;

    const APP_NAME: &str = "CopySpeak";

    /// `$XDG_CONFIG_HOME/autostart` (default `~/.config/autostart`).
    fn autostart_dir() -> Result<PathBuf, String> {
        let config = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => {
                let home =
                    std::env::var_os("HOME").ok_or("Neither XDG_CONFIG_HOME nor HOME is set")?;
                PathBuf::from(home).join(".config")
            }
        };
        Ok(config.join("autostart"))
    }

    fn entry_path() -> Result<PathBuf, String> {
        Ok(autostart_dir()?.join(format!("{APP_NAME}.desktop")))
    }

    pub fn enable_autostart() -> Result<(), String> {
        let path = entry_path()?;
        let dir = path.parent().expect("entry path has a parent");
        std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create autostart dir: {e}"))?;
        std::fs::write(&path, DESKTOP_ENTRY)
            .map_err(|e| format!("Failed to write autostart entry: {e}"))?;
        log::info!("Enabled auto-start: {}", path.display());
        Ok(())
    }

    pub fn disable_autostart() -> Result<(), String> {
        let path = entry_path()?;
        match std::fs::remove_file(&path) {
            Ok(()) => {
                log::info!("Disabled auto-start");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::info!("Auto-start was already disabled (entry not found)");
                Ok(())
            }
            Err(e) => Err(format!("Failed to remove autostart entry: {e}")),
        }
    }

    pub fn is_autostart_enabled() -> Result<bool, String> {
        Ok(entry_path()?.exists())
    }
}

#[cfg(not(target_os = "windows"))]
use linux_imp::is_autostart_enabled as is_autostart_enabled_impl;
#[cfg(not(target_os = "windows"))]
use linux_imp::{disable_autostart, enable_autostart};
#[cfg(target_os = "windows")]
use windows_imp::is_autostart_enabled as is_autostart_enabled_impl;
#[cfg(target_os = "windows")]
use windows_imp::{disable_autostart, enable_autostart};

#[allow(dead_code)]
pub fn is_autostart_enabled() -> Result<bool, String> {
    is_autostart_enabled_impl()
}

pub fn sync_autostart_with_config(enabled: bool) -> Result<(), String> {
    if enabled {
        enable_autostart()
    } else {
        disable_autostart()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_exe_path() {
        let result = get_current_exe_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.to_string_lossy().ends_with(".exe")
                || path.to_string_lossy().contains("copyspeak")
        );
    }

    #[test]
    fn test_is_autostart_enabled_does_not_crash() {
        let result = is_autostart_enabled();
        assert!(result.is_ok());
    }
}
