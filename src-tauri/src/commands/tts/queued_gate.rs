//! Serialization of queued readings for just-in-time fragment synthesis.
//!
//! A queued reading (`speak_queued`: double-copy, browser companion) emits its
//! fragments as `audio-fragment-ready` events, and the webview's fragment queue
//! plays them in ENQUEUE order. The JIT gate submits a later fragment only as
//! the previous one's playback advances, so a second reading must not enqueue
//! its first fragment before the active reading enqueued its last: the two
//! texts would interleave.
//!
//! Each reading holds this gate for its whole run. A reading that arrives while
//! another holds the gate asks the holder to stop waiting and submit the rest
//! at once, then waits for its turn ([`QueuedGate::flush_requested`]). That
//! keeps the queue FIFO, and a paused or long reading cannot block the next one
//! forever.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// One reading's turn to emit fragments. Dropped when the reading's synthesis
/// loop ends on any path, which releases the next reading.
pub(crate) struct QueuedTurn<'a> {
    /// Held for the reading's whole run; unreleased early on purpose.
    _turn: tokio::sync::MutexGuard<'a, ()>,
}

/// The process-wide turn, and the flush request a waiting reading posts.
pub(crate) struct QueuedGate {
    turn: Mutex<()>,
    flush: AtomicBool,
}

impl QueuedGate {
    pub(crate) fn new() -> Self {
        Self {
            turn: Mutex::new(()),
            flush: AtomicBool::new(false),
        }
    }

    /// Take this reading's turn. When another reading holds it, ask that
    /// reading to submit the rest at once first, so the wait ends with its
    /// remaining synthesis rather than its whole playback.
    pub(crate) async fn acquire(&self) -> QueuedTurn<'_> {
        // Set before waiting: a holder already inside a JIT wait sees the flag
        // on its next poll, and one that has not reached the wait yet checks it
        // before its first gate decision.
        self.flush.store(true, Ordering::SeqCst);
        let turn = self.turn.lock().await;
        // No holder is left to flush, so this reading gates normally again.
        self.flush.store(false, Ordering::SeqCst);
        QueuedTurn { _turn: turn }
    }

    /// True while a newer reading waits for this turn: submit the rest now.
    pub(crate) fn flush_requested(&self) -> bool {
        self.flush.load(Ordering::SeqCst)
    }
}

impl Default for QueuedGate {
    fn default() -> Self {
        Self::new()
    }
}

static GATE: OnceLock<QueuedGate> = OnceLock::new();

fn gate() -> &'static QueuedGate {
    GATE.get_or_init(QueuedGate::new)
}

/// Take the turn for one `speak_queued` reading.
pub(crate) async fn acquire() -> QueuedTurn<'static> {
    gate().acquire().await
}

/// True while a newer reading waits for the current one to flush its remaining
/// fragments. The JIT wait polls this so a paused reading cannot hold the turn.
pub(crate) fn flush_requested() -> bool {
    gate().flush_requested()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The common case: no reading in flight, so the turn is free and nothing
    /// asks for a flush.
    #[tokio::test]
    async fn a_free_turn_is_taken_at_once_without_a_flush_request() {
        let gate = QueuedGate::new();
        let turn = gate.acquire().await;
        assert!(!gate.flush_requested());
        drop(turn);
    }

    /// A reading that arrives during another asks the holder to flush and waits
    /// for the turn; the turn then passes to it with no stale flush request.
    #[tokio::test]
    async fn a_waiting_reading_flushes_the_holder_and_takes_the_next_turn() {
        let gate = Arc::new(QueuedGate::new());
        let first = gate.acquire().await;
        let waiter = {
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                let turn = gate.acquire().await;
                // The new holder must gate normally, not see the old request.
                let flushed_while_holding = gate.flush_requested();
                drop(turn);
                flushed_while_holding
            })
        };
        for _ in 0..200 {
            if gate.flush_requested() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(gate.flush_requested(), "the holder must be asked to flush");
        assert!(!waiter.is_finished(), "the waiter must wait for the turn");
        drop(first);
        assert!(
            !waiter.await.unwrap(),
            "the new holder must gate normally again"
        );
    }

    /// After a reading releases the turn, the next one takes it without a flush
    /// request left over.
    #[tokio::test]
    async fn the_turn_passes_to_the_next_reading_without_a_flush_request() {
        let gate = QueuedGate::new();
        drop(gate.acquire().await);
        let second = gate.acquire().await;
        assert!(!gate.flush_requested());
        drop(second);
    }
}
