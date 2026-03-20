// History Manager: Centralized backend for history database operations
// Handles JSON metadata persistence, audio file tracking, and cleanup orchestration
#![allow(dead_code)]

use crate::history::{self, HistoryEntry, HistoryLog, HistoryLogMetadata};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Mutex;

/// History database manager providing high-level operations
pub struct HistoryManager {
    history: Mutex<HistoryLog>,
}

impl HistoryManager {
    /// Create a new history manager, loading existing history from disk
    pub fn new() -> Self {
        let history = history::load();
        Self {
            history: Mutex::new(history),
        }
    }

    /// Create a new history manager with a specific history log
    pub fn with_history(history: HistoryLog) -> Self {
        Self {
            history: Mutex::new(history),
        }
    }

    /// Get the inner history log (for direct access in commands)
    pub fn inner(&self) -> &Mutex<HistoryLog> {
        &self.history
    }

    /// Add a new entry to history and persist to disk
    pub fn add_entry(&self, entry: HistoryEntry) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        hist.add(entry);
        self.save_internal(&hist)
    }

    /// Add a new entry with minimal data and persist to disk
    pub fn add_entry_minimal(
        &self,
        text: &str,
        voice: &str,
        duration_ms: u64,
    ) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        hist.add(history::create_entry(text, "unknown", voice, 1.0));
        let entry_idx = hist.entries().len() - 1;
        hist.entries_mut()[entry_idx].duration_ms = duration_ms;
        hist.entries_mut()[entry_idx].success = true;
        hist.entries_mut()[entry_idx].attempts = 1;
        self.save_internal(&hist)
    }

    /// Get all history entries
    pub fn get_all_entries(&self) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries().iter().cloned().collect()
    }

    /// Get history entries with pagination
    pub fn get_entries_paginated(&self, offset: usize, limit: usize) -> (Vec<HistoryEntry>, usize) {
        let hist = self.history.lock().unwrap();
        let total = hist.entries().len();
        let entries: Vec<_> = hist
            .entries()
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        (entries, total)
    }

    /// Get a specific entry by ID
    pub fn get_entry_by_id(&self, id: &str) -> Option<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.get_by_id(id).cloned()
    }

    /// Search history entries by text content
    pub fn search_by_text(&self, query: &str) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        let query_lower = query.to_lowercase();
        hist.entries()
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }

    /// Search history entries by date range
    pub fn search_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Search history entries by voice
    pub fn search_by_voice(&self, voice: &str) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.voice == voice)
            .cloned()
            .collect()
    }

    /// Delete a specific entry by ID and persist changes
    pub fn delete_entry(&self, id: &str) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        let entry = hist.get_by_id(id).cloned();

        if let Some(entry) = entry {
            // Remove the associated audio file if it exists
            if let Some(ref output_path) = entry.output_path {
                if let Err(e) = std::fs::remove_file(output_path) {
                    log::warn!("Failed to remove audio file {}: {}", output_path, e);
                }
            }

            // Remove entry from history
            let idx = hist.entries().iter().position(|e| e.id == id);
            if let Some(idx) = idx {
                hist.entries_mut().remove(idx);
            }

            self.save_internal(&hist)?;
        }

        Ok(())
    }

    /// Delete multiple entries by IDs and persist changes
    pub fn delete_entries(&self, ids: &[String]) -> Result<usize, String> {
        let mut deleted = 0;
        for id in ids {
            self.delete_entry(id)?;
            deleted += 1;
        }
        Ok(deleted)
    }

    /// Clear all history entries and persist changes
    pub fn clear_all(&self) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;

        // Remove all audio files referenced in history
        let files_to_remove: Vec<_> = hist
            .entries()
            .iter()
            .filter_map(|e| e.output_path.clone())
            .collect();

        for file_path in files_to_remove {
            if let Err(e) = std::fs::remove_file(&file_path) {
                log::warn!("Failed to remove audio file {}: {}", file_path, e);
            }
        }

        // Clear history
        hist.clear();
        self.save_internal(&hist)
    }

    /// Update a history entry and persist changes
    pub fn update_entry(&self, entry: HistoryEntry) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        let idx = hist.entries().iter().position(|e| e.id == entry.id);

        if let Some(idx) = idx {
            hist.entries_mut()[idx] = entry;
            self.save_internal(&hist)?;
        }

        Ok(())
    }

    /// Update metadata for an entry (tags, annotations, etc.)
    pub fn update_entry_metadata(
        &self,
        id: &str,
        tags: Option<Vec<String>>,
        metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        let entry = hist.get_by_id(id).cloned();

        if let Some(mut entry) = entry {
            if let Some(new_tags) = tags {
                entry.tags = new_tags;
            }
            if let Some(new_metadata) = metadata {
                entry.metadata = new_metadata;
            }

            let idx = hist.entries().iter().position(|e| e.id == id);
            if let Some(idx) = idx {
                hist.entries_mut()[idx] = entry;
                self.save_internal(&hist)?;
            }
        }

        Ok(())
    }

    /// Get history metadata
    pub fn get_metadata(&self) -> HistoryLogMetadata {
        let hist = self.history.lock().unwrap();
        hist.metadata.clone()
    }

    /// Get history statistics
    pub fn get_statistics(&self) -> history::HistoryStatistics {
        let hist = self.history.lock().unwrap();
        hist.get_statistics()
    }

    /// Get unique TTS engines from history
    pub fn get_unique_engines(&self) -> Vec<String> {
        let hist = self.history.lock().unwrap();
        hist.get_unique_engines()
    }

    /// Get unique voices from history
    pub fn get_unique_voices(&self) -> Vec<String> {
        let hist = self.history.lock().unwrap();
        hist.get_unique_voices()
    }

    /// Get unique tags from history
    pub fn get_unique_tags(&self) -> Vec<String> {
        let hist = self.history.lock().unwrap();
        hist.get_unique_tags()
    }

    /// Get date range of history entries
    pub fn get_date_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let hist = self.history.lock().unwrap();
        hist.get_date_range()
    }

    /// Update file size tracking for a specific file
    pub fn update_file_size(&self, file_path: &str, size_bytes: u64) -> Result<(), String> {
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        hist.update_file_size(file_path, size_bytes);
        self.save_internal(&hist)
    }

    /// Cleanup orphaned files (files not referenced in history)
    pub fn cleanup_orphaned_files(&self) -> Result<usize, String> {
        history::cleanup_orphaned_files(&self.history)
    }

    /// Cleanup old entries based on auto delete mode
    pub fn cleanup_old_entries(&self, auto_delete_mode: &crate::config::AutoDeleteMode) -> Result<usize, String> {
        history::cleanup_old_entries(&self.history, auto_delete_mode)
    }

    /// Run full cleanup (orphaned files and old entries)
    pub fn run_full_cleanup(&self, auto_delete_mode: &crate::config::AutoDeleteMode) -> (usize, usize) {
        history::run_full_cleanup(&self.history, auto_delete_mode)
    }

    /// Persist history to disk
    pub fn save(&self) -> Result<(), String> {
        let hist = self.history.lock().map_err(|e| e.to_string())?;
        self.save_internal(&hist)
    }

    /// Internal save method (requires caller to hold lock or pass reference)
    fn save_internal(&self, hist: &HistoryLog) -> Result<(), String> {
        history::save(hist)
    }

    /// Reload history from disk
    pub fn reload(&self) -> Result<(), String> {
        let new_history = history::load();
        let mut hist = self.history.lock().map_err(|e| e.to_string())?;
        *hist = new_history;
        Ok(())
    }

    /// Get the history database path
    pub fn get_db_path(&self) -> PathBuf {
        history::history_path()
    }

    /// Get the history directory path
    pub fn get_history_dir(&self) -> PathBuf {
        history::history_dir()
    }

    /// Check if an entry exists by ID
    pub fn entry_exists(&self, id: &str) -> bool {
        let hist = self.history.lock().unwrap();
        hist.get_by_id(id).is_some()
    }

    /// Get the number of entries in history
    pub fn entry_count(&self) -> usize {
        let hist = self.history.lock().unwrap();
        hist.entries().len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        let hist = self.history.lock().unwrap();
        hist.entries.is_empty()
    }

    /// Get entries created since a given timestamp
    pub fn get_entries_since(&self, since: DateTime<Utc>) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Get entries before a given timestamp
    pub fn get_entries_before(&self, before: DateTime<Utc>) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.timestamp < before)
            .cloned()
            .collect()
    }

    /// Get entries that have audio files
    pub fn get_entries_with_audio(&self) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.output_path.is_some())
            .cloned()
            .collect()
    }

    /// Get successful entries only
    pub fn get_successful_entries(&self) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| e.success)
            .cloned()
            .collect()
    }

    /// Get failed entries only
    pub fn get_failed_entries(&self) -> Vec<HistoryEntry> {
        let hist = self.history.lock().unwrap();
        hist.entries()
            .iter()
            .filter(|e| !e.success)
            .cloned()
            .collect()
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}
