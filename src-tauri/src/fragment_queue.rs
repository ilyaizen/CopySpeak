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

    /// Clear all fragments from the queue.
    pub fn clear(&self) {
        let mut fragments = self.fragments.lock().unwrap();
        fragments.clear();
        self.current_index.store(usize::MAX, Ordering::SeqCst);
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
mod tests {
    use super::*;

    fn create_test_fragment(text: &str, index: usize, total: usize) -> TextFragment {
        TextFragment::new(text.to_string(), index, total)
    }

    #[test]
    fn test_empty_queue() {
        let queue = FragmentQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.status(), QueueStatus::Idle);
    }

    #[test]
    fn test_add_single_fragment() {
        let queue = FragmentQueue::new();
        let fragment = create_test_fragment("Hello world", 0, 1);
        queue.add_fragment(fragment);

        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_add_multiple_fragments() {
        let queue = FragmentQueue::new();
        let fragments = vec![
            create_test_fragment("First", 0, 3),
            create_test_fragment("Second", 1, 3),
            create_test_fragment("Third", 2, 3),
        ];
        queue.add_fragments(fragments);

        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn test_clear_queue() {
        let queue = FragmentQueue::new();
        queue.add_fragment(create_test_fragment("Test", 0, 1));
        assert_eq!(queue.len(), 1);

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_navigation_first_time() {
        let queue = FragmentQueue::new();
        queue.add_fragment(create_test_fragment("First", 0, 2));
        queue.add_fragment(create_test_fragment("Second", 1, 2));

        // Not playing yet, should return index 0
        assert_eq!(queue.next(), Some(0));

        // Move to index 0
        queue.skip_to(0).unwrap();
        assert_eq!(queue.current_index(), Some(0));

        // Next should return index 1
        assert_eq!(queue.next(), Some(1));

        // Move to index 1
        queue.skip_to(1).unwrap();
        assert_eq!(queue.current_index(), Some(1));

        // Next should return None (end of queue)
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn test_navigation_after_set_index() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![
            create_test_fragment("First", 0, 3),
            create_test_fragment("Second", 1, 3),
            create_test_fragment("Third", 2, 3),
        ]);

        // Set current index to 1
        queue.skip_to(1).unwrap();
        assert_eq!(queue.current_index(), Some(1));

        // Next should be 2
        assert_eq!(queue.next(), Some(2));

        // Move to index 2
        queue.skip_to(2).unwrap();

        // Next should be None (at end)
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn test_navigation_previous() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![
            create_test_fragment("First", 0, 3),
            create_test_fragment("Second", 1, 3),
            create_test_fragment("Third", 2, 3),
        ]);

        queue.skip_to(2).unwrap();
        assert_eq!(queue.current_index(), Some(2));

        assert_eq!(queue.previous(), Some(1));

        queue.skip_to(1).unwrap();
        assert_eq!(queue.current_index(), Some(1));

        assert_eq!(queue.previous(), Some(0));

        queue.skip_to(0).unwrap();
        assert_eq!(queue.current_index(), Some(0));

        assert_eq!(queue.previous(), None);
    }

    #[test]
    fn test_skip_to() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![
            create_test_fragment("First", 0, 3),
            create_test_fragment("Second", 1, 3),
            create_test_fragment("Third", 2, 3),
        ]);

        // Valid skip
        assert!(queue.skip_to(1).is_ok());
        assert_eq!(queue.current_index(), Some(1));

        // Invalid skip
        assert!(queue.skip_to(10).is_err());
    }

    #[test]
    fn test_audio_storage() {
        let queue = FragmentQueue::new();
        queue.add_fragment(create_test_fragment("Test", 0, 1));

        assert!(!queue.has_audio(0));

        let fake_audio = vec![1, 2, 3, 4, 5];
        queue.set_audio(0, fake_audio.clone());

        assert!(queue.has_audio(0));
        assert_eq!(queue.get_audio(0), Some(fake_audio));
    }

    #[test]
    fn test_status_transitions() {
        let queue = FragmentQueue::new();
        queue.add_fragment(create_test_fragment("Test", 0, 1));

        assert_eq!(queue.status(), QueueStatus::Idle);

        queue.start();
        assert_eq!(queue.status(), QueueStatus::Playing);

        queue.pause();
        assert_eq!(queue.status(), QueueStatus::Paused);

        queue.resume();
        assert_eq!(queue.status(), QueueStatus::Playing);

        queue.stop();
        assert_eq!(queue.status(), QueueStatus::Stopped);
    }

    #[test]
    fn test_get_fragment() {
        let queue = FragmentQueue::new();
        let fragments = vec![
            create_test_fragment("First", 0, 2),
            create_test_fragment("Second", 1, 2),
        ];
        queue.add_fragments(fragments);

        let first = queue.get_fragment(0).unwrap();
        assert_eq!(first.text, "First");
        assert_eq!(first.index, 0);

        let second = queue.get_fragment(1).unwrap();
        assert_eq!(second.text, "Second");
        assert_eq!(second.index, 1);
    }

    #[test]
    fn test_stop_flag() {
        let queue = FragmentQueue::new();

        assert!(!queue.should_stop());

        queue.stop();
        assert!(queue.should_stop());

        queue.clear_stop_flag();
        assert!(!queue.should_stop());
    }
}
