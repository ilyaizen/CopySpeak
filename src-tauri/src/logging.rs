use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming, WriteMode};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

pub fn logs_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("CopySpeak").join("logs")
}

pub fn init_logging() -> Result<(), String> {
    let log_dir = logs_dir();

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }

    // Use debug level in debug builds, info level in release builds
    #[cfg(debug_assertions)]
    let log_level = "debug";
    #[cfg(not(debug_assertions))]
    let log_level = "info";

    // Duplicate debug/info logs to stderr in debug builds, only warn in release
    #[cfg(debug_assertions)]
    let duplicate_level = Duplicate::Debug;
    #[cfg(not(debug_assertions))]
    let duplicate_level = Duplicate::Warn;

    Logger::try_with_str(log_level)
        .map_err(|e| format!("Invalid log spec: {}", e))?
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("app")
                .suppress_timestamp()
                .suffix("log"),
        )
        .rotate(
            Criterion::AgeOrSize(Age::Day, 10 * 1024 * 1024),
            Naming::Timestamps,
            Cleanup::KeepLogAndCompressedFiles(5, 0),
        )
        .duplicate_to_stderr(duplicate_level)
        .write_mode(WriteMode::BufferAndFlush)
        .format_for_files(flexi_logger::detailed_format)
        .start()
        .map_err(|e| format!("Failed to initialize logger: {}", e))?;

    log::info!("Logging initialized to {}", log_dir.display());
    Ok(())
}

pub fn set_debug_mode(enabled: bool) {
    let was_enabled = DEBUG_MODE.swap(enabled, Ordering::SeqCst);
    if enabled && !was_enabled {
        log::info!("Debug mode enabled - verbose logging active");
    } else if !enabled && was_enabled {
        log::info!("Debug mode disabled");
    }
}

pub fn is_debug_mode() -> bool {
    DEBUG_MODE.load(Ordering::SeqCst)
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if $crate::logging::is_debug_mode() {
            log::debug!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logs_dir_returns_valid_path() {
        let path = logs_dir();
        assert!(path.ends_with("logs"));
        assert!(path.to_string_lossy().contains("CopySpeak"));
    }

    #[test]
    fn test_logs_dir_uses_data_dir() {
        let path = logs_dir();
        let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        let expected = data_dir.join("CopySpeak").join("logs");
        assert_eq!(path, expected);
    }
}
