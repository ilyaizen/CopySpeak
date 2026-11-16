// Startup read guard for config.json and history.json.
//
// Runs before config or history load, cleanup, or any write. An existing file
// that is unreadable or does not match its schema blocks startup. A missing
// file is normal and passes. Error text names the path and the failure class
// only. It never prints file contents, because config.json holds provider keys.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::config::{self, AppConfig};
use crate::history::{self, HistoryLog};

#[derive(Debug)]
pub enum StartupDataError {
    /// The path exists but cannot be read (directory, permissions, I/O error).
    Unreadable { path: PathBuf, reason: String },
    /// The file reads, but is not JSON or does not match the expected schema.
    Invalid {
        path: PathBuf,
        reason: &'static str,
        line: usize,
        column: usize,
    },
}

impl fmt::Display for StartupDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, reason } => write!(
                f,
                "CopySpeech startup blocked: {} cannot be read ({reason}). \
                 Nothing was loaded or changed. Fix access or restore the file, then restart.",
                path.display()
            ),
            Self::Invalid {
                path,
                reason,
                line,
                column,
            } => write!(
                f,
                "CopySpeech startup blocked: {} is invalid ({reason} at line {line}, column {column}). \
                 Nothing was loaded or changed. Repair or restore the file, then restart.",
                path.display()
            ),
        }
    }
}

impl std::error::Error for StartupDataError {}

/// Check config.json and history.json at their real app paths.
pub fn check_startup_data() -> Result<(), StartupDataError> {
    check_startup_data_at(&config::config_path(), &history::history_path())
}

/// Check both files at injected paths. Config is checked first.
/// Returns the first failure. Missing files pass.
pub fn check_startup_data_at(
    config_path: &Path,
    history_path: &Path,
) -> Result<(), StartupDataError> {
    check_file::<AppConfig>(config_path)?;
    check_file::<HistoryLog>(history_path)
}

/// Read one file and deserialize it into the expected schema type.
fn check_file<T: DeserializeOwned>(path: &Path) -> Result<(), StartupDataError> {
    if path.is_dir() {
        return Err(StartupDataError::Unreadable {
            path: path.to_path_buf(),
            reason: "path is a directory".into(),
        });
    }

    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        // Only a true "not found" counts as a missing (normal first-run) file.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(StartupDataError::Unreadable {
                path: path.to_path_buf(),
                reason: format!("{:?}: {e}", e.kind()),
            });
        }
    };

    // Do not use serde's Display text: data errors can echo the offending value.
    serde_json::from_slice::<T>(&bytes)
        .map(|_| ())
        .map_err(|e| {
            let reason = match e.classify() {
                serde_json::error::Category::Io => "read error",
                serde_json::error::Category::Syntax => "not valid JSON",
                serde_json::error::Category::Data => "does not match expected schema",
                serde_json::error::Category::Eof => "unexpected end of file",
            };
            StartupDataError::Invalid {
                path: path.to_path_buf(),
                reason,
                line: e.line(),
                column: e.column(),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Unique scratch dir under the OS temp dir. Removed by `Scratch::drop`.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "copyspeech-startup-guard-{name}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        fn path(&self, file: &str) -> PathBuf {
            self.0.join(file)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Valid config JSON generated from the real `AppConfig` type.
    fn valid_config_json() -> String {
        serde_json::to_string_pretty(&AppConfig::default()).expect("serialize AppConfig")
    }

    /// Valid history JSON generated from the real `HistoryLog` type.
    fn valid_history_json() -> String {
        serde_json::to_string_pretty(&HistoryLog::new()).expect("serialize HistoryLog")
    }

    #[test]
    fn missing_files_pass() {
        let s = Scratch::new("missing");
        let result = check_startup_data_at(&s.path("config.json"), &s.path("history.json"));
        assert!(result.is_ok(), "missing files must pass: {result:?}");
    }

    #[test]
    fn valid_generated_files_pass() {
        let s = Scratch::new("valid");
        let cfg = s.path("config.json");
        let hist = s.path("history.json");
        std::fs::write(&cfg, valid_config_json()).unwrap();
        std::fs::write(&hist, valid_history_json()).unwrap();
        assert!(check_startup_data_at(&cfg, &hist).is_ok());
    }

    #[test]
    fn missing_config_with_valid_history_passes() {
        let s = Scratch::new("missing-config");
        let hist = s.path("history.json");
        std::fs::write(&hist, valid_history_json()).unwrap();
        assert!(check_startup_data_at(&s.path("config.json"), &hist).is_ok());
    }

    #[test]
    fn malformed_config_json_blocks_with_path_and_position() {
        let s = Scratch::new("malformed-config");
        let cfg = s.path("config.json");
        std::fs::write(&cfg, "{\n  \"general\": ,\n}").unwrap();
        let err = check_startup_data_at(&cfg, &s.path("history.json"))
            .expect_err("malformed config must block");
        match &err {
            StartupDataError::Invalid {
                path, reason, line, ..
            } => {
                assert_eq!(path, &cfg);
                assert_eq!(*reason, "not valid JSON");
                assert_eq!(*line, 2);
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert!(err.to_string().contains(&cfg.display().to_string()));
    }

    #[test]
    fn malformed_history_json_blocks() {
        let s = Scratch::new("malformed-history");
        let hist = s.path("history.json");
        std::fs::write(&hist, "{ \"entries\": [").unwrap();
        let err = check_startup_data_at(&s.path("config.json"), &hist)
            .expect_err("malformed history must block");
        assert!(matches!(
            err,
            StartupDataError::Invalid { ref path, .. } if path == &hist
        ));
    }

    #[test]
    fn empty_file_blocks_as_unexpected_eof() {
        let s = Scratch::new("empty");
        let cfg = s.path("config.json");
        std::fs::write(&cfg, "").unwrap();
        let err = check_startup_data_at(&cfg, &s.path("history.json"))
            .expect_err("empty file must block");
        assert!(matches!(
            err,
            StartupDataError::Invalid {
                reason: "unexpected end of file",
                ..
            }
        ));
    }

    #[test]
    fn non_utf8_bytes_block_as_invalid() {
        let s = Scratch::new("non-utf8");
        let hist = s.path("history.json");
        std::fs::write(&hist, [0xff_u8, 0xfe, 0x00, 0x7b]).unwrap();
        let err =
            check_startup_data_at(&s.path("config.json"), &hist).expect_err("non-UTF-8 must block");
        assert!(matches!(err, StartupDataError::Invalid { .. }));
    }

    #[test]
    fn schema_mismatch_blocks_without_echoing_value() {
        let s = Scratch::new("schema");
        let cfg = s.path("config.json");
        // Start from a real generated config, then break one field's type.
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_config_json()).expect("parse generated config");
        value["general"] = serde_json::Value::String("SECRET-SENTINEL-XYZ".into());
        std::fs::write(&cfg, serde_json::to_string(&value).unwrap()).unwrap();

        let err = check_startup_data_at(&cfg, &s.path("history.json"))
            .expect_err("schema mismatch must block");
        assert!(matches!(
            err,
            StartupDataError::Invalid {
                reason: "does not match expected schema",
                ..
            }
        ));
        assert!(
            !err.to_string().contains("SECRET-SENTINEL-XYZ"),
            "error text must not echo file values: {err}"
        );
    }

    #[test]
    fn directory_at_config_path_blocks_as_unreadable() {
        let s = Scratch::new("dir-config");
        let cfg = s.path("config.json");
        std::fs::create_dir_all(&cfg).unwrap();
        let err = check_startup_data_at(&cfg, &s.path("history.json"))
            .expect_err("directory at config path must block");
        assert!(matches!(err, StartupDataError::Unreadable { ref path, .. } if path == &cfg));
    }

    #[test]
    fn directory_at_history_path_blocks_as_unreadable() {
        let s = Scratch::new("dir-history");
        let hist = s.path("history.json");
        std::fs::create_dir_all(&hist).unwrap();
        let err = check_startup_data_at(&s.path("config.json"), &hist)
            .expect_err("directory at history path must block");
        assert!(matches!(err, StartupDataError::Unreadable { ref path, .. } if path == &hist));
    }

    #[test]
    fn config_error_reported_before_history() {
        let s = Scratch::new("order");
        let cfg = s.path("config.json");
        let hist = s.path("history.json");
        std::fs::write(&cfg, "not json").unwrap();
        std::fs::write(&hist, "also not json").unwrap();
        let err = check_startup_data_at(&cfg, &hist).expect_err("both bad must block");
        assert!(matches!(err, StartupDataError::Invalid { ref path, .. } if path == &cfg));
    }
}
