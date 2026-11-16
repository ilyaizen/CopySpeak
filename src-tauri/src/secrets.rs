// Secret resolution: overlay a .env file (next to the executable) on top of
// config.json values. Env wins when set & non-empty; config.json stays the
// fallback for keys typed in the UI. Env values are never written back to disk
// (config.json is serialized from its own fields, not the resolved values).

use std::path::{Path, PathBuf};

/// Load the active `.env` into the process environment at startup.
/// Lookup: `<exe-dir>/.env` first; on Windows, if that file is absent, the
/// previous install's `%LOCALAPPDATA%/CopySpeak/.env`. An existing but
/// unreadable file is NOT replaced by the fallback (logged, nothing loaded).
/// Format: `KEY=VALUE` per line; blank and `#`-prefixed lines are ignored.
/// Values are never logged; malformed lines are reported by line number only.
// ponytail: naive parser, no quotes/escapes/export — API tokens are simple
// tokens. Swap to dotenvy if the file ever needs shell-style quoting.
pub fn load_dotenv() {
    let Some(path) = select_dotenv_path(exe_dir().as_deref(), legacy_dotenv_dir().as_deref())
    else {
        return;
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            log::warn!(
                "[secrets] could not read {} ({:?}); no .env loaded",
                path.display(),
                err.kind()
            );
            return;
        }
    };

    let (pairs, malformed) = parse_dotenv(&contents);
    for line_no in &malformed {
        log::warn!(
            "[secrets] skipping malformed .env line {line_no} in {}",
            path.display()
        );
    }
    for (key, value) in &pairs {
        std::env::set_var(key, value);
    }
    if !pairs.is_empty() {
        log::info!(
            "[secrets] loaded {} var(s) from {}",
            pairs.len(),
            path.display()
        );
    }
}

/// Parse `.env` contents into `(key, value)` pairs plus the 1-based numbers of
/// malformed lines. Malformed lines are reported by number, never by content.
fn parse_dotenv(contents: &str) -> (Vec<(String, String)>, Vec<usize>) {
    let mut pairs = Vec::new();
    let mut malformed = Vec::new();
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    for (idx, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            malformed.push(idx + 1);
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        pairs.push((key.to_string(), value.trim().to_string()));
    }
    (pairs, malformed)
}

/// Choose which `.env` to load. The adjacent path wins unless it is absent;
/// unreadable paths do not silently fall back to older credentials.
fn select_dotenv_path(exe_dir: Option<&Path>, legacy_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = exe_dir {
        let adjacent = dir.join(".env");
        match std::fs::symlink_metadata(&adjacent) {
            Ok(_) => return Some(adjacent),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Some(adjacent),
        }
    }
    let legacy = legacy_dir?.join(".env");
    legacy.is_file().then_some(legacy)
}

/// Previous install's config directory on Windows (`%LOCALAPPDATA%/CopySpeak`).
/// None on other platforms or when LOCALAPPDATA is unset.
fn legacy_dotenv_dir() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("CopySpeak"))
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
// ponytail: executable-adjacent .env wins; old Windows installs are a fallback.
// In dev the executable is target/debug/copyspeech.exe.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // All paths and values here are temp dirs and dummy strings; no real
    // config, environment, or secrets are read.

    /// Fresh, empty directory under the OS temp dir. Callers remove it.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "copyspeech-secrets-test-{}-{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn adjacent_env_wins_over_legacy() {
        let adjacent = temp_dir("adjacent-wins-adj");
        let legacy = temp_dir("adjacent-wins-legacy");
        std::fs::write(adjacent.join(".env"), "KEY=dummy-adjacent\n").unwrap();
        std::fs::write(legacy.join(".env"), "KEY=dummy-legacy\n").unwrap();

        assert_eq!(
            select_dotenv_path(Some(&adjacent), Some(&legacy)),
            Some(adjacent.join(".env"))
        );

        let _ = std::fs::remove_dir_all(&adjacent);
        let _ = std::fs::remove_dir_all(&legacy);
    }

    #[test]
    fn legacy_env_used_when_adjacent_missing() {
        let adjacent = temp_dir("legacy-fallback-adj");
        let legacy = temp_dir("legacy-fallback-legacy");
        std::fs::write(legacy.join(".env"), "KEY=dummy-legacy\n").unwrap();

        assert_eq!(
            select_dotenv_path(Some(&adjacent), Some(&legacy)),
            Some(legacy.join(".env"))
        );

        let _ = std::fs::remove_dir_all(&adjacent);
        let _ = std::fs::remove_dir_all(&legacy);
    }

    #[test]
    fn legacy_used_when_exe_dir_unknown() {
        let legacy = temp_dir("legacy-no-exe-dir");
        std::fs::write(legacy.join(".env"), "KEY=dummy-legacy\n").unwrap();

        assert_eq!(
            select_dotenv_path(None, Some(&legacy)),
            Some(legacy.join(".env"))
        );

        let _ = std::fs::remove_dir_all(&legacy);
    }

    #[test]
    fn none_when_neither_exists() {
        let adjacent = temp_dir("none-adj");
        let legacy = temp_dir("none-legacy");

        assert_eq!(select_dotenv_path(Some(&adjacent), Some(&legacy)), None);
        assert_eq!(select_dotenv_path(None, None), None);

        let _ = std::fs::remove_dir_all(&adjacent);
        let _ = std::fs::remove_dir_all(&legacy);
    }

    #[test]
    fn directory_named_env_does_not_fall_back() {
        // An existing unreadable path must not be replaced by legacy credentials.
        let adjacent = temp_dir("dir-named-env-adj");
        let legacy = temp_dir("dir-named-env-legacy");
        std::fs::create_dir_all(adjacent.join(".env")).unwrap();
        std::fs::write(legacy.join(".env"), "KEY=dummy-legacy\n").unwrap();

        assert_eq!(
            select_dotenv_path(Some(&adjacent), Some(&legacy)),
            Some(adjacent.join(".env"))
        );

        let _ = std::fs::remove_dir_all(&adjacent);
        let _ = std::fs::remove_dir_all(&legacy);
    }

    #[test]
    fn parse_skips_blank_and_comments_and_trims() {
        let (pairs, malformed) =
            parse_dotenv("# comment\n\n  KEY_A = dummy-a  \nKEY_B=dummy-b\n=orphan\n");
        assert_eq!(
            pairs,
            vec![
                ("KEY_A".to_string(), "dummy-a".to_string()),
                ("KEY_B".to_string(), "dummy-b".to_string()),
            ]
        );
        assert!(malformed.is_empty());
    }

    #[test]
    fn parse_accepts_utf8_bom() {
        let (pairs, malformed) = parse_dotenv("\u{feff}KEY=dummy\n");
        assert_eq!(pairs, vec![("KEY".to_string(), "dummy".to_string())]);
        assert!(malformed.is_empty());
    }

    #[test]
    fn parse_reports_malformed_by_line_number_only() {
        let (pairs, malformed) =
            parse_dotenv("KEY_A=dummy-a\ndummy-secret-no-equals\nKEY_B=dummy-b\n");
        assert_eq!(pairs.len(), 2);
        assert_eq!(malformed, vec![2]);
    }
}
