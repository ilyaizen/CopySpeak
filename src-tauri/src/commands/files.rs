//! File export helpers backed by the native save dialog.

/// Write UTF-8 text to a path the user picked through the native save dialog.
/// The dialog plugin owns path selection; this command only writes the bytes.
#[tauri::command]
pub fn save_text_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents.as_bytes()).map_err(|e| e.to_string())
}
