// Chrome native-messaging registration for the bundled browser companion host.
//
// Installed Windows builds ship `copyspeech-browser-host.exe` next to the app.
// On launch, the app writes the host manifest beside that binary and points
// Chrome's per-user registry key at it. Dev builds skip this, so a developer's
// repo host registration is never overwritten.

use std::path::Path;

pub const HOST_NAME: &str = "com.copyspeech.browser";
pub const HOST_EXE: &str = "copyspeech-browser-host.exe";
/// Chrome extension ID. The extension manifest pins it with a `key` field, so
/// the ID stays the same no matter which folder the extension loads from.
pub const CHROME_EXTENSION_ID: &str = "hojbmioeiccpgkpnpblkdbeegjkcccad";
const CHROME_HOSTS_KEY: &str = r"Software\Google\Chrome\NativeMessagingHosts";

/// Chrome native-messaging manifest for the bundled host.
pub fn host_manifest_json(host_exe: &Path) -> String {
    serde_json::json!({
        "name": HOST_NAME,
        "description": "CopySpeech browser companion native host",
        "path": host_exe.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{CHROME_EXTENSION_ID}/")],
    })
    .to_string()
}

/// Write the manifest and point Chrome at it for an installed build.
/// Returns `Ok(true)` when the manifest or registry value changed.
pub fn ensure_chrome_registration(resource_dir: &Path) -> Result<bool, String> {
    if cfg!(debug_assertions) {
        return Ok(false);
    }
    register_in(resource_dir, CHROME_HOSTS_KEY)
}

/// Pure-ish core: `hosts_key` is the HKCU subkey that holds Chrome's host entries.
/// Tests pass a scratch key so the real Chrome registration is never touched.
fn register_in(resource_dir: &Path, hosts_key: &str) -> Result<bool, String> {
    let host = resource_dir.join(HOST_EXE);
    if !host.is_file() {
        log::info!(
            "[browser] bundled host not found at {}; Chrome registration skipped",
            host.display()
        );
        return Ok(false);
    }

    let manifest_path = resource_dir.join(format!("{HOST_NAME}.json"));
    let manifest = host_manifest_json(&host);
    let manifest_changed =
        std::fs::read_to_string(&manifest_path).ok().as_deref() != Some(manifest.as_str());
    if manifest_changed {
        std::fs::write(&manifest_path, &manifest)
            .map_err(|e| format!("Failed to write {}: {e}", manifest_path.display()))?;
    }

    let registry_changed = point_registry_at(hosts_key, &manifest_path)?;
    Ok(manifest_changed || registry_changed)
}

#[cfg(windows)]
fn point_registry_at(hosts_key: &str, manifest_path: &Path) -> Result<bool, String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(format!(r"{hosts_key}\{HOST_NAME}"))
        .map_err(|e| format!("Failed to open Chrome host registry key: {e}"))?;
    let target = manifest_path.to_string_lossy().to_string();
    let current: Option<String> = key.get_value("").ok();
    if current.as_deref() == Some(target.as_str()) {
        return Ok(false);
    }
    key.set_value("", &target)
        .map_err(|e| format!("Failed to set Chrome host registry value: {e}"))?;
    log::info!("[browser] Chrome native host registered: {target}");
    Ok(true)
}

/// Other platforms register through their own installers (see
/// `browser-native-host/install-linux.sh`), so this is a no-op here.
#[cfg(not(windows))]
fn point_registry_at(_hosts_key: &str, _manifest_path: &Path) -> Result<bool, String> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Scratch directory plus scratch registry subkey, removed on drop.
    struct Scratch {
        dir: PathBuf,
        #[cfg(windows)]
        key: String,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let unique = format!("{name}-{}", std::process::id());
            let dir = std::env::temp_dir().join(format!("copyspeech-reg-{unique}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Scratch {
                dir,
                #[cfg(windows)]
                key: format!(r"Software\CopySpeechRegistrationTest\{unique}"),
            }
        }

        fn hosts_key(&self) -> String {
            #[cfg(windows)]
            {
                self.key.clone()
            }
            #[cfg(not(windows))]
            {
                String::from("unused")
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
            // Delete only this test's subkey. Tests run in parallel, so the
            // shared parent must stay.
            #[cfg(windows)]
            {
                use winreg::enums::HKEY_CURRENT_USER;
                let _ = winreg::RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.key);
            }
        }
    }

    fn fake_host(dir: &Path) {
        std::fs::write(dir.join(HOST_EXE), b"stub").expect("fake host");
    }

    #[test]
    fn manifest_names_host_and_pins_extension_origin() {
        let exe = Path::new(r"C:\Users\User\AppData\Local\CopySpeech\copyspeech-browser-host.exe");
        let parsed: serde_json::Value =
            serde_json::from_str(&host_manifest_json(exe)).expect("valid json");
        assert_eq!(parsed["name"], HOST_NAME);
        assert_eq!(parsed["type"], "stdio");
        assert_eq!(parsed["path"], exe.to_string_lossy().as_ref());
        assert_eq!(
            parsed["allowed_origins"][0],
            format!("chrome-extension://{CHROME_EXTENSION_ID}/")
        );
    }

    #[test]
    fn missing_host_writes_nothing_and_registers_nothing() {
        let scratch = Scratch::new("missing-host");
        let changed = register_in(&scratch.dir, &scratch.hosts_key()).expect("no error");
        assert!(!changed);
        assert!(!scratch.dir.join(format!("{HOST_NAME}.json")).exists());
    }

    #[test]
    fn registration_writes_manifest_and_is_idempotent() {
        let scratch = Scratch::new("idempotent");
        fake_host(&scratch.dir);
        let hosts = scratch.hosts_key();

        assert!(register_in(&scratch.dir, &hosts).expect("first run"));
        let manifest_path = scratch.dir.join(format!("{HOST_NAME}.json"));
        let written = std::fs::read_to_string(&manifest_path).expect("manifest written");
        assert_eq!(written, host_manifest_json(&scratch.dir.join(HOST_EXE)));

        assert!(
            !register_in(&scratch.dir, &hosts).expect("second run"),
            "nothing changed"
        );
    }

    #[test]
    fn moved_install_updates_manifest_and_registry() {
        let first = Scratch::new("moved-first");
        let second = Scratch::new("moved-second");
        fake_host(&first.dir);
        fake_host(&second.dir);
        let hosts = first.hosts_key();

        register_in(&first.dir, &hosts).expect("first install");
        assert!(register_in(&second.dir, &hosts).expect("reinstall elsewhere"));
    }
}
