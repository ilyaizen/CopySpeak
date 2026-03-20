// Speech history log: circular buffer of TTS events persisted to history.json.
// Records complete metadata including timestamp, text, voice, duration, engine, format, and file paths.

use crate::config;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

const MAX_ENTRIES: usize = 1000;

/// Complete metadata for a single history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    // Core identification
    pub id: String,
    pub timestamp: DateTime<Utc>,

    // Text content
    pub text: String,
    pub text_length: u32,

    // TTS settings
    pub tts_engine: String,
    pub voice: String,
    pub speed: f32,

    // Output format and file tracking
    pub output_format: Option<String>,
    pub output_path: Option<String>,
    pub duration_ms: u64,

    // Execution metadata
    pub batch_id: Option<String>,
    pub app_name: Option<String>,
    pub source: Option<String>,
    pub filters_applied: Vec<String>,

    // Status tracking
    pub success: bool,
    pub error_message: Option<String>,
    pub attempts: u32,

    // User annotations
    pub tags: Vec<String>,

    // Additional metadata (extensible key-value store)
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Container for history log with metadata tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryLog {
    /// Circular buffer of history entries
    pub entries: VecDeque<HistoryEntry>,

    /// Metadata about the history log itself
    pub metadata: HistoryLogMetadata,
}

/// Metadata about the history log collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryLogMetadata {
    /// Schema version for compatibility tracking
    pub version: String,

    /// When the history log was created
    pub created_at: DateTime<Utc>,

    /// When the history log was last modified
    pub last_modified: DateTime<Utc>,

    /// Count of total entries ever recorded (for analytics)
    pub total_entries_recorded: u64,

    /// Statistics about the current entries
    pub total_entries_current: u32,
    pub total_duration_ms: u64,
    pub successful_entries: u32,
    pub failed_entries: u32,

    /// Tracking of unique voices and engines used
    pub unique_voices: Vec<String>,
    pub unique_engines: Vec<String>,

    /// File tracking metadata
    pub file_tracking: FileTrackingMetadata,
}

/// Metadata for tracking associated files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTrackingMetadata {
    /// Map of output file paths to their history entry IDs
    pub file_to_entry: HashMap<String, String>,

    /// Total size of all audio files in bytes
    pub total_file_size_bytes: u64,

    /// Last time file tracking was updated
    pub last_updated: DateTime<Utc>,

    /// Map of entry IDs to their file metadata
    pub entry_file_metadata: HashMap<String, FileMetadata>,
}

/// Detailed metadata for a tracked audio file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes
    pub size_bytes: u64,

    /// File format (wav, mp3, etc.)
    pub format: Option<String>,

    /// When the file was created
    pub created_at: DateTime<Utc>,

    /// When the file was last modified
    pub modified_at: DateTime<Utc>,

    /// Whether the file exists on disk
    pub exists: bool,

    /// SHA-256 hash of the file for deduplication
    pub file_hash: Option<String>,
}

impl Default for HistoryLog {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl HistoryLog {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            entries: VecDeque::with_capacity(MAX_ENTRIES),
            metadata: HistoryLogMetadata {
                version: "1.0".to_string(),
                created_at: now,
                last_modified: now,
                total_entries_recorded: 0,
                total_entries_current: 0,
                total_duration_ms: 0,
                successful_entries: 0,
                failed_entries: 0,
                unique_voices: Vec::new(),
                unique_engines: Vec::new(),
                file_tracking: FileTrackingMetadata {
                    file_to_entry: HashMap::new(),
                    total_file_size_bytes: 0,
                    last_updated: now,
                    entry_file_metadata: HashMap::new(),
                },
            },
        }
    }

    pub fn add(&mut self, entry: HistoryEntry) {
        // Update statistics
        self.metadata.last_modified = Utc::now();
        self.metadata.total_entries_recorded += 1;
        self.metadata.total_duration_ms += entry.duration_ms;

        if entry.success {
            self.metadata.successful_entries += 1;
        } else {
            self.metadata.failed_entries += 1;
        }

        // Track unique voices and engines
        if !self.metadata.unique_voices.contains(&entry.voice) {
            self.metadata.unique_voices.push(entry.voice.clone());
        }
        if !self.metadata.unique_engines.contains(&entry.tts_engine) {
            self.metadata.unique_engines.push(entry.tts_engine.clone());
        }

        // Track output file if present
        if let Some(ref output_path) = entry.output_path {
            // Check if file exists and get its size
            let file_size = std::path::Path::new(output_path)
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);

            let format = std::path::Path::new(output_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|s| s.to_lowercase());

            self.link_file_to_entry(&entry.id, output_path, file_size, format);
        }

        // Manage circular buffer
        if self.entries.len() >= MAX_ENTRIES {
            if let Some(removed) = self.entries.pop_front() {
                // Remove file tracking for removed entry
                if let Some(ref path) = removed.output_path {
                    self.unlink_file(path);
                }
                // Update counters
                if removed.success {
                    self.metadata.successful_entries =
                        self.metadata.successful_entries.saturating_sub(1);
                } else {
                    self.metadata.failed_entries = self.metadata.failed_entries.saturating_sub(1);
                }
                self.metadata.total_duration_ms = self
                    .metadata
                    .total_duration_ms
                    .saturating_sub(removed.duration_ms);
            }
        }

        self.entries.push_back(entry);
        self.metadata.total_entries_current = self.entries.len() as u32;
    }

    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut VecDeque<HistoryEntry> {
        &mut self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.metadata.file_tracking.file_to_entry.clear();
        self.metadata.file_tracking.entry_file_metadata.clear();
        self.metadata.file_tracking.total_file_size_bytes = 0;
        self.metadata.last_modified = Utc::now();
        self.metadata.total_entries_current = 0;
        self.metadata.total_duration_ms = 0;
        self.metadata.successful_entries = 0;
        self.metadata.failed_entries = 0;
    }

    /// Get an entry by ID
    pub fn get_by_id(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get unique TTS engines from history
    pub fn get_unique_engines(&self) -> Vec<String> {
        use std::collections::HashSet;
        let mut engines: HashSet<String> = HashSet::new();
        for entry in &self.entries {
            engines.insert(entry.tts_engine.clone());
        }
        let mut result: Vec<String> = engines.into_iter().collect();
        result.sort();
        result
    }

    /// Get unique voices from history
    pub fn get_unique_voices(&self) -> Vec<String> {
        use std::collections::HashSet;
        let mut voices: HashSet<String> = HashSet::new();
        for entry in &self.entries {
            voices.insert(entry.voice.clone());
        }
        let mut result: Vec<String> = voices.into_iter().collect();
        result.sort();
        result
    }

    /// Get unique tags from history
    pub fn get_unique_tags(&self) -> Vec<String> {
        use std::collections::HashSet;
        let mut tags: HashSet<String> = HashSet::new();
        for entry in &self.entries {
            for tag in &entry.tags {
                tags.insert(tag.clone());
            }
        }
        let mut result: Vec<String> = tags.into_iter().collect();
        result.sort();
        result
    }

    /// Get date range of history entries
    pub fn get_date_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        if self.entries.is_empty() {
            return None;
        }
        let mut entries: Vec<&HistoryEntry> = self.entries.iter().collect();
        entries.sort_by_key(|e| e.timestamp);
        Some((entries.first()?.timestamp, entries.last()?.timestamp))
    }

    /// Update file size tracking
    #[allow(dead_code)]
    pub fn update_file_size(&mut self, _file_path: &str, size_bytes: u64) {
        self.metadata.file_tracking.total_file_size_bytes += size_bytes;
        self.metadata.file_tracking.last_updated = Utc::now();
    }

    /// Link a file path to a history entry with metadata
    pub fn link_file_to_entry(
        &mut self,
        entry_id: &str,
        file_path: &str,
        file_size: u64,
        format: Option<String>,
    ) {
        let now = Utc::now();

        let file_metadata = FileMetadata {
            size_bytes: file_size,
            format,
            created_at: now,
            modified_at: now,
            exists: true,
            file_hash: None,
        };

        self.metadata
            .file_tracking
            .file_to_entry
            .insert(file_path.to_string(), entry_id.to_string());
        self.metadata
            .file_tracking
            .entry_file_metadata
            .insert(entry_id.to_string(), file_metadata);
        self.metadata.file_tracking.total_file_size_bytes += file_size;
        self.metadata.file_tracking.last_updated = now;

        log::debug!(
            "Linked file '{}' to entry '{}' (size: {} bytes)",
            file_path,
            entry_id,
            file_size
        );
    }

    /// Get a history entry by its file path
    pub fn get_entry_by_file_path(&self, file_path: &str) -> Option<&HistoryEntry> {
        let entry_id = self.metadata.file_tracking.file_to_entry.get(file_path)?;
        self.get_by_id(entry_id)
    }

    /// Get a mutable history entry by its file path
    pub fn get_entry_by_file_path_mut(&mut self, file_path: &str) -> Option<&mut HistoryEntry> {
        let entry_id = self
            .metadata
            .file_tracking
            .file_to_entry
            .get(file_path)?
            .clone();
        self.entries_mut().iter_mut().find(|e| e.id == entry_id)
    }

    /// Get file metadata for a history entry
    pub fn get_file_metadata(&self, entry_id: &str) -> Option<&FileMetadata> {
        self.metadata
            .file_tracking
            .entry_file_metadata
            .get(entry_id)
    }

    /// Get file path for a history entry
    pub fn get_file_path(&self, entry_id: &str) -> Option<&String> {
        self.metadata
            .file_tracking
            .file_to_entry
            .iter()
            .find(|(_, id)| *id == entry_id)
            .map(|(path, _)| path)
    }

    /// Check if a file path is tracked in history
    pub fn is_file_tracked(&self, file_path: &str) -> bool {
        self.metadata
            .file_tracking
            .file_to_entry
            .contains_key(file_path)
    }

    /// Verify that a tracked file exists on disk
    pub fn verify_file_exists(&mut self, file_path: &str) -> bool {
        let entry_id = match self.metadata.file_tracking.file_to_entry.get(file_path) {
            Some(id) => id.clone(),
            None => return false,
        };

        let exists = std::path::Path::new(file_path).exists();

        if let Some(metadata) = self
            .metadata
            .file_tracking
            .entry_file_metadata
            .get_mut(&entry_id)
        {
            metadata.exists = exists;
        }

        if !exists {
            log::warn!("Tracked file not found: {}", file_path);
        }

        exists
    }

    /// Verify all tracked files and update their existence status
    pub fn verify_all_files(&mut self) -> (usize, usize) {
        let mut existing_count = 0;
        let mut missing_count = 0;

        for (file_path, entry_id) in self.metadata.file_tracking.file_to_entry.clone() {
            let exists = std::path::Path::new(&file_path).exists();

            if let Some(metadata) = self
                .metadata
                .file_tracking
                .entry_file_metadata
                .get_mut(&entry_id)
            {
                metadata.exists = exists;
            }

            if exists {
                existing_count += 1;
            } else {
                missing_count += 1;
            }
        }

        if missing_count > 0 {
            log::warn!(
                "File verification: {} files exist, {} files missing",
                existing_count,
                missing_count
            );
        }

        (existing_count, missing_count)
    }

    /// Remove a file from tracking (but keep the history entry)
    pub fn unlink_file(&mut self, file_path: &str) -> Option<String> {
        let entry_id = self
            .metadata
            .file_tracking
            .file_to_entry
            .remove(file_path)?;

        if let Some(metadata) = self
            .metadata
            .file_tracking
            .entry_file_metadata
            .remove(&entry_id)
        {
            self.metadata.file_tracking.total_file_size_bytes = self
                .metadata
                .file_tracking
                .total_file_size_bytes
                .saturating_sub(metadata.size_bytes);
        }

        self.metadata.file_tracking.last_updated = Utc::now();

        if let Some(entry) = self.entries_mut().iter_mut().find(|e| e.id == entry_id) {
            entry.output_path = None;
        }

        log::debug!("Unlinked file '{}' from entry '{}'", file_path, entry_id);
        Some(entry_id)
    }

    /// Get all orphaned files (files in tracking directory but not referenced)
    pub fn get_orphaned_files(&self) -> Vec<String> {
        let history_dir = history_dir();
        let mut orphaned = Vec::new();

        if !history_dir.exists() {
            return orphaned;
        }

        if let Ok(entries) = std::fs::read_dir(&history_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(path) = entry.path().to_str() {
                        if !self.is_file_tracked(path) && entry.path().is_file() {
                            orphaned.push(path.to_string());
                        }
                    }
                }
            }
        }

        orphaned
    }

    /// Get all missing files (tracked files that don't exist on disk)
    pub fn get_missing_files(&self) -> Vec<(String, String)> {
        let mut missing = Vec::new();

        for (file_path, entry_id) in &self.metadata.file_tracking.file_to_entry {
            if !std::path::Path::new(file_path).exists() {
                missing.push((file_path.clone(), entry_id.clone()));
            }
        }

        missing
    }

    /// Get total size of tracked files in a human-readable format
    pub fn get_total_file_size_human(&self) -> String {
        let bytes = self.metadata.file_tracking.total_file_size_bytes;
        format_file_size(bytes)
    }

    /// Get file size for a specific entry
    pub fn get_entry_file_size(&self, entry_id: &str) -> Option<u64> {
        self.get_file_metadata(entry_id)
            .map(|metadata| metadata.size_bytes)
    }

    /// Update file format for a history entry
    pub fn update_file_format(&mut self, entry_id: &str, format: &str) {
        if let Some(metadata) = self
            .metadata
            .file_tracking
            .entry_file_metadata
            .get_mut(entry_id)
        {
            metadata.format = Some(format.to_string());
            self.metadata.file_tracking.last_updated = Utc::now();
        }
    }

    /// Get statistics about current entries
    pub fn get_statistics(&self) -> HistoryStatistics {
        let total = self.entries.len();
        let success_count = self.entries.iter().filter(|e| e.success).count();

        HistoryStatistics {
            total_entries: total,
            successful_entries: success_count,
            failed_entries: total - success_count,
            total_duration_ms: self.metadata.total_duration_ms,
            average_duration_ms: if total > 0 {
                self.metadata.total_duration_ms / total as u64
            } else {
                0
            },
            unique_voices_count: self.metadata.unique_voices.len(),
            unique_engines_count: self.metadata.unique_engines.len(),
            file_size_bytes: self.metadata.file_tracking.total_file_size_bytes,
        }
    }
}

/// Format file size in bytes to human-readable format
#[allow(dead_code)]
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Statistics about the history log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStatistics {
    pub total_entries: usize,
    pub successful_entries: usize,
    pub failed_entries: usize,
    pub total_duration_ms: u64,
    pub average_duration_ms: u64,
    pub unique_voices_count: usize,
    pub unique_engines_count: usize,
    pub file_size_bytes: u64,
}

/// Get the path to the history.json file
pub fn history_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("CopySpeak").join("history.json")
}

/// Get the directory containing history data files
pub fn history_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("CopySpeak").join("history")
}

/// Load history from disk, creating default if not found
pub fn load() -> HistoryLog {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("History parse error, starting fresh: {e}");
            HistoryLog::new()
        }),
        Err(_) => {
            log::info!("No history found at {}, starting fresh", path.display());
            HistoryLog::new()
        }
    }
}

/// Save history to disk as JSON with pretty formatting
pub fn save(history: &HistoryLog) -> Result<(), String> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create history dir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(history).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

/// Generate a unique history entry ID
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let random = (timestamp as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        & 0xFFFFFFFF;
    format!("hist_{}_{:x}", timestamp, random)
}

/// Create a new history entry with complete metadata
#[allow(dead_code)]
pub fn create_entry(text: &str, tts_engine: &str, voice: &str, speed: f32) -> HistoryEntry {
    HistoryEntry {
        id: generate_id(),
        timestamp: Utc::now(),
        text: text.to_string(),
        text_length: text.len() as u32,
        tts_engine: tts_engine.to_string(),
        voice: voice.to_string(),
        speed,
        output_format: None,
        output_path: None,
        duration_ms: 0,
        batch_id: None,
        app_name: None,
        source: None,
        filters_applied: Vec::new(),
        success: false,
        error_message: None,
        attempts: 0,
        tags: Vec::new(),
        metadata: HashMap::new(),
    }
}

/// Add a new entry to the history with basic info
#[allow(dead_code)]
pub fn add_entry(
    history: &Mutex<HistoryLog>,
    text: &str,
    tts_engine: &str,
    voice: &str,
    duration_ms: u64,
    output_path: Option<String>,
) {
    add_entry_with_batch(
        history,
        text,
        tts_engine,
        voice,
        duration_ms,
        output_path,
        None,
        HashMap::new(),
    );
}

/// Add a new entry to the history with batch info and metadata
pub fn add_entry_with_batch(
    history: &Mutex<HistoryLog>,
    text: &str,
    tts_engine: &str,
    voice: &str,
    duration_ms: u64,
    output_path: Option<String>,
    batch_id: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
) {
    let entry = HistoryEntry {
        id: generate_id(),
        timestamp: Utc::now(),
        text: text.to_string(),
        text_length: text.len() as u32,
        tts_engine: tts_engine.to_string(),
        voice: voice.to_string(),
        speed: 1.0,
        output_format: None,
        output_path,
        duration_ms,
        batch_id,
        app_name: None,
        source: None,
        filters_applied: Vec::new(),
        success: true,
        error_message: None,
        attempts: 1,
        tags: Vec::new(),
        metadata,
    };

    let mut hist = history.lock().unwrap();
    hist.add(entry);

    if let Err(e) = save(&hist) {
        log::error!("Failed to save history: {}", e);
    }
}

/// Add a complete entry with all metadata
#[allow(dead_code)]
pub fn add_entry_complete(history: &Mutex<HistoryLog>, entry: HistoryEntry) -> Result<(), String> {
    let mut hist = history.lock().unwrap();
    hist.add(entry);
    save(&hist)
}

/// Cleanup orphaned history files not referenced in the current history log
pub fn cleanup_orphaned_files(history: &Mutex<HistoryLog>) -> Result<usize, String> {
    let orphaned_files = {
        let hist = history.lock().unwrap();
        hist.get_orphaned_files()
    };

    let mut files_removed = 0;

    for file_path in orphaned_files {
        match std::fs::remove_file(&file_path) {
            Ok(_) => {
                files_removed += 1;
                log::info!("Removed orphaned file: {}", file_path);
            }
            Err(e) => {
                log::warn!("Failed to remove file {}: {}", file_path, e);
            }
        }
    }

    Ok(files_removed)
}

/// Cleanup old history entries based on the given AutoDeleteMode
pub fn cleanup_old_entries(
    history: &Mutex<HistoryLog>,
    auto_delete_mode: &crate::config::AutoDeleteMode,
) -> Result<usize, String> {
    let mut hist = history.lock().unwrap();
    let initial_count = hist.entries().len();

    let mut files_to_remove: Vec<String> = Vec::new();

    match auto_delete_mode {
        crate::config::AutoDeleteMode::Never => {
            // Nothing to do
            return Ok(0);
        }
        crate::config::AutoDeleteMode::KeepLatest(max_entries) => {
            let max_entries = *max_entries as usize;
            if initial_count > max_entries {
                // Sort by timestamp descending (latest first) to safely keep latest
                let mut entries: Vec<HistoryEntry> = hist.entries().iter().cloned().collect();
                entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

                // Identify files to remove from the older entries
                for entry in entries.iter().skip(max_entries) {
                    if let Some(ref path) = entry.output_path {
                        files_to_remove.push(path.clone());
                    }
                }

                // Truncate to keep only the latest max_entries
                entries.truncate(max_entries);

                // Re-sort back to ascending for the circular buffer
                entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

                let mut new_deque = VecDeque::with_capacity(crate::history::MAX_ENTRIES);
                for e in entries {
                    new_deque.push_back(e);
                }
                *hist.entries_mut() = new_deque;
            }
        }
        crate::config::AutoDeleteMode::AfterDays(max_age_days) => {
            let max_age_days = (*max_age_days).max(1);
            let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);

            hist.entries_mut().retain(|entry| {
                if entry.timestamp < cutoff {
                    if let Some(ref path) = entry.output_path {
                        files_to_remove.push(path.clone());
                    }
                    false
                } else {
                    true
                }
            });
        }
    }

    let entries_removed = initial_count - hist.entries().len();
    hist.metadata.total_entries_current = hist.entries().len() as u32;
    hist.metadata.last_modified = Utc::now();

    drop(hist);

    for file_path in files_to_remove {
        if let Err(e) = std::fs::remove_file(&file_path) {
            log::warn!("Failed to remove file {}: {}", file_path, e);
        }
    }

    if entries_removed > 0 {
        let hist = history.lock().unwrap();
        save(&hist)?;
        log::info!(
            "Removed {} old history entries due to auto-delete mode",
            entries_removed
        );
    }

    Ok(entries_removed)
}

/// Run a full history cleanup including orphaned files and old entries
pub fn run_full_cleanup(
    history: &Mutex<HistoryLog>,
    auto_delete_mode: &crate::config::AutoDeleteMode,
) -> (usize, usize) {
    let files_removed = match cleanup_orphaned_files(history) {
        Ok(count) => count,
        Err(e) => {
            log::error!("Failed to cleanup orphaned files: {}", e);
            0
        }
    };

    let entries_removed = match cleanup_old_entries(history, auto_delete_mode) {
        Ok(count) => count,
        Err(e) => {
            log::error!("Failed to cleanup old entries: {}", e);
            0
        }
    };

    (files_removed, entries_removed)
}

/// Start background cleanup service with configurable retention settings
///
/// This service periodically runs cleanup operations based on configuration:
/// - Runs every 24 hours
/// - Removes entries based on AutoDeleteMode
/// - Optionally removes orphaned files (if cleanup_orphaned_files is true)
pub fn start_cleanup_service(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let cleanup_interval = std::time::Duration::from_secs(24 * 3600);
        let mut first_run = true;

        log::info!("History background cleanup service started");

        loop {
            // Skip delay on first run to execute cleanup immediately on startup
            if first_run {
                first_run = false;
            } else {
                std::thread::sleep(cleanup_interval);
            }

            // Re-read config to pick up any changes
            let (should_cleanup_orphaned, auto_delete_mode) = {
                let config: tauri::State<std::sync::Mutex<config::AppConfig>> = app_handle.state();
                let cfg = config.lock().unwrap();
                (
                    cfg.history.cleanup_orphaned_files,
                    cfg.history.auto_delete.clone(),
                )
            };

            let history: tauri::State<std::sync::Mutex<HistoryLog>> = app_handle.state();

            // Execute cleanup based on configuration
            let (files_removed, entries_removed) = if should_cleanup_orphaned {
                // Run full cleanup: orphaned files + old entries
                run_full_cleanup(&history, &auto_delete_mode)
            } else {
                // Only cleanup old entries
                let entries = match cleanup_old_entries(&history, &auto_delete_mode) {
                    Ok(count) => count,
                    Err(e) => {
                        log::error!("Failed to cleanup old entries: {}", e);
                        0
                    }
                };
                (0, entries)
            };

            // Log cleanup results
            if files_removed > 0 || entries_removed > 0 {
                log::info!(
                    "History cleanup: {} files removed, {} entries removed",
                    files_removed,
                    entries_removed
                );
            }
        }
    });
}

/// Save audio bytes to storage based on HistoryConfig mode.
/// `engine_identifier` - engine/preset name (e.g., "piper", "kokoro", "openai", "elevenlabs")
/// `voice_name` - human-readable voice name (e.g., "joe", "alloy", "rachel")
/// `file_ext` - audio file extension (e.g., "wav", "mp3")
pub fn save_audio_to_storage(
    config: &crate::config::HistoryConfig,
    audio_bytes: &[u8],
    engine_identifier: &str,
    voice_name: &str,
    file_ext: &str,
) -> Option<String> {
    if !config.enabled {
        return None;
    }

    let dir: std::path::PathBuf = match config.storage_mode {
        crate::config::StorageMode::Temp => {
            std::env::temp_dir().join("CopySpeak_Generations").into()
        }
        crate::config::StorageMode::Persistent => {
            config.persistent_dir.clone().unwrap_or_else(|| {
                dirs::document_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("CopySpeak")
            })
        }
    };

    let _ = std::fs::create_dir_all(&dir);

    let now = chrono::Local::now();
    let minute_key = now.format("%Y-%m-%d-%H-%M").to_string();
    let count = config::get_and_increment_minute_counter(&minute_key);
    let filename = if count == 1 {
        format!(
            "{}-{}-{}.{}",
            engine_identifier.to_lowercase(),
            voice_name.to_lowercase(),
            minute_key,
            file_ext
        )
    } else {
        format!(
            "{}-{}-{}-{}.{}",
            engine_identifier.to_lowercase(),
            voice_name.to_lowercase(),
            minute_key,
            count,
            file_ext
        )
    };
    let path = dir.join(filename);

    match std::fs::write(&path, audio_bytes) {
        Ok(_) => Some(path.to_string_lossy().into_owned()),
        Err(e) => {
            log::error!("Failed to save history audio: {}", e);
            None
        }
    }
}
