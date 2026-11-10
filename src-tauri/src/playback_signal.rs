//! Playback-completion signaling for callers that must block until the
//! webview finishes speaking (Hermes' control-server `wait` requests).
//!
//! Windows playback lives in the webview: the store's queue drain
//! (`finishPlayback`) and user stop (`handleStop`) are the only terminal
//! signals, so the frontend emits `playback-finished` there and the Rust side
//! waits on it here. A generation timestamp ignores finishes that raced with
//! the requester's own synthesis (e.g. a concurrent reader stopping early).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Freshness window for a reported caption snapshot. The webview reports at
/// about 10 Hz while playback runs, so an older snapshot means playback
/// stopped (the webview sends a final `active: false` report on stop).
const CAPTION_FRESH: Duration = Duration::from_secs(1);

lazy_static::lazy_static! {
    static ref LAST_FINISH_MS: AtomicU64 = AtomicU64::new(0);
    static ref RECENT_CAPTION: Mutex<Option<CaptionSnapshot>> = Mutex::new(None);
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

#[cfg(test)]
mod tests {
    use super::*;

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
