// Secret resolution: overlay a .env file (next to the executable) on top of
// config.json values. Env wins when set & non-empty; config.json stays the
// fallback for keys typed in the UI. Env values are never written back to disk
// (config.json is serialized from its own fields, not the resolved values).

use std::path::PathBuf;

/// Load `<exe-dir>/.env` into the process environment at startup.
/// Format: `KEY=VALUE` per line; blank and `#`-prefixed lines are ignored.
// ponytail: naive parser, no quotes/escapes/export — API tokens are simple
// tokens. Swap to dotenvy if the file ever needs shell-style quoting.
pub fn load_dotenv() {
    let Some(dir) = exe_dir() else {
        return;
    };
    let path = dir.join(".env");
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };

    let mut count = 0;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            log::warn!("[secrets] skipping malformed .env line: {line}");
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        std::env::set_var(key, value.trim());
        count += 1;
    }
    if count > 0 {
        log::info!("[secrets] loaded {count} var(s) from {}", path.display());
    }
}

/// Resolve a credential: first non-empty value among the named env vars wins,
/// otherwise fall back to the config.json value. Returned values are trimmed.
pub fn resolve(config_val: &str, env_names: &[&str]) -> String {
    for name in env_names {
        if let Ok(v) = std::env::var(name) {
            let v = v.trim();
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    config_val.trim().to_string()
}

/// Directory containing the running executable — the .env lookup root.
// ponytail: single location (next to copyspeak.exe) per design; in dev the exe
// is target/debug/copyspeak.exe, so drop .env there for `cargo tauri dev`.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}
