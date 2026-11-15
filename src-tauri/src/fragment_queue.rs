// Fragment queue management for sequential TTS processing.
// Handles queueing of text fragments and sequential playback with auto-advancement.
#![allow(dead_code)]

use crate::pagination::TextFragment;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Status of the fragment queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum QueueStatus {
    /// Queue is empty or not started
    Idle,
    /// Currently playing a fragment
    Playing,
    /// Playback is paused
    Paused,
    /// Queue is stopped
    Stopped,
}

/// A queued fragment with its synthesized audio.
#[derive(Clone)]
pub struct QueuedFragment {
    /// The text fragment to speak
    pub fragment: TextFragment,
    /// Synthesized audio bytes (None if not yet synthesized)
    pub audio: Option<Vec<u8>>,
}

/// Fragment queue for sequential TTS playback.
pub struct FragmentQueue {
    /// Queue of fragments to play
    fragments: Arc<Mutex<Vec<QueuedFragment>>>,
    /// Index of currently playing fragment (None if not playing)
    current_index: Arc<AtomicUsize>,
    /// Current queue status
    status: Arc<AtomicUsize>,
    /// Flag to stop playback
    stop_flag: Arc<AtomicBool>,
}

impl FragmentQueue {
    /// Create a new fragment queue.
    pub fn new() -> Self {
        Self {
            fragments: Arc::new(Mutex::new(Vec::new())),
            current_index: Arc::new(AtomicUsize::new(usize::MAX)),
            status: Arc::new(AtomicUsize::new(QueueStatus::Idle as usize)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Add a single text fragment to the queue.
    pub fn add_fragment(&self, fragment: TextFragment) {
        let queued = QueuedFragment {
            fragment,
            audio: None,
        };
        let mut fragments = self.fragments.lock().unwrap();
        fragments.push(queued);
        log::debug!(
            "[FragmentQueue] Added fragment {} to queue (total: {})",
            fragments.len(),
            fragments.len()
        );
    }

    /// Add multiple text fragments to the queue.
    pub fn add_fragments(&self, fragments: Vec<TextFragment>) {
        let mut queue = self.fragments.lock().unwrap();
        for fragment in fragments {
            queue.push(QueuedFragment {
                fragment,
                audio: None,
            });
        }
        log::debug!(
            "[FragmentQueue] Added {} fragments to queue (total: {})",
            queue.len(),
            queue.len()
        );
    }

    /// Clear all fragments from the queue and reset the transport state.
    ///
    /// `speak_queued` clears before refilling, so a stop flag left set here
    /// would bail every subsequent queued reading at its first fragment check.
    pub fn clear(&self) {
        let mut fragments = self.fragments.lock().unwrap();
        fragments.clear();
        self.current_index.store(usize::MAX, Ordering::SeqCst);
        self.clear_stop_flag();
        self.set_status(QueueStatus::Idle);
        log::debug!("[FragmentQueue] Queue cleared");
    }

    /// Get the total number of fragments in the queue.
    pub fn len(&self) -> usize {
        let fragments = self.fragments.lock().unwrap();
        fragments.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the current queue status.
    pub fn status(&self) -> QueueStatus {
        match self.status.load(Ordering::SeqCst) {
            0 => QueueStatus::Idle,
            1 => QueueStatus::Playing,
            2 => QueueStatus::Paused,
            3 => QueueStatus::Stopped,
            _ => QueueStatus::Idle,
        }
    }

    /// Set the queue status.
    fn set_status(&self, status: QueueStatus) {
        self.status.store(status as usize, Ordering::SeqCst);
        log::debug!("[FragmentQueue] Status changed to: {:?}", status);
    }

    /// Get the index of the currently playing fragment.
    pub fn current_index(&self) -> Option<usize> {
        let idx = self.current_index.load(Ordering::SeqCst);
        if idx == usize::MAX {
            None
        } else {
            Some(idx)
        }
    }

    /// Set the index of the currently playing fragment.
    pub fn set_current_index(&self, index: usize) {
        self.current_index.store(index, Ordering::SeqCst);
    }

    /// Get the currently playing fragment.
    pub fn current_fragment(&self) -> Option<TextFragment> {
        let idx = self.current_index()?;
        let fragments = self.fragments.lock().unwrap();
        fragments.get(idx).map(|q| q.fragment.clone())
    }

    /// Get all fragments in the queue.
    pub fn fragments(&self) -> Vec<TextFragment> {
        let queue = self.fragments.lock().unwrap();
        queue.iter().map(|q| q.fragment.clone()).collect()
    }

    /// Get a specific fragment by index.
    pub fn get_fragment(&self, index: usize) -> Option<TextFragment> {
        let queue = self.fragments.lock().unwrap();
        queue.get(index).map(|q| q.fragment.clone())
    }

    /// Set synthesized audio for a fragment.
    pub fn set_audio(&self, index: usize, audio: Vec<u8>) {
        let mut queue = self.fragments.lock().unwrap();
        if let Some(queued) = queue.get_mut(index) {
            queued.audio = Some(audio);
        }
    }

    /// Get synthesized audio for a fragment.
    pub fn get_audio(&self, index: usize) -> Option<Vec<u8>> {
        let queue = self.fragments.lock().unwrap();
        queue.get(index).and_then(|q| q.audio.clone())
    }

    /// Check if a fragment has synthesized audio.
    pub fn has_audio(&self, index: usize) -> bool {
        let queue = self.fragments.lock().unwrap();
        queue.get(index).map(|q| q.audio.is_some()).unwrap_or(false)
    }

    /// Move to the next fragment in the queue.
    /// Returns the index of the next fragment, or None if at the end.
    /// This does NOT update the current_index - use set_current_index() with the result.
    pub fn next(&self) -> Option<usize> {
        let current_idx = self.current_index.load(Ordering::SeqCst);
        let queue_len = self.len();

        if queue_len == 0 {
            return None;
        }

        if current_idx == usize::MAX {
            // Not playing yet, start at first
            Some(0)
        } else if current_idx + 1 < queue_len {
            // Not at end, return next
            Some(current_idx + 1)
        } else {
            // At or past end
            None
        }
    }

    /// Move to the previous fragment in the queue.
    /// Returns the index of the previous fragment, or None if at the start.
    pub fn previous(&self) -> Option<usize> {
        let current_idx = self.current_index.load(Ordering::SeqCst);
        if current_idx == 0 || current_idx == usize::MAX {
            None
        } else {
            Some(current_idx.saturating_sub(1))
        }
    }

    /// Skip to a specific fragment index.
    pub fn skip_to(&self, index: usize) -> Result<(), String> {
        let queue_len = self.len();
        if index >= queue_len {
            return Err(format!(
                "Invalid fragment index: {} (queue size: {})",
                index, queue_len
            ));
        }
        self.current_index.store(index, Ordering::SeqCst);
        log::debug!("[FragmentQueue] Skipped to fragment {}", index);
        Ok(())
    }

    /// Stop playback and clear the stop flag.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.set_status(QueueStatus::Stopped);
        log::debug!("[FragmentQueue] Stop requested");
    }

    /// Check if stop was requested.
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    /// Clear the stop flag.
    pub fn clear_stop_flag(&self) {
        self.stop_flag.store(false, Ordering::SeqCst);
    }

    /// Pause playback.
    pub fn pause(&self) {
        if self.status() == QueueStatus::Playing {
            self.set_status(QueueStatus::Paused);
        }
    }

    /// Resume playback.
    pub fn resume(&self) {
        if self.status() == QueueStatus::Paused {
            self.set_status(QueueStatus::Playing);
        }
    }

    /// Start playback from the beginning or resume.
    pub fn start(&self) {
        if self.is_empty() {
            log::warn!("[FragmentQueue] Cannot start: queue is empty");
            return;
        }
        self.clear_stop_flag();
        if self.current_index.load(Ordering::SeqCst) == usize::MAX {
            self.current_index.store(0, Ordering::SeqCst);
        }
        self.set_status(QueueStatus::Playing);
    }
}

impl Default for FragmentQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "fragment_queue_tests.rs"]
mod tests;
