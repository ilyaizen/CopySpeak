// Fragment queue management for sequential TTS processing.
// Handles queueing of text fragments and sequential playback with auto-advancement.

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

    /// Get all fragments in the queue.
    pub fn fragments(&self) -> Vec<TextFragment> {
        let queue = self.fragments.lock().unwrap();
        queue.iter().map(|q| q.fragment.clone()).collect()
    }

    /// Set synthesized audio for a fragment.
    pub fn set_audio(&self, index: usize, audio: Vec<u8>) {
        let mut queue = self.fragments.lock().unwrap();
        if let Some(queued) = queue.get_mut(index) {
            queued.audio = Some(audio);
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
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.status(), QueueStatus::Idle);
    }

    #[test]
    fn test_add_fragments_and_len() {
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
        let fragments = vec![create_test_fragment("Test", 0, 1)];
        queue.add_fragments(fragments);
        assert_eq!(queue.len(), 1);
        queue.clear();
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_skip_to() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![
            create_test_fragment("First", 0, 3),
            create_test_fragment("Second", 1, 3),
            create_test_fragment("Third", 2, 3),
        ]);
        assert!(queue.skip_to(1).is_ok());
        assert_eq!(queue.current_index(), Some(1));
        assert!(queue.skip_to(10).is_err());
    }

    #[test]
    fn test_set_audio() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![create_test_fragment("Test", 0, 1)]);
        let fake_audio = vec![1, 2, 3, 4, 5];
        queue.set_audio(0, fake_audio);
    }

    #[test]
    fn test_stop_and_should_stop() {
        let queue = FragmentQueue::new();
        assert!(!queue.should_stop());
        queue.stop();
        assert!(queue.should_stop());
        assert_eq!(queue.status(), QueueStatus::Stopped);
    }

    #[test]
    fn test_fragments() {
        let queue = FragmentQueue::new();
        queue.add_fragments(vec![
            create_test_fragment("First", 0, 2),
            create_test_fragment("Second", 1, 2),
        ]);
        let frags = queue.fragments();
        assert_eq!(frags.len(), 2);
        assert_eq!(frags[0].text, "First");
    }
}
