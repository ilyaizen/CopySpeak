// Launch-on-login persistence.
//
// Windows: HKCU Run registry value. Linux (systemd-based sessions, which
// includes uwsm-wrapped Hyprland): an XDG autostart `.desktop` entry in
// `$XDG_CONFIG_HOME/autostart`, started by `xdg-desktop-autostart.target`.

use std::path::PathBuf;

fn get_current_exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))
}

#[cfg(target_os = "windows")]
mod windows_imp {
    use super::*;

    use std::path::Path;
    use winreg::enums::*;
    use winreg::RegKey;

    const APP_NAME: &str = "CopySpeech";
    /// Run value written by CopySpeak 0.2.x. Removed when CopySpeech takes over,
    /// so one login does not start both apps.
    const LEGACY_APP_NAME: &str = "CopySpeak";
    const REGISTRY_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    /// Installed folder and executable of this build, under %LOCALAPPDATA%.
    const INSTALL_DIR: &str = "CopySpeech";
    const INSTALL_EXE: &str = "copyspeech.exe";
    /// Installed folder and executable of CopySpeak 0.2.x, under %LOCALAPPDATA%.
    const LEGACY_INSTALL_DIR: &str = "CopySpeak";
    const LEGACY_INSTALL_EXE: &str = "copyspeak.exe";

    fn get_run_key() -> Result<RegKey, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(REGISTRY_KEY, KEY_SET_VALUE | KEY_READ)
            .map_err(|e| format!("Failed to open registry key: {}", e))
    }

    /// Delete one Run value. A missing value counts as success.
    fn remove_run_value(run_key: &RegKey, name: &str) -> Result<(), String> {
        match run_key.delete_value(name) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Failed to remove registry value {name}: {e}")),
        }
    }

    pub fn enable_autostart() -> Result<(), String> {
        let exe_path = get_current_exe_path()?;
        let exe_path_str = exe_path.to_string_lossy();

        let quoted_path = format!("\"{}\"", exe_path_str);

        let run_key = get_run_key()?;
        run_key
            .set_value(APP_NAME, &quoted_path)
            .map_err(|e| format!("Failed to set registry value: {}", e))?;
        remove_run_value(&run_key, LEGACY_APP_NAME)?;

        log::info!("Enabled auto-start: {}", quoted_path);
        Ok(())
    }

    pub fn disable_autostart() -> Result<(), String> {
        let run_key = get_run_key()?;
        remove_run_value(&run_key, APP_NAME)?;
        remove_run_value(&run_key, LEGACY_APP_NAME)?;
        log::info!("Disabled auto-start");
        Ok(())
    }

    /// Read one Run value. A missing value is `None`.
    fn read_run_value(run_key: &RegKey, name: &str) -> Result<Option<String>, String> {
        match run_key.get_value::<String, _>(name) {
            Ok(value) => Ok(Some(value)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("Failed to read registry value {name}: {e}")),
        }
    }

    fn install_exe(local_app_data: &Path) -> PathBuf {
        local_app_data.join(INSTALL_DIR).join(INSTALL_EXE)
    }

    fn legacy_install_exe(local_app_data: &Path) -> PathBuf {
        local_app_data
            .join(LEGACY_INSTALL_DIR)
            .join(LEGACY_INSTALL_EXE)
    }

    /// Windows paths compare case-insensitively, and `/` equals `\`.
    fn same_windows_path(text: &str, expected: &Path) -> bool {
        let normalize = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
        normalize(text) == normalize(&expected.to_string_lossy())
    }

    /// Pure migration gate. Migrates only when this build runs from its
    /// installed folder, the 0.2.x Run value launches the 0.2.x installed exe,
    /// and no CopySpeech Run value exists yet. Dev, portable, and other copies
    /// never claim a registration.
    fn migration_allowed(
        local_app_data: &Path,
        current_exe: &Path,
        new_value_present: bool,
        legacy_run_value: Option<&str>,
    ) -> bool {
        !new_value_present
            && same_windows_path(&current_exe.to_string_lossy(), &install_exe(local_app_data))
            && legacy_run_value.is_some_and(|value| {
                same_windows_path(value.trim_matches('"'), &legacy_install_exe(local_app_data))
            })
    }

    /// Move a 0.2.x Run registration to CopySpeech when `migration_allowed`.
    /// Returns whether a migration ran.
    pub fn migrate_legacy_registration() -> Result<bool, String> {
        let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            return Ok(false);
        };
        let current_exe = get_current_exe_path()?;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(REGISTRY_KEY, KEY_READ)
            .map_err(|e| format!("Failed to open registry key for reading: {}", e))?;
        let new_value_present = read_run_value(&run_key, APP_NAME)?.is_some();
        let legacy_value = read_run_value(&run_key, LEGACY_APP_NAME)?;

        if !migration_allowed(
            &local_app_data,
            &current_exe,
            new_value_present,
            legacy_value.as_deref(),
        ) {
            return Ok(false);
        }
        enable_autostart()?;
        log::info!("Migrated auto-start from CopySpeak to CopySpeech");
        Ok(true)
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

    #[cfg(test)]
    mod tests {
        // Pure gate only. No test reads or writes the real Run key.
        use super::*;

        const LOCAL: &str = r"C:\Users\User\AppData\Local";
        const INSTALLED_EXE: &str = r"C:\Users\User\AppData\Local\CopySpeech\copyspeech.exe";
        const LEGACY_RUN: &str = "\"C:\\Users\\User\\AppData\\Local\\CopySpeak\\copyspeak.exe\"";

        fn allowed(current: &str, new_present: bool, legacy: Option<&str>) -> bool {
            migration_allowed(Path::new(LOCAL), Path::new(current), new_present, legacy)
        }

        #[test]
        fn actual_0_2_x_to_0_3_upgrade_migrates() {
            assert!(allowed(INSTALLED_EXE, false, Some(LEGACY_RUN)));
        }

        #[test]
        fn path_case_and_separator_differences_still_match() {
            assert!(allowed(
                "c:/users/user/appdata/local/copyspeech/COPYSPEECH.EXE",
                false,
                Some("\"c:\\users\\user\\appdata\\local\\copyspeak\\COPYSPEAK.EXE\""),
            ));
        }

        #[test]
        fn existing_new_registration_is_not_overwritten() {
            assert!(!allowed(INSTALLED_EXE, true, Some(LEGACY_RUN)));
        }

        #[test]
        fn debug_build_is_not_migrated() {
            let dev = r"D:\Projects\CopySpeak\src-tauri\target\debug\copyspeech.exe";
            assert!(!allowed(dev, false, Some(LEGACY_RUN)));
            assert!(!allowed(
                dev,
                false,
                Some(r#""D:\Projects\CopySpeak\src-tauri\target\debug\copyspeak.exe""#),
            ));
        }

        #[test]
        fn portable_copy_is_not_migrated() {
            let portable = r"D:\Portable\CopySpeech\copyspeech.exe";
            assert!(!allowed(portable, false, Some(LEGACY_RUN)));
        }

        #[test]
        fn legacy_run_pointing_elsewhere_is_not_migrated() {
            assert!(!allowed(
                INSTALLED_EXE,
                false,
                Some(r#""D:\Portable\CopySpeak\copyspeak.exe""#),
            ));
        }

        #[test]
        fn missing_legacy_run_is_not_migrated() {
            assert!(!allowed(INSTALLED_EXE, false, None));
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_imp {
    use super::*;
    use std::path::Path;

    const APP_NAME: &str = "CopySpeech";
    /// Entry name written by CopySpeak 0.2.x. Removed when CopySpeech takes over.
    const LEGACY_APP_NAME: &str = "CopySpeak";

    const DESKTOP_ENTRY: &str = "\
[Desktop Entry]
Type=Application
Name=CopySpeech
Exec=copyspeech
Terminal=false
X-GNOME-Autostart-enabled=true
";

    /// Lines of the 0.2.x entry, verbatim. A file must match all of them to be
    /// treated as this app's legacy entry.
    const LEGACY_ENTRY_LINES: &[&str] = &[
        "[Desktop Entry]",
        "Type=Application",
        "Name=CopySpeak",
        "Exec=copyspeak",
        "Terminal=false",
        "X-GNOME-Autostart-enabled=true",
    ];

    /// Installed locations that prove a packaged build. The legacy entry runs
    /// `copyspeak` from PATH, so only these PATH-visible binaries count.
    const SYSTEM_BINS: &[&str] = &["/usr/bin/copyspeech", "/usr/local/bin/copyspeech"];

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

    fn entry_path(name: &str) -> Result<PathBuf, String> {
        Ok(autostart_dir()?.join(format!("{name}.desktop")))
    }

    /// Delete one desktop entry. A missing file counts as success.
    fn remove_entry(path: &PathBuf) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Failed to remove autostart entry: {e}")),
        }
    }

    /// Write the CopySpeech entry into `dir`, then drop the 0.2.x entry there.
    fn enable_in(dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create autostart dir: {e}"))?;
        let path = dir.join(format!("{APP_NAME}.desktop"));
        std::fs::write(&path, DESKTOP_ENTRY)
            .map_err(|e| format!("Failed to write autostart entry: {e}"))?;
        remove_entry(&dir.join(format!("{LEGACY_APP_NAME}.desktop")))?;
        log::info!("Enabled auto-start: {}", path.display());
        Ok(())
    }

    pub fn enable_autostart() -> Result<(), String> {
        enable_in(&autostart_dir()?)
    }

    /// True when `content` is the 0.2.x entry, ignoring blank lines and
    /// surrounding whitespace.
    fn is_legacy_entry(content: &str) -> bool {
        let lines: Vec<&str> = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        lines == LEGACY_ENTRY_LINES
    }

    /// Migrate `dir`'s 0.2.x entry to CopySpeech. Runs only when the running
    /// binary is one of `system_bins`, the legacy entry matches exactly, and no
    /// CopySpeech entry exists yet. Returns whether a migration ran.
    fn migrate_in(dir: &Path, current_exe: &Path, system_bins: &[PathBuf]) -> Result<bool, String> {
        let legacy = dir.join(format!("{LEGACY_APP_NAME}.desktop"));
        let new = dir.join(format!("{APP_NAME}.desktop"));

        if new.symlink_metadata().is_ok() || !legacy.exists() {
            return Ok(false);
        }
        if !system_bins.iter().any(|bin| bin == current_exe) {
            return Ok(false);
        }
        let content = std::fs::read_to_string(&legacy)
            .map_err(|e| format!("Failed to read legacy autostart entry: {e}"))?;
        if !is_legacy_entry(&content) {
            return Ok(false);
        }
        enable_in(dir)?;
        Ok(true)
    }

    /// Migrate the real autostart dir. Canonicalizes both sides, so a symlinked
    /// system binary still matches the resolved current exe.
    pub fn migrate_legacy_registration() -> Result<bool, String> {
        let current_exe = std::fs::canonicalize(get_current_exe_path()?)
            .map_err(|e| format!("Failed to resolve executable path: {e}"))?;
        let system_bins: Vec<PathBuf> = SYSTEM_BINS
            .iter()
            .filter_map(|bin| std::fs::canonicalize(bin).ok())
            .collect();
        migrate_in(&autostart_dir()?, &current_exe, &system_bins)
    }

    pub fn disable_autostart() -> Result<(), String> {
        remove_entry(&entry_path(APP_NAME)?)?;
        remove_entry(&entry_path(LEGACY_APP_NAME)?)?;
        log::info!("Disabled auto-start");
        Ok(())
    }

    pub fn is_autostart_enabled() -> Result<bool, String> {
        Ok(entry_path(APP_NAME)?.exists())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Scratch autostart dir under the OS temp dir; removed on drop.
        struct Scratch(PathBuf);

        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        const LEGACY_ENTRY: &str = "[Desktop Entry]
Type=Application
Name=CopySpeak
Exec=copyspeak
Terminal=false
X-GNOME-Autostart-enabled=true
";

        fn scratch(name: &str) -> (PathBuf, Scratch) {
            let dir = std::env::temp_dir().join(format!(
                "copyspeech-autostart-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            (dir.clone(), Scratch(dir))
        }

        fn bins() -> Vec<PathBuf> {
            vec![PathBuf::from("/usr/bin/copyspeech")]
        }

        #[test]
        fn upgrade_from_installed_build_renames_entry_and_keeps_unrelated_files() {
            let (dir, _guard) = scratch("upgrade");
            std::fs::write(dir.join("CopySpeak.desktop"), LEGACY_ENTRY).unwrap();
            std::fs::write(dir.join("Other.desktop"), "unrelated").unwrap();

            let migrated = migrate_in(&dir, Path::new("/usr/bin/copyspeech"), &bins()).unwrap();

            assert!(migrated);
            assert!(!dir.join("CopySpeak.desktop").exists());
            assert_eq!(
                std::fs::read_to_string(dir.join("CopySpeech.desktop")).unwrap(),
                DESKTOP_ENTRY
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("Other.desktop")).unwrap(),
                "unrelated"
            );
        }

        #[test]
        fn existing_new_entry_is_not_overwritten() {
            let (dir, _guard) = scratch("existing-new");
            std::fs::write(dir.join("CopySpeak.desktop"), LEGACY_ENTRY).unwrap();
            std::fs::write(dir.join("CopySpeech.desktop"), "user-custom").unwrap();

            let migrated = migrate_in(&dir, Path::new("/usr/bin/copyspeech"), &bins()).unwrap();

            assert!(!migrated);
            assert_eq!(
                std::fs::read_to_string(dir.join("CopySpeech.desktop")).unwrap(),
                "user-custom"
            );
            assert!(dir.join("CopySpeak.desktop").exists());
        }

        #[test]
        fn appimage_or_portable_run_is_not_migrated() {
            let (dir, _guard) = scratch("appimage");
            std::fs::write(dir.join("CopySpeak.desktop"), LEGACY_ENTRY).unwrap();

            let appimage = Path::new("/tmp/.mount_CopySp/usr/bin/copyspeech");
            assert!(!migrate_in(&dir, appimage, &bins()).unwrap());
            assert!(dir.join("CopySpeak.desktop").exists());
            assert!(!dir.join("CopySpeech.desktop").exists());
        }

        #[test]
        fn dev_build_is_not_migrated() {
            let (dir, _guard) = scratch("dev");
            std::fs::write(dir.join("CopySpeak.desktop"), LEGACY_ENTRY).unwrap();

            let dev = Path::new("/home/user/CopySpeak/src-tauri/target/debug/copyspeech");
            assert!(!migrate_in(&dir, dev, &bins()).unwrap());
            assert!(!dir.join("CopySpeech.desktop").exists());
        }

        #[test]
        fn unverified_legacy_content_is_not_migrated() {
            let (dir, _guard) = scratch("unverified");
            std::fs::write(
                dir.join("CopySpeak.desktop"),
                "[Desktop Entry]\nName=CopySpeak\nExec=/home/user/dev/copyspeak\n",
            )
            .unwrap();

            assert!(!migrate_in(&dir, Path::new("/usr/bin/copyspeech"), &bins()).unwrap());
            assert!(!dir.join("CopySpeech.desktop").exists());
        }

        #[test]
        fn missing_legacy_entry_is_not_migrated() {
            let (dir, _guard) = scratch("missing");
            assert!(!migrate_in(&dir, Path::new("/usr/bin/copyspeech"), &bins()).unwrap());
            assert!(!dir.join("CopySpeech.desktop").exists());
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_imp {
    use super::*;

    const LABEL: &str = "com.copyspeak.tts";

    /// `~/Library/LaunchAgents` — per-user launchd agents.
    fn launch_agents_dir() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME")
            .ok_or("HOME is not set")
            .map(PathBuf::from)?;
        Ok(home.join("Library").join("LaunchAgents"))
    }

    fn entry_path() -> Result<PathBuf, String> {
        Ok(launch_agents_dir()?.join(format!("{LABEL}.plist")))
    }

    fn plist_contents() -> Result<String, String> {
        let exe = get_current_exe_path()?.display().to_string();
        Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \x20 <key>Label</key><string>{LABEL}</string>\n\
             \x20 <key>ProgramArguments</key>\n\
             \x20 <array>\n\
             \x20\x20 <string>{exe}</string>\n\
             \x20 </array>\n\
             \x20 <key>RunAtLoad</key><true/>\n\
             </dict>\n\
             </plist>\n"
        ))
    }

    pub fn enable_autostart() -> Result<(), String> {
        let path = entry_path()?;
        let dir = path.parent().expect("entry path has a parent");
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create LaunchAgents dir: {e}"))?;
        std::fs::write(&path, plist_contents()?)
            .map_err(|e| format!("Failed to write LaunchAgent plist: {e}"))?;
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
            Err(e) => Err(format!("Failed to remove LaunchAgent plist: {e}")),
        }
    }

    pub fn is_autostart_enabled() -> Result<bool, String> {
        Ok(entry_path()?.exists())
    }

    /// No 0.2.x launch agent existed under this label, so nothing to migrate.
    pub fn migrate_legacy_registration() -> Result<bool, String> {
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
use linux_imp::is_autostart_enabled as is_autostart_enabled_impl;
#[cfg(target_os = "linux")]
use linux_imp::{disable_autostart, enable_autostart, migrate_legacy_registration};
#[cfg(target_os = "macos")]
use macos_imp::is_autostart_enabled as is_autostart_enabled_impl;
#[cfg(target_os = "macos")]
use macos_imp::{disable_autostart, enable_autostart, migrate_legacy_registration};
#[cfg(target_os = "windows")]
use windows_imp::is_autostart_enabled as is_autostart_enabled_impl;
#[cfg(target_os = "windows")]
use windows_imp::{disable_autostart, enable_autostart, migrate_legacy_registration};

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

/// Startup migration from the CopySpeak 0.2.x launch-on-login registration to
/// CopySpeech. Returns whether a migration ran.
///
/// Runs only when launch-on-login is enabled and the build is not a dev build.
/// The platform gate then checks the installed location, the legacy
/// registration, and that no CopySpeech registration exists yet.
pub fn migrate_legacy_autostart(launch_on_login: bool) -> Result<bool, String> {
    migrate_legacy_with(
        launch_on_login,
        cfg!(debug_assertions),
        migrate_legacy_registration,
    )
}

fn migrate_legacy_with(
    launch_on_login: bool,
    dev_build: bool,
    migrate: impl FnOnce() -> Result<bool, String>,
) -> Result<bool, String> {
    if !launch_on_login || dev_build {
        return Ok(false);
    }
    migrate()
}

/// Keep saved preferences and OS registration together; propagate OS failures.
pub fn persist_with_autostart(
    old: bool,
    new: bool,
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    persist_change(old, new, persist, sync_autostart_with_config)
}

fn persist_change(
    old: bool,
    new: bool,
    persist: impl FnOnce() -> Result<(), String>,
    mut register: impl FnMut(bool) -> Result<(), String>,
) -> Result<(), String> {
    if old != new {
        register(new)?;
    }
    if let Err(error) = persist() {
        if old != new {
            if let Err(rollback) = register(old) {
                return Err(format!(
                    "{error}; failed to restore launch-on-login: {rollback}"
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_autostart_change_does_not_persist_settings() {
        let mut persisted = false;
        let result = persist_change(
            false,
            true,
            || {
                persisted = true;
                Ok(())
            },
            |_| Err("registration denied".into()),
        );
        assert_eq!(result, Err("registration denied".into()));
        assert!(!persisted);
    }

    #[test]
    fn failed_config_save_restores_previous_autostart() {
        let mut registrations = Vec::new();
        let result = persist_change(
            false,
            true,
            || Err("disk full".into()),
            |enabled| {
                registrations.push(enabled);
                Ok(())
            },
        );
        assert_eq!(result, Err("disk full".into()));
        assert_eq!(registrations, vec![true, false]);
    }

    #[test]
    fn unchanged_autostart_does_not_touch_os_registration() {
        let mut registrations = Vec::new();
        let result = persist_change(
            true,
            true,
            || Ok(()),
            |enabled| {
                registrations.push(enabled);
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert!(registrations.is_empty());
    }

    #[test]
    fn migration_runs_platform_gate_for_enabled_release_build() {
        let mut attempted = 0;
        let result = migrate_legacy_with(true, false, || {
            attempted += 1;
            Ok(true)
        });
        assert_eq!(result, Ok(true));
        assert_eq!(attempted, 1);
    }

    #[test]
    fn migration_skips_disabled_setting_without_calling_platform() {
        let mut attempted = false;
        let result = migrate_legacy_with(false, false, || {
            attempted = true;
            Ok(true)
        });
        assert_eq!(result, Ok(false));
        assert!(!attempted);
    }

    #[test]
    fn migration_skips_dev_build_without_calling_platform() {
        let mut attempted = false;
        let result = migrate_legacy_with(true, true, || {
            attempted = true;
            Ok(true)
        });
        assert_eq!(result, Ok(false));
        assert!(!attempted);
    }

    #[test]
    fn migration_reports_platform_failure() {
        let result = migrate_legacy_with(true, false, || Err("registry unreadable".into()));
        assert_eq!(result, Err("registry unreadable".into()));
    }

    #[test]
    fn test_get_current_exe_path() {
        let result = get_current_exe_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.to_string_lossy().ends_with(".exe")
                || path.to_string_lossy().contains("copyspeech")
        );
    }
}
