//! Playback-completion signaling for callers that must block until the
//! webview finishes speaking (Hermes' control-server `wait` requests).
//!
//! Windows playback lives in the webview: the store's queue drain
//! (`finishPlayback`) and user stop (`handleStop`) are the only terminal
//! signals, so the frontend emits `playback-finished` there and the Rust side
//! waits on it here. A generation timestamp ignores finishes that raced with
//! the requester's own synthesis (e.g. a concurrent reader stopping early).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

lazy_static::lazy_static! {
    static ref LAST_FINISH_MS: AtomicU64 = AtomicU64::new(0);
}

/// Frontend hook: call when any playback run reaches a terminal state.
pub fn signal_finished() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    LAST_FINISH_MS.store(now, Ordering::Release);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Resolve when a `playback-finished` event newer than `not_before_ms` arrives
/// (or is already pending), or when `timeout` elapses first. Returns whether a
/// finish was observed. The control server treats a timeout as success — the
/// user simply stopped playback — so the boolean is for logging only.
pub async fn wait_until(not_before_ms: u64, timeout: Duration) -> bool {
    // A finish may already have landed between speak_now resolving and this
    // wait starting (fast cache replay, quick stop).
    if LAST_FINISH_MS.load(Ordering::Acquire) >= not_before_ms && not_before_ms > 0 {
        return true;
    }
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if LAST_FINISH_MS.load(Ordering::Acquire) >= not_before_ms {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Timestamp to pass as `not_before_ms` — everything older is ignored.
pub fn generation() -> u64 {
    now_ms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The tests share the process-global LAST_FINISH_MS; run them one at a
    /// time so one test's finish cannot satisfy the other's wait.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// A finish newer than the request's generation resolves immediately,
    /// including one that landed before the wait even started.
    #[tokio::test]
    async fn wait_resolves_on_pending_or_newer_finish() {
        let _guard = TEST_LOCK.lock().unwrap();
        let gen = generation();
        std::thread::sleep(Duration::from_millis(5));
        signal_finished();
        let started = std::time::Instant::now();
        assert!(wait_until(gen, Duration::from_secs(1)).await);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    /// Finishes older than the generation never satisfy the wait; the
    /// timeout path resolves false.
    #[tokio::test]
    async fn wait_ignores_stale_finish_until_timeout() {
        let _guard = TEST_LOCK.lock().unwrap();
        signal_finished();
        std::thread::sleep(Duration::from_millis(5));
        let gen = generation();
        let started = std::time::Instant::now();
        assert!(!wait_until(gen, Duration::from_millis(120)).await);
        assert!(started.elapsed() >= Duration::from_millis(120));
    }
}
