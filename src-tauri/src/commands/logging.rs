use crate::logging;

/// Read the current application log file contents.
/// Returns the last N lines from the log file, or an error if the file cannot be read.
#[tauri::command]
pub fn get_logs(max_lines: Option<usize>) -> Result<String, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_logs called (max_lines: {:?})", max_lines);
    }

    let log_dir = logging::logs_dir();

    // Find the most recent log file (files starting with "app" and ending with ".log")
    let log_file = std::fs::read_dir(&log_dir)
        .map_err(|e| format!("Failed to read log directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            name.starts_with("app") && name.ends_with(".log")
        })
        .max_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

    let log_file = match log_file {
        Some(file) => file,
        None => return Ok("No log file found.".to_string()),
    };

    let content = std::fs::read_to_string(&log_file.path())
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    // If max_lines is specified, return only the last N lines
    if let Some(lines) = max_lines {
        let all_lines: Vec<&str> = content.lines().collect();
        let start = if all_lines.len() > lines {
            all_lines.len() - lines
        } else {
            0
        };
        let truncated: Vec<&str> = all_lines[start..].to_vec();
        Ok(truncated.join("\n"))
    } else {
        Ok(content)
    }
}

/// Get the path to the log directory.
#[tauri::command]
pub fn get_logs_path() -> Result<String, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_logs_path called");
    }

    let log_dir = logging::logs_dir();
    Ok(log_dir.to_string_lossy().to_string())
}
