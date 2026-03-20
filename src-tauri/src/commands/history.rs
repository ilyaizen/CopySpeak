// History CRUD, search, export, and file-tracking commands.

use crate::audio::AudioPlayer;
use crate::history::{self, HistoryEntry, HistoryLog};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Compare two history entries by their fragment_index metadata.
/// Used for sorting batch entries in order.
fn compare_by_fragment_index(a: &HistoryEntry, b: &HistoryEntry) -> std::cmp::Ordering {
    let idx_a = a
        .metadata
        .get("fragment_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let idx_b = b
        .metadata
        .get("fragment_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    idx_a.cmp(&idx_b)
}

// ── Types ───────────────────────────────────────────────────────────────────

/// Pagination options for listing history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryListOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Sort order: "newest" or "oldest"
    pub sort_order: Option<String>,
}

/// Search filter options for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySearchOptions {
    pub search_text: Option<String>,
    pub tts_engine: Option<String>,
    pub voice: Option<String>,
    pub success_only: Option<bool>,
    /// Filter by date range (ISO date strings: YYYY-MM-DD)
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Export format for history data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HistoryExportFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "csv")]
    Csv,
}

/// Result of a history export operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryExportResult {
    pub file_path: String,
    pub entry_count: usize,
    pub format: String,
}

// ── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_history(history: State<'_, Mutex<HistoryLog>>) -> Vec<HistoryEntry> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history called");
    }
    let hist = history.lock().unwrap();
    hist.entries().iter().cloned().collect()
}

#[tauri::command]
pub fn clear_history(history: State<'_, Mutex<HistoryLog>>) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] clear_history called");
    }
    let mut hist = history.lock().unwrap();
    hist.clear();
    history::save(&hist)?;
    log::info!("History cleared");
    Ok(())
}

/// Get complete history with all metadata
#[tauri::command]
pub fn get_history_with_metadata(
    history: State<'_, Mutex<HistoryLog>>,
) -> (Vec<crate::history::HistoryEntry>, crate::history::HistoryLogMetadata) {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_with_metadata called");
    }
    let hist = history.lock().unwrap();
    let entries: Vec<_> = hist.entries().iter().cloned().collect();
    let metadata = hist.metadata.clone();
    (entries, metadata)
}

/// Get history statistics
#[tauri::command]
pub fn get_history_statistics(
    history: State<'_, Mutex<HistoryLog>>,
) -> crate::history::HistoryStatistics {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_statistics called");
    }
    let hist = history.lock().unwrap();
    hist.get_statistics()
}

/// Get file tracking information
#[tauri::command]
pub fn get_file_tracking(
    history: State<'_, Mutex<HistoryLog>>,
) -> crate::history::FileTrackingMetadata {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_file_tracking called");
    }
    let hist = history.lock().unwrap();
    hist.metadata.file_tracking.clone()
}

/// Get a history entry by its file path
#[tauri::command]
pub fn get_entry_by_file_path(
    history: State<'_, Mutex<HistoryLog>>,
    file_path: String,
) -> Option<HistoryEntry> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_entry_by_file_path called (path: {})", file_path);
    }
    let hist = history.lock().unwrap();
    hist.get_entry_by_file_path(&file_path).cloned()
}

/// Verify that a tracked file exists on disk
#[tauri::command]
pub fn verify_file_exists(
    history: State<'_, Mutex<HistoryLog>>,
    file_path: String,
) -> bool {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] verify_file_exists called (path: {})", file_path);
    }
    let mut hist = history.lock().unwrap();
    hist.verify_file_exists(&file_path)
}

/// Verify all tracked files and update their existence status
#[tauri::command]
pub fn verify_all_files(
    history: State<'_, Mutex<HistoryLog>>,
) -> (usize, usize) {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] verify_all_files called");
    }
    let mut hist = history.lock().unwrap();
    hist.verify_all_files()
}

/// Get all orphaned files (files in tracking directory but not referenced)
#[tauri::command]
pub fn get_orphaned_files(
    history: State<'_, Mutex<HistoryLog>>,
) -> Vec<String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_orphaned_files called");
    }
    let hist = history.lock().unwrap();
    hist.get_orphaned_files()
}

/// Get all missing files (tracked files that don't exist on disk)
#[tauri::command]
pub fn get_missing_files(
    history: State<'_, Mutex<HistoryLog>>,
) -> Vec<(String, String)> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_missing_files called");
    }
    let hist = history.lock().unwrap();
    hist.get_missing_files()
}

/// Remove a file from tracking (but keep the history entry)
#[tauri::command]
pub fn unlink_file(
    history: State<'_, Mutex<HistoryLog>>,
    file_path: String,
) -> Result<Option<String>, String> {
    log::debug!("[IPC] unlink_file called (path: {})", file_path);
    let mut hist = history.lock().unwrap();
    let entry_id = hist.unlink_file(&file_path);
    history::save(&hist)?;
    Ok(entry_id)
}

/// Get file metadata for a history entry
#[tauri::command]
pub fn get_file_metadata(
    history: State<'_, Mutex<HistoryLog>>,
    entry_id: String,
) -> Option<crate::history::FileMetadata> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_file_metadata called (entry_id: {})", entry_id);
    }
    let hist = history.lock().unwrap();
    hist.get_file_metadata(&entry_id).cloned()
}

/// Check if a file path is tracked in history
#[tauri::command]
pub fn is_file_tracked(
    history: State<'_, Mutex<HistoryLog>>,
    file_path: String,
) -> bool {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] is_file_tracked called (path: {})", file_path);
    }
    let hist = history.lock().unwrap();
    hist.is_file_tracked(&file_path)
}

/// Delete a specific history entry by ID
#[tauri::command]
pub fn delete_history_entry(
    history: State<'_, Mutex<HistoryLog>>,
    player: State<'_, Mutex<AudioPlayer>>,
    entry_id: String,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] delete_history_entry called (id: {})", entry_id);
    }

    // Check if the entry is currently playing
    {
        let p = player.lock().unwrap();
        if let Some(playing_id) = p.get_playing_entry_id() {
            if playing_id == entry_id {
                log::warn!("Attempt to delete currently playing entry: {}", entry_id);
                return Err("Cannot delete this entry because its audio is currently playing. Please stop playback first.".to_string());
            }
        }
    }

    let mut hist = history.lock().unwrap();
    let entries_mut = hist.entries_mut();

    if let Some(pos) = entries_mut.iter().position(|e| e.id == entry_id) {
        let output_path = entries_mut[pos].output_path.clone();
        entries_mut.remove(pos);
        let _ = entries_mut;
        hist.metadata.last_modified = chrono::Utc::now();
        hist.metadata.total_entries_current = hist.entries().len() as u32;
        history::save(&hist)?;
        // Delete the associated audio file if it exists
        if let Some(path) = output_path {
            let file_path = std::path::Path::new(&path);
            if file_path.exists() {
                if let Err(e) = std::fs::remove_file(file_path) {
                    log::warn!("Failed to delete audio file {}: {}", path, e);
                } else {
                    log::info!("Deleted audio file: {}", path);
                }
            }
        }
        log::info!("Deleted history entry: {}", entry_id);
        Ok(())
    } else {
        let _ = entries_mut;
        Err(format!("History entry not found: {}", entry_id))
    }
}

/// Copy history entry text to clipboard.
#[tauri::command]
pub fn copy_history_entry_text(
    history: State<'_, Mutex<HistoryLog>>,
    entry_id: String,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] copy_history_entry_text called (id: {})", entry_id);
    }

    let hist = history.lock().unwrap();
    let entry = hist.get_by_id(&entry_id)
        .ok_or_else(|| format!("History entry not found: {}", entry_id))?;

    let text = entry.text.clone();
    drop(hist);

    crate::clipboard::set_clipboard_text(&text)?;

    log::info!("Copied text from history entry: {}", entry_id);
    Ok(())
}

/// List history entries with optional pagination and filtering
#[tauri::command]
pub fn list_history(
    history: State<'_, Mutex<HistoryLog>>,
    options: Option<HistoryListOptions>,
) -> Vec<HistoryEntry> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] list_history called");
    }

    let hist = history.lock().unwrap();
    let mut entries: Vec<HistoryEntry> = hist.entries().iter().cloned().collect();

    // Apply sorting
    let sort_order = options.as_ref().and_then(|o| o.sort_order.as_deref()).unwrap_or("newest");
    match sort_order {
        "oldest" => entries.reverse(),
        _ => {},
    }

    // Apply pagination
    let limit = options.as_ref().and_then(|o| o.limit).unwrap_or(100);
    let offset = options.as_ref().and_then(|o| o.offset).unwrap_or(0);

    entries.into_iter()
        .skip(offset)
        .take(limit)
        .collect()
}

/// Search history with text and filter options
#[tauri::command]
pub fn search_history(
    history: State<'_, Mutex<HistoryLog>>,
    options: HistorySearchOptions,
) -> Vec<HistoryEntry> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] search_history called");
    }

    let hist = history.lock().unwrap();

    let start_date = options.start_date.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
            .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let end_date = options.end_date.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|nd| nd.and_hms_opt(23, 59, 59).unwrap())
            .map(|dt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let results: Vec<HistoryEntry> = hist.entries()
        .iter()
        .filter(|entry| {
            if let Some(ref search_text) = options.search_text {
                if !entry.text.to_lowercase().contains(&search_text.to_lowercase()) {
                    return false;
                }
            }
            if let Some(ref engine) = options.tts_engine {
                if entry.tts_engine != *engine {
                    return false;
                }
            }
            if let Some(ref voice) = options.voice {
                if entry.voice != *voice {
                    return false;
                }
            }
            if let Some(success_only) = options.success_only {
                if success_only && !entry.success {
                    return false;
                }
            }
            if let Some(ref start) = start_date {
                if entry.timestamp < *start {
                    return false;
                }
            }
            if let Some(ref end) = end_date {
                if entry.timestamp > *end {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    let limit = options.limit.unwrap_or(100);
    let offset = options.offset.unwrap_or(0);
    results.into_iter().skip(offset).take(limit).collect()
}

/// Export history to a file in the specified format
#[tauri::command]
pub fn export_history(
    history: State<'_, Mutex<HistoryLog>>,
    format: HistoryExportFormat,
) -> Result<HistoryExportResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] export_history called (format: {:?})", format);
    }

    let hist = history.lock().unwrap();
    let entries: Vec<_> = hist.entries().iter().cloned().collect();
    let entry_count = entries.len();

    let export_dir = history::history_dir();
    std::fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Failed to create export directory: {}", e))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = match format {
        HistoryExportFormat::Json => format!("history_export_{}.json", timestamp),
        HistoryExportFormat::Csv => format!("history_export_{}.csv", timestamp),
    };

    let file_path = export_dir.join(&filename);

    match format {
        HistoryExportFormat::Json => {
            let json = serde_json::to_string_pretty(&entries)
                .map_err(|e| format!("Serialize error: {}", e))?;
            std::fs::write(&file_path, json)
                .map_err(|e| format!("Write error: {}", e))?;
        }
        HistoryExportFormat::Csv => {
            let mut csv = "id,timestamp,text,tts_engine,voice,success\n".to_string();
            for entry in &entries {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    entry.id,
                    entry.timestamp,
                    entry.text.replace(",", ";"),
                    entry.tts_engine,
                    entry.voice,
                    entry.success
                ));
            }
            std::fs::write(&file_path, csv)
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    Ok(HistoryExportResult {
        file_path: file_path.to_string_lossy().to_string(),
        entry_count,
        format: match format {
            HistoryExportFormat::Json => "json".to_string(),
            HistoryExportFormat::Csv => "csv".to_string(),
        },
    })
}

/// Get unique TTS engines from history for filter dropdowns
#[tauri::command]
pub fn get_history_unique_engines(
    history: State<'_, Mutex<HistoryLog>>,
) -> Vec<String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_unique_engines called");
    }
    let hist = history.lock().unwrap();
    hist.get_unique_engines()
}

/// Get unique voices from history for filter dropdowns
#[tauri::command]
pub fn get_history_unique_voices(
    history: State<'_, Mutex<HistoryLog>>,
) -> Vec<String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_unique_voices called");
    }
    let hist = history.lock().unwrap();
    hist.get_unique_voices()
}

/// Get unique tags from history for filter dropdowns
#[tauri::command]
pub fn get_history_unique_tags(
    history: State<'_, Mutex<HistoryLog>>,
) -> Vec<String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_unique_tags called");
    }
    let hist = history.lock().unwrap();
    hist.get_unique_tags()
}

/// Get date range of history entries for filter UI
#[tauri::command]
pub fn get_history_date_range(
    history: State<'_, Mutex<HistoryLog>>,
) -> Option<(String, String)> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_date_range called");
    }
    let hist = history.lock().unwrap();
    hist.get_date_range().map(|(start, end)| {
        (
            start.format("%Y-%m-%d").to_string(),
            end.format("%Y-%m-%d").to_string(),
        )
    })
}

/// Run history cleanup with specified options
#[tauri::command]
pub fn run_history_cleanup(
    history: State<'_, Mutex<HistoryLog>>,
    config: State<'_, Mutex<crate::config::AppConfig>>,
) -> Result<(usize, usize), String> {
    log::debug!("[IPC] run_history_cleanup called");

    let cfg = config.lock().map_err(|e| e.to_string())?;

    // Copy the relevant history config values to avoid lifetime issues
    let cleanup_files = cfg.history.cleanup_orphaned_files;
    let auto_delete_mode = cfg.history.auto_delete.clone();

    // Drop the lock here before calling history::cleanup_* functions
    drop(cfg);

    let files_removed = if cleanup_files {
        match history::cleanup_orphaned_files(&history) {
            Ok(count) => count,
            Err(e) => {
                log::error!("Failed to cleanup orphaned files: {}", e);
                0
            }
        }
    } else {
        0
    };

    let entries_removed = match history::cleanup_old_entries(&history, &auto_delete_mode) {
        Ok(count) => count,
        Err(e) => {
            log::error!("Failed to cleanup old entries: {}", e);
            0
        }
    };

    log::info!(
        "History cleanup completed: {} files removed, {} entries removed",
        files_removed,
        entries_removed
    );

    Ok((files_removed, entries_removed))
}

/// Get all history entries that belong to a batch.
/// Returns entries sorted by fragment_index (from metadata).
#[tauri::command]
pub fn get_history_batch(
    history: State<'_, Mutex<HistoryLog>>,
    batch_id: String,
) -> Vec<HistoryEntry> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_history_batch called (batch_id: {})", batch_id);
    }

    let hist = history.lock().unwrap();
    let mut entries: Vec<HistoryEntry> = hist
        .entries()
        .iter()
        .filter(|e| e.batch_id.as_deref() == Some(batch_id.as_str()))
        .cloned()
        .collect();

    entries.sort_by(compare_by_fragment_index);

    entries
}

/// Delete all history entries in a batch.
/// Removes entries and their associated audio files.
#[tauri::command]
pub fn delete_history_batch(
    history: State<'_, Mutex<HistoryLog>>,
    batch_id: String,
) -> Result<usize, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] delete_history_batch called (batch_id: {})", batch_id);
    }

    let mut hist = history.lock().unwrap();

    // Collect IDs and paths to delete
    let to_delete: Vec<(String, Option<String>)> = hist
        .entries()
        .iter()
        .filter(|e| e.batch_id.as_deref() == Some(batch_id.as_str()))
        .map(|e| (e.id.clone(), e.output_path.clone()))
        .collect();

    let deleted_count = to_delete.len();

    if deleted_count == 0 {
        return Ok(0);
    }

    // Remove entries from history
    hist.entries_mut()
        .retain(|e| e.batch_id.as_deref() != Some(batch_id.as_str()));

    hist.metadata.last_modified = chrono::Utc::now();
    hist.metadata.total_entries_current = hist.entries().len() as u32;

    history::save(&hist)?;

    // Delete associated audio files
    for (_, output_path) in &to_delete {
        if let Some(path) = output_path {
            let file_path = std::path::Path::new(path);
            if file_path.exists() {
                if let Err(e) = std::fs::remove_file(file_path) {
                    log::warn!("Failed to delete audio file {}: {}", path, e);
                } else {
                    log::info!("Deleted audio file: {}", path);
                }
            }
        }
    }

    log::info!(
        "Deleted batch {} ({} entries)",
        batch_id,
        deleted_count
    );

    Ok(deleted_count)
}

/// Play all audio fragments in a batch sequentially.
/// Emits audio-fragment-ready events for each fragment.
#[tauri::command]
pub async fn play_history_batch(
    app: AppHandle,
    history: State<'_, Mutex<HistoryLog>>,
    player: State<'_, Mutex<AudioPlayer>>,
    batch_id: String,
) -> Result<(), String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] play_history_batch called (batch_id: {})", batch_id);
    }

    // Check if audio is already playing
    {
        let p = player.lock().unwrap();
        if p.is_playing() {
            log::warn!("Cannot start batch playback: audio already playing");
            return Err("Audio is already playing. Please stop current playback first.".to_string());
        }
    }

    // Get all entries for this batch, sorted by fragment_index
    let entries: Vec<HistoryEntry> = {
        let hist = history.lock().unwrap();
        let mut batch_entries: Vec<HistoryEntry> = hist
            .entries()
            .iter()
            .filter(|e| e.batch_id.as_deref() == Some(batch_id.as_str()))
            .cloned()
            .collect();

        batch_entries.sort_by(compare_by_fragment_index);

        batch_entries
    };

    if entries.is_empty() {
        return Err(format!("No entries found for batch: {}", batch_id));
    }

    let total = entries.len();
    log::info!("[IPC] Playing batch {} ({} fragments)", batch_id, total);

    // Get the combined text from all fragments for HUD display
    let combined_text: String = entries.iter().map(|e|e.text.as_str()).collect::<Vec<_>>().join(" ");

    // Calculate total duration from all entries
    let total_duration_ms: u64 = entries.iter().map(|e| e.duration_ms).sum();

    // Track missing fragments for user notification
    let mut missing_fragments: Vec<(usize, String)> = Vec::new();

    // Show HUD for playback with total duration
    crate::hud::show_hud_playback(&app, Some(combined_text), Some(total_duration_ms));

    // Emit pagination started event
    let _ = app.emit(
        "pagination:started",
        crate::commands::PaginationEvent {
            total,
            current_index: 0,
            is_paginated: true,
        },
    );

    for (index, entry) in entries.iter().enumerate() {
        let output_path = match &entry.output_path {
            Some(path) => path.clone(),
            None => {
                log::warn!("Entry {} has no output_path, skipping", entry.id);
                missing_fragments.push((index, format!("fragment {} has no audio file", index + 1)));
                continue;
            }
        };

        if !std::path::Path::new(&output_path).exists() {
            log::warn!("Audio file not found: {}, skipping", output_path);
            missing_fragments.push((index, format!("fragment {} audio file missing", index + 1)));
            continue;
        }

        // Read audio file
        let wav_bytes = match std::fs::read(&output_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("Failed to read audio file '{}': {}", output_path, e);
                missing_fragments.push((index, format!("fragment {} audio file unreadable", index + 1)));
                continue;
            }
        };

        // Encode to base64
        use base64::{engine::general_purpose, Engine as _};
        let audio_base64 = general_purpose::STANDARD.encode(&wav_bytes);

        // Emit fragment-started event
        let _ = app.emit(
            "pagination:fragment-started",
            crate::commands::PaginationEvent {
                total,
                current_index: index,
                is_paginated: true,
            },
        );

        // Emit audio fragment ready
        let _ = app.emit(
            "audio-fragment-ready",
            crate::commands::AudioFragmentEvent {
                audio_base64,
                fragment_index: index,
                fragment_total: total,
                is_final: index == total - 1,
                text: entry.text.clone(),
            },
        );

        // Emit fragment-ready event
        let _ = app.emit(
            "pagination:fragment-ready",
            crate::commands::PaginationEvent {
                total,
                current_index: index,
                is_paginated: true,
            },
        );
    }

    // Notify user if any fragments were missing
    if !missing_fragments.is_empty() {
        let missing_count = missing_fragments.len();
        let missing_list: String = missing_fragments
            .iter()
            .map(|(_, desc)| desc.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        log::warn!(
            "[IPC] Batch {} playback: {} fragment(s) missing: {}",
            batch_id,
            missing_count,
            missing_list
        );
        let _ = app.emit(
            "batch:warning",
            serde_json::json!({
                "message": format!("{} fragment(s) could not be played: {}", missing_count, missing_list),
                "missing_fragments": missing_fragments.iter().map(|(idx, _)| idx).collect::<Vec<_>>(),
            }),
        );
    }

    log::info!("[IPC] Batch {} playback initiated", batch_id);
    Ok(())
}
