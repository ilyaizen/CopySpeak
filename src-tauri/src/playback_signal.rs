//! Playback-completion signaling for callers that must block until the
//! webview finishes speaking (Hermes' control-server `wait` requests).
//!
//! Windows playback lives in the webview: the store's queue drain
//! (`finishPlayback`) and user stop (`handleStop`) are the only terminal
//! signals, so the frontend emits `playback-finished` there and the Rust side
//! waits on it here. Each active waiter records only its own reading's finish,
//! including finishes that arrive before synthesis returns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

/// Freshness window for a reported caption snapshot. The webview reports at
/// about 10 Hz while playback runs, so an older snapshot means playback
/// stopped (the webview sends a final `active: false` report on stop).
const CAPTION_FRESH: Duration = Duration::from_secs(1);

lazy_static::lazy_static! {
    static ref WAITERS: Mutex<Vec<Weak<WaitState>>> = Mutex::new(Vec::new());
    static ref RECENT_CAPTION: Mutex<Option<CaptionSnapshot>> = Mutex::new(None);
}

struct WaitState {
    request_id: Option<String>,
    finished: AtomicBool,
}

/// A subscription registered before synthesis can emit a finish.
pub struct PlaybackWait(Arc<WaitState>);

impl PlaybackWait {
    pub fn new(request_id: Option<&str>) -> Self {
        let state = Arc::new(WaitState {
            request_id: request_id.map(str::to_owned),
            finished: AtomicBool::new(false),
        });
        WAITERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&state));
        Self(state)
    }

    pub async fn wait(self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.0.finished.load(Ordering::Acquire) {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

impl Drop for PlaybackWait {
    fn drop(&mut self) {
        let own = Arc::downgrade(&self.0);
        WAITERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|waiter| !waiter.ptr_eq(&own));
    }
}

/// Record each finish for every active subscriber whose reading it ended.
pub fn signal_finished_for(reading: Option<u64>) {
    let mut waiters = WAITERS.lock().unwrap_or_else(|e| e.into_inner());
    waiters.retain(|waiter| {
        let Some(waiter) = waiter.upgrade() else {
            return false;
        };
        let matches = match waiter.request_id.as_deref() {
            None => true,
            Some(id) => reading.is_some() && crate::speak_requests::reading_for_id(id) == reading,
        };
        if matches {
            waiter.finished.store(true, Ordering::Release);
        }
        true
    });
}

/// The caption snapshot the webview last rendered, plus its report time.
struct CaptionSnapshot {
    at: Instant,
    json: String,
}

impl CaptionSnapshot {
    /// The JSON while the snapshot describes live playback; `None` once stale.
    fn fresh(&self) -> Option<&str> {
        if self.at.elapsed() < CAPTION_FRESH {
            Some(&self.json)
        } else {
            None
        }
    }
}

/// Frontend hook: remember the caption JSON the webview just rendered. Called
/// at most about every 100 ms while a reading plays.
pub fn remember_caption(json: String) {
    let mut recent = RECENT_CAPTION.lock().unwrap();
    *recent = Some(CaptionSnapshot {
        at: Instant::now(),
        json,
    });
}

/// The last reported caption while it is fresh; `None` after the webview
/// stopped reporting. The control server's `GET /status` reads this.
pub fn current_caption() -> Option<String> {
    let recent = RECENT_CAPTION.lock().unwrap();
    recent.as_ref()?.fresh().map(str::to_string)
}

/// Live playback progress the webview reported for the fragment now audible.
/// The JIT pagination gate decides from this when to submit the next fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackProgress {
    /// Pagination fragment the playhead sits in, when the webview reports it.
    pub fragment_index: Option<usize>,
    /// Audible position inside that fragment, in milliseconds.
    pub position_ms: u64,
    /// Full duration of that fragment, in milliseconds; 0 while unknown.
    pub duration_ms: u64,
    /// Playback is paused: the position is frozen and not advancing.
    pub paused: bool,
    /// The player reports live playback rather than an idle webview.
    pub active: bool,
}

/// Parse a caption snapshot into playback progress. A missing, null or
/// non-finite field becomes absent or zero, so an older webview build that
/// sends no `fragment_index` still parses.
fn parse_progress(json: &str) -> Option<PlaybackProgress> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    if !value.is_object() {
        return None;
    }
    let number = |key: &str| -> Option<f64> {
        value
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
    };
    let boolean = |key: &str| -> bool {
        value
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    Some(PlaybackProgress {
        fragment_index: number("fragment_index").map(|value| value.max(0.0) as usize),
        position_ms: number("position_ms").unwrap_or(0.0).max(0.0) as u64,
        duration_ms: number("duration_ms").unwrap_or(0.0).max(0.0) as u64,
        paused: boolean("paused"),
        active: boolean("active"),
    })
}

/// The last reported playback progress while the caption is fresh; `None`
/// after the webview stopped reporting.
pub fn current_progress() -> Option<PlaybackProgress> {
    parse_progress(current_caption().as_deref()?)
}

/// Tauri command: the webview reports its HUD caption snapshot for `GET
/// /status`. The snapshot is stored as sent; the status route serves it
/// verbatim.
#[tauri::command]
pub fn report_playback_caption(caption: serde_json::Value) {
    remember_caption(caption.to_string());
}

/// Playback tests share process-global subscribers and finish signals.
#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn playback_wait_test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unscoped_wait_observes_only_finishes_after_subscription() {
        let _guard = TEST_LOCK.lock().unwrap();
        signal_finished_for(None);
        assert!(!PlaybackWait::new(None).wait(Duration::from_millis(50)).await);
        let waiter = PlaybackWait::new(None);
        signal_finished_for(Some(7));
        assert!(waiter.wait(Duration::from_millis(50)).await);
    }

    #[tokio::test]
    async fn foreign_finish_cannot_overwrite_a_subscribed_readings_finish() {
        let _guard = TEST_LOCK.lock().unwrap();
        let id = "playback-finish-not-overwritten";
        let crate::speak_requests::Registration::Proceed(seq) =
            crate::speak_requests::register(id)
        else {
            panic!("request must register");
        };
        let _ticket = crate::speak_requests::Ticket::new(seq);
        crate::speak_requests::attach_reading(seq, 876543);
        let waiter = PlaybackWait::new(Some(id));
        signal_finished_for(Some(876543));
        signal_finished_for(Some(876544));
        assert!(waiter.wait(Duration::from_millis(100)).await);
    }

    #[tokio::test]
    async fn finishes_before_reading_attaches_do_not_satisfy_request() {
        let _guard = TEST_LOCK.lock().unwrap();
        let id = "playback-finish-before-reading";
        let crate::speak_requests::Registration::Proceed(seq) =
            crate::speak_requests::register(id)
        else {
            panic!("request must register");
        };
        let _ticket = crate::speak_requests::Ticket::new(seq);
        let waiter = PlaybackWait::new(Some(id));
        signal_finished_for(None);
        crate::speak_requests::attach_reading(seq, 876545);
        assert!(!waiter.wait(Duration::from_millis(50)).await);
    }

    #[tokio::test]
    async fn foreign_and_unattributed_finishes_do_not_release_a_reading() {
        let _guard = TEST_LOCK.lock().unwrap();
        let id = "playback-finish-foreign";
        let crate::speak_requests::Registration::Proceed(seq) =
            crate::speak_requests::register(id)
        else {
            panic!("request must register");
        };
        let _ticket = crate::speak_requests::Ticket::new(seq);
        crate::speak_requests::attach_reading(seq, 876546);
        let waiter = PlaybackWait::new(Some(id));
        signal_finished_for(Some(876547));
        signal_finished_for(None);
        assert!(!waiter.wait(Duration::from_millis(50)).await);
    }

    #[test]
    fn dropping_subscribers_releases_their_registry_entries() {
        let _guard = TEST_LOCK.lock().unwrap();
        let waiter = PlaybackWait::new(None);
        let weak = Arc::downgrade(&waiter.0);
        assert!(WAITERS.lock().unwrap().iter().any(|entry| entry.ptr_eq(&weak)));
        drop(waiter);
        assert!(!WAITERS.lock().unwrap().iter().any(|entry| entry.ptr_eq(&weak)));
        assert!(weak.upgrade().is_none());
    }


    /// A caption snapshot describes live playback only inside the freshness
    /// window; the status route falls back to inactive after it goes stale.
    #[test]
    fn caption_snapshot_goes_stale_after_the_freshness_window() {
        let fresh = CaptionSnapshot {
            at: Instant::now(),
            json: r#"{"active":true}"#.to_string(),
        };
        assert_eq!(fresh.fresh(), Some(r#"{"active":true}"#));
        let stale = CaptionSnapshot {
            at: Instant::now() - CAPTION_FRESH - Duration::from_millis(1),
            json: r#"{"active":true}"#.to_string(),
        };
        assert_eq!(stale.fresh(), None);
    }

    /// The JIT gate reads a progress snapshot out of the caption JSON: the
    /// webview sends fractional positions and (on the batch path) can send a
    /// null duration before the audio decodes.
    #[test]
    fn progress_parses_fractional_positions_and_optional_fields() {
        let progress = parse_progress(
            r#"{"active":true,"fragment_index":2,"position_ms":1234.7,"duration_ms":41000.2,"paused":false}"#,
        )
        .expect("captions with a position parse");
        assert_eq!(
            progress,
            PlaybackProgress {
                fragment_index: Some(2),
                position_ms: 1234,
                duration_ms: 41000,
                paused: false,
                active: true,
            }
        );

        let legacy = parse_progress(r#"{"active":true,"position_ms":20,"duration_ms":6000}"#)
            .expect("a webview build without a fragment index still parses");
        assert_eq!(legacy.fragment_index, None);
        assert_eq!(legacy.position_ms, 20);

        let undecoded = parse_progress(
            r#"{"active":true,"fragment_index":0,"position_ms":null,"duration_ms":null,"paused":true}"#,
        )
        .expect("null fields parse as zero");
        assert_eq!(undecoded.position_ms, 0);
        assert_eq!(undecoded.duration_ms, 0);
        assert!(undecoded.paused);
    }

    /// A malformed or non-object snapshot yields no progress, which the JIT
    /// gate treats as a dead caption clock rather than as a position.
    #[test]
    fn progress_rejects_malformed_snapshots() {
        assert_eq!(parse_progress("not json"), None);
        assert_eq!(parse_progress("[]"), None);
        assert_eq!(parse_progress("null"), None);
    }
}
