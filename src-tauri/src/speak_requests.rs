//! Request-scoped cancellation for control-server speaks (Hermes barge-in).
//!
//! A caller names a `/speak` with a `request_id` and cancels exactly that
//! request with `POST /stop {"request_id": ...}`. A global stop only reaches
//! whatever is audible at the moment it lands, which loses three races:
//!
//! - **Stop before the speak registers** (the caller's process is still
//!   booting): the id is tombstoned, and the speak returns cancelled without
//!   synthesizing when it arrives.
//! - **Stop while queued or synthesizing**: the request's [`crate::AbortScope`]
//!   reads cancelled, so the synthesis is discarded and nothing it produced is
//!   emitted to the webview.
//! - **Stop arriving late**: a request that already finished ignores it, so a
//!   delayed stop can never cut a later request's playback.
//!
//! Playback is stopped only while the cancelled request still owns the
//! speakers ([`PlaybackOwner`]): it was the last reading to emit audio. Any
//! newer reading takes ownership at its own first audible emit, and the stale
//! stop leaves it alone. A reading that has only started synthesizing owns
//! nothing, so stopping it never cuts the previous reading.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a finished or cancelled id is remembered. Covers the slowest
/// caller start-up (node launch, stdin read) by a wide margin; past it, a
/// stop for that id is treated as unknown and tombstoned again.
const RETAIN_FOR: Duration = Duration::from_secs(15 * 60);
/// Bound on remembered ids; the oldest settled ids go first. Live requests are
/// never evicted.
const MAX_SETTLED: usize = 1024;
/// Longest accepted request id.
const MAX_ID_LEN: usize = 128;

/// Sequence 0 means "no request": readings outside the control server.
pub const NO_REQUEST: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Live,
    Finished,
    Cancelled,
}

#[derive(Debug)]
struct Entry {
    seq: u64,
    phase: Phase,
    /// The cancel also stopped playback this request owned; the webview's
    /// `playback-finished` for that stop is still in flight.
    stopped_playback: bool,
    touched: Instant,
}

/// Result of registering a `/speak` under an id.
#[derive(Debug, PartialEq, Eq)]
pub enum Registration {
    /// New request; carries its sequence number.
    Proceed(u64),
    /// A stop for this id arrived first: do not speak.
    Cancelled,
    /// The id is already in use by another speak.
    Duplicate,
}

/// Result of a scoped stop.
#[derive(Debug, PartialEq, Eq)]
pub enum StopOutcome {
    /// No speak with this id has registered yet; it will be refused on arrival.
    Preempted,
    /// The live request was cancelled; `stopped_playback` says whether it
    /// still owned the speakers, so its playback was stopped too.
    Cancelled { seq: u64, stopped_playback: bool },
    /// The request already finished; nothing to do.
    AlreadyFinished,
    /// The request was already cancelled; nothing to do.
    AlreadyCancelled,
}

impl StopOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            StopOutcome::Preempted => "preempted",
            StopOutcome::Cancelled { .. } => "cancelled",
            StopOutcome::AlreadyFinished => "already_finished",
            StopOutcome::AlreadyCancelled => "already_cancelled",
        }
    }
}

/// Validate a caller-supplied request id: 1..=128 chars of `[A-Za-z0-9._:-]`.
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > MAX_ID_LEN {
        return Err(format!("request_id must be 1-{} characters", MAX_ID_LEN));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
    {
        return Err("request_id may only contain A-Z a-z 0-9 . _ : -".to_string());
    }
    Ok(())
}

/// Registry of request ids and their phase.
#[derive(Default)]
pub struct Registry {
    next_seq: u64,
    by_id: HashMap<String, Entry>,
    cancelled: HashSet<u64>,
}

impl Registry {
    pub fn register(&mut self, id: &str, now: Instant) -> Registration {
        self.prune(now);
        if let Some(entry) = self.by_id.get_mut(id) {
            return match entry.phase {
                Phase::Cancelled => {
                    entry.touched = now;
                    Registration::Cancelled
                }
                Phase::Live | Phase::Finished => Registration::Duplicate,
            };
        }
        let seq = self.allocate_seq();
        self.by_id.insert(
            id.to_string(),
            Entry {
                seq,
                phase: Phase::Live,
                stopped_playback: false,
                touched: now,
            },
        );
        Registration::Proceed(seq)
    }

    /// The sequence of `id` while its request is live. Unknown, finished and
    /// cancelled ids return `None`.
    pub fn live_seq(&self, id: &str) -> Option<u64> {
        self.by_id
            .get(id)
            .filter(|entry| entry.phase == Phase::Live)
            .map(|entry| entry.seq)
    }

    /// Cancel `id`. `owns_playback(seq)` reports whether that live request
    /// still owns the speakers; the answer is recorded with the cancellation
    /// in one step, so a waiter never sees "cancelled" without it.
    pub fn cancel(
        &mut self,
        id: &str,
        now: Instant,
        owns_playback: impl FnOnce(u64) -> bool,
    ) -> StopOutcome {
        self.prune(now);
        match self.by_id.get_mut(id) {
            None => {
                let seq = self.allocate_seq();
                self.cancelled.insert(seq);
                self.by_id.insert(
                    id.to_string(),
                    Entry {
                        seq,
                        phase: Phase::Cancelled,
                        stopped_playback: false,
                        touched: now,
                    },
                );
                StopOutcome::Preempted
            }
            Some(entry) => match entry.phase {
                Phase::Live => {
                    let stopped_playback = owns_playback(entry.seq);
                    entry.phase = Phase::Cancelled;
                    entry.stopped_playback = stopped_playback;
                    entry.touched = now;
                    self.cancelled.insert(entry.seq);
                    StopOutcome::Cancelled {
                        seq: entry.seq,
                        stopped_playback,
                    }
                }
                Phase::Finished => StopOutcome::AlreadyFinished,
                Phase::Cancelled => StopOutcome::AlreadyCancelled,
            },
        }
    }

    /// Mark a live request finished. A cancelled one stays cancelled.
    pub fn finish(&mut self, seq: u64, now: Instant) {
        if let Some(entry) = self.by_id.values_mut().find(|entry| entry.seq == seq) {
            if entry.phase == Phase::Live {
                entry.phase = Phase::Finished;
            }
            entry.touched = now;
        }
    }

    pub fn is_cancelled(&self, seq: u64) -> bool {
        seq != NO_REQUEST && self.cancelled.contains(&seq)
    }

    /// Whether `seq`'s cancellation also stopped its playback.
    pub fn stopped_playback(&self, seq: u64) -> bool {
        self.by_id
            .values()
            .any(|entry| entry.seq == seq && entry.stopped_playback)
    }

    fn allocate_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }

    fn prune(&mut self, now: Instant) {
        let expired: Vec<String> = self
            .by_id
            .iter()
            .filter(|(_, e)| e.phase != Phase::Live && now.duration_since(e.touched) > RETAIN_FOR)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.forget(&id);
        }
        let settled = self
            .by_id
            .values()
            .filter(|e| e.phase != Phase::Live)
            .count();
        if settled > MAX_SETTLED {
            let mut oldest: Vec<(Instant, String)> = self
                .by_id
                .iter()
                .filter(|(_, e)| e.phase != Phase::Live)
                .map(|(id, e)| (e.touched, id.clone()))
                .collect();
            oldest.sort();
            for (_, id) in oldest.into_iter().take(settled - MAX_SETTLED) {
                self.forget(&id);
            }
        }
    }

    fn forget(&mut self, id: &str) {
        if let Some(entry) = self.by_id.remove(id) {
            self.cancelled.remove(&entry.seq);
        }
    }
}

/// Which reading currently owns the speakers: the last one to emit audio.
/// Emits and scoped stops serialize on this lock, so a stop
/// emitted for request R always reaches the webview before any later
/// reading's start or audio, never after it.
pub struct PlaybackOwner {
    current: Mutex<u64>,
}

impl PlaybackOwner {
    pub const fn new() -> Self {
        Self {
            current: Mutex::new(NO_REQUEST),
        }
    }

    /// Take ownership for `request` and run `emit` under the lock, unless
    /// `aborted()` already reads true (then nothing is emitted).
    pub fn claim(&self, request: u64, aborted: impl FnOnce() -> bool, emit: impl FnOnce()) -> bool {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if aborted() {
            return false;
        }
        *current = request;
        emit();
        true
    }

    /// Run `f` with the current owner while holding the owner lock, so no
    /// claim (and no emit) can interleave with it.
    pub fn with_current<R>(&self, f: impl FnOnce(u64) -> R) -> R {
        let current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        f(*current)
    }

    /// Run `stop` only while `request` still owns the speakers. Production
    /// stops go through [`cancel`], which decides and stops in one critical
    /// section; this is the owner rule on its own, for tests.
    #[cfg(test)]
    pub fn stop_if_owner(&self, request: u64, stop: impl FnOnce()) -> bool {
        let current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if request == NO_REQUEST || *current != request {
            return false;
        }
        stop();
        true
    }
}

/// Re-arms the webview's streamed-playback gate exactly once per reading, at
/// the reading's first audible chunk.
///
/// The webview drops every `audio-stream-chunk` while its `_streamStopped`
/// gate is set, and only a `synthesis-state-change(true)` clears it. The
/// reading's start signal clears the gate at synthesis start, but a scoped
/// stop for the PREVIOUS owner can land after that and re-set it — stopping
/// the previous reading is exactly what it should do. The innocent next
/// reading must not be silenced by it: at its first audible chunk — claimed
/// under the speaker-ownership lock, so it serializes with that stop — the
/// gate is re-armed again, immediately before the chunk on the wire.
///
/// Once per reading keeps a stop that lands mid-stream authoritative: no
/// later chunk resurrects stopped playback.
pub struct StreamRearm(AtomicBool);

impl StreamRearm {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// Run `rearm` on the first call only; later calls are no-ops. Returns
    /// whether this call rearmed.
    pub fn rearm(&self, rearm: impl FnOnce()) -> bool {
        if self.0.swap(true, Ordering::Relaxed) {
            return false;
        }
        rearm();
        true
    }
}

lazy_static::lazy_static! {
    static ref REGISTRY: Mutex<Registry> = Mutex::new(Registry::default());
}

static OWNER: PlaybackOwner = PlaybackOwner::new();

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Register a speak under `id` (see [`Registry::register`]).
pub fn register(id: &str) -> Registration {
    registry().register(id, Instant::now())
}

/// Cancel `id`; when the request is live and still owns playback, run `stop`.
///
/// Everything happens under the owner lock (lock order owner → registry, the
/// same as an emit's claim): an emit for this request either ran before (its
/// audio is then stopped) or runs after and sees the cancelled scope, and no
/// newer reading can claim the speakers between the owner check and `stop`.
pub fn cancel(id: &str, stop: impl FnOnce()) -> StopOutcome {
    OWNER.with_current(|owner| {
        let outcome = registry().cancel(id, Instant::now(), |seq| owner == seq);
        if let StopOutcome::Cancelled {
            stopped_playback: true,
            ..
        } = outcome
        {
            stop();
        }
        outcome
    })
}

/// Whether `id` is registered and neither finished nor cancelled.
#[cfg(test)]
pub fn is_live(id: &str) -> bool {
    registry().live_seq(id).is_some()
}

pub fn is_cancelled(seq: u64) -> bool {
    seq != NO_REQUEST && registry().is_cancelled(seq)
}

/// Claim the speakers for `request` and emit under the owner lock.
pub fn claim_playback(request: u64, aborted: impl FnOnce() -> bool, emit: impl FnOnce()) -> bool {
    OWNER.claim(request, aborted, emit)
}

/// Audio that belongs to no request (history replay, cached replay) is about
/// to play: take ownership away from any request, so a late scoped stop for
/// an older request cannot cut it. Call before emitting the audio.
pub fn claim_unscoped_playback() {
    OWNER.claim(NO_REQUEST, || false, || {});
}

/// How long a scoped playback control (`/pause`, `/resume`, `/seek`) waits for
/// its live request to become audible. A control that arrives while the
/// reading still synthesizes applies as soon as its first chunk is audible;
/// past this bound it is answered as not playing.
const CONTROL_OWNER_WAIT: Duration = Duration::from_secs(10);
/// How often a waiting control re-checks ownership.
const CONTROL_OWNER_POLL: Duration = Duration::from_millis(25);

/// Result of one ownership attempt inside [`dispatch_to_owner_with`].
enum OwnerCheck {
    /// The request owns playback; `run` has executed under the owner lock.
    Ran,
    /// The request is live but another reading owns the speakers.
    NotOwner,
    /// Unknown, finished or cancelled: no wait can make this request audible.
    NotLive,
}

/// Run `run` for `id` while its live request owns the speakers, waiting for a
/// reading that is still starting to become audible.
///
/// The control path shares the scoped stop's critical section: the owner lock
/// stays held while `run` emits, so a control for request R can never land
/// after a newer reading claims the speakers, and a claim never interleaves
/// with it. A request that is unknown, finished or cancelled returns `false`
/// at once. A live request that never becomes the owner (queued behind
/// another reading) returns `false` after [`CONTROL_OWNER_WAIT`].
///
/// Returns whether `run` executed. Callers map `false` to a `not_playing`
/// answer and must not emit anything else on that path.
pub fn dispatch_to_owner(id: &str, run: impl FnMut(u64)) -> bool {
    dispatch_to_owner_with(
        id,
        &OWNER,
        &REGISTRY,
        CONTROL_OWNER_WAIT,
        CONTROL_OWNER_POLL,
        run,
    )
}

/// [`dispatch_to_owner`] on injected state and timings, so tests use local
/// instances instead of the process-global owner and registry.
fn dispatch_to_owner_with(
    id: &str,
    owner: &PlaybackOwner,
    registry: &Mutex<Registry>,
    timeout: Duration,
    poll: Duration,
    mut run: impl FnMut(u64),
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match try_run_if_owner(id, owner, registry, &mut run) {
            OwnerCheck::Ran => return true,
            OwnerCheck::NotLive => return false,
            OwnerCheck::NotOwner => {}
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(poll);
    }
}

/// One ownership attempt: check the request is live, compare the speaker
/// owner, then run under the owner lock (lock order owner -> registry, the
/// same as [`cancel`]). The registry guard drops before `run`, so the emit
/// never holds the registry lock.
fn try_run_if_owner(
    id: &str,
    owner: &PlaybackOwner,
    registry: &Mutex<Registry>,
    run: &mut impl FnMut(u64),
) -> OwnerCheck {
    owner.with_current(|current| {
        let seq = registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .live_seq(id);
        match seq {
            None => OwnerCheck::NotLive,
            Some(seq) if seq == current => {
                run(seq);
                OwnerCheck::Ran
            }
            Some(_) => OwnerCheck::NotOwner,
        }
    })
}

/// Holds a registered request live; dropping it marks the request finished.
pub struct Ticket {
    seq: u64,
}

impl Ticket {
    pub fn new(seq: u64) -> Self {
        Self { seq }
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn is_cancelled(&self) -> bool {
        is_cancelled(self.seq)
    }

    /// Whether this request's cancellation stopped its playback (the webview
    /// will answer with a `playback-finished` that belongs to this request).
    pub fn stopped_playback(&self) -> bool {
        registry().stopped_playback(self.seq)
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        registry().finish(self.seq, Instant::now());
    }
}

/// How often a pending future re-checks for cancellation.
const CANCEL_POLL: Duration = Duration::from_millis(25);

/// Drive `fut` to completion unless request `seq` is cancelled first; `None`
/// means cancelled. The future is kept (not recreated) across polls, so a lock
/// future keeps its place in the queue.
pub async fn until_cancelled<F: std::future::Future>(seq: u64, fut: F) -> Option<F::Output> {
    until_cancelled_or_aborted(seq, || false, fut).await
}

/// Drive `fut` to completion unless request `seq` is cancelled or `aborted`
/// reports true; `None` means stopped. An unscoped reading (hotkey, tray, Play)
/// carries no request id, so `aborted` is how a global abort reaches it while it
/// waits. The future is kept (not recreated) across polls, so a lock future
/// keeps its place in the queue.
pub async fn until_cancelled_or_aborted<F, P>(seq: u64, aborted: P, fut: F) -> Option<F::Output>
where
    F: std::future::Future,
    P: Fn() -> bool,
{
    let mut fut = Box::pin(fut);
    loop {
        if is_cancelled(seq) || aborted() {
            return None;
        }
        if let Ok(output) = tokio::time::timeout(CANCEL_POLL, &mut fut).await {
            return Some(output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn stop_before_register_preempts_the_speak() {
        let mut reg = Registry::default();
        let now = t0();
        assert_eq!(reg.cancel("r1", now, |_| false), StopOutcome::Preempted);
        assert_eq!(reg.register("r1", now), Registration::Cancelled);
        // A second late registration of the same id stays refused.
        assert_eq!(reg.register("r1", now), Registration::Cancelled);
    }

    #[test]
    fn stop_while_live_cancels_only_that_request() {
        let mut reg = Registry::default();
        let now = t0();
        let Registration::Proceed(a) = reg.register("a", now) else {
            panic!("a must register");
        };
        let Registration::Proceed(b) = reg.register("b", now) else {
            panic!("b must register");
        };
        assert_ne!(a, b);
        assert_eq!(
            reg.cancel("a", now, |_| false),
            StopOutcome::Cancelled {
                seq: a,
                stopped_playback: false
            }
        );
        assert!(reg.is_cancelled(a));
        assert!(!reg.is_cancelled(b), "a stop for a must not reach b");
        assert_eq!(
            reg.cancel("a", now, |_| false),
            StopOutcome::AlreadyCancelled
        );
    }

    #[test]
    fn cancel_records_whether_it_stopped_the_requests_playback() {
        let mut reg = Registry::default();
        let now = t0();
        let Registration::Proceed(a) = reg.register("a", now) else {
            panic!("a must register");
        };
        let Registration::Proceed(b) = reg.register("b", now) else {
            panic!("b must register");
        };
        assert_eq!(
            reg.cancel("a", now, |seq| seq == a),
            StopOutcome::Cancelled {
                seq: a,
                stopped_playback: true
            }
        );
        assert!(reg.stopped_playback(a));
        // b does not own the speakers (a does): its cancel stops nothing.
        assert_eq!(
            reg.cancel("b", now, |seq| seq == a),
            StopOutcome::Cancelled {
                seq: b,
                stopped_playback: false
            }
        );
        assert!(!reg.stopped_playback(b));
    }

    #[test]
    fn late_stop_after_finish_is_a_no_op() {
        let mut reg = Registry::default();
        let now = t0();
        let Registration::Proceed(a) = reg.register("a", now) else {
            panic!("a must register");
        };
        reg.finish(a, now);
        assert_eq!(
            reg.cancel("a", now, |_| false),
            StopOutcome::AlreadyFinished
        );
        assert!(!reg.is_cancelled(a));
        // A later request is unaffected by the stale stop.
        let Registration::Proceed(b) = reg.register("b", now) else {
            panic!("b must register");
        };
        assert!(!reg.is_cancelled(b));
    }

    #[test]
    fn finish_keeps_a_cancelled_request_cancelled() {
        let mut reg = Registry::default();
        let now = t0();
        let Registration::Proceed(a) = reg.register("a", now) else {
            panic!("a must register");
        };
        reg.cancel("a", now, |_| false);
        reg.finish(a, now);
        assert!(reg.is_cancelled(a));
        assert_eq!(
            reg.cancel("a", now, |_| false),
            StopOutcome::AlreadyCancelled
        );
    }

    #[test]
    fn reused_id_is_refused_as_duplicate() {
        let mut reg = Registry::default();
        let now = t0();
        assert!(matches!(reg.register("a", now), Registration::Proceed(_)));
        assert_eq!(reg.register("a", now), Registration::Duplicate);
    }

    #[test]
    fn no_request_is_never_cancelled() {
        let mut reg = Registry::default();
        let now = t0();
        reg.cancel("x", now, |_| false);
        assert!(!reg.is_cancelled(NO_REQUEST));
    }

    #[test]
    fn settled_ids_expire_but_live_ones_never_do() {
        let mut reg = Registry::default();
        let start = t0();
        reg.cancel("tomb", start, |_| false);
        let Registration::Proceed(live) = reg.register("live", start) else {
            panic!("live must register");
        };
        let later = start + RETAIN_FOR + Duration::from_secs(1);
        // The tombstone expired: the id registers fresh.
        assert!(matches!(
            reg.register("tomb", later),
            Registration::Proceed(_)
        ));
        // The live request survived pruning and can still be cancelled.
        assert_eq!(
            reg.cancel("live", later, |_| false),
            StopOutcome::Cancelled {
                seq: live,
                stopped_playback: false
            }
        );
    }

    #[test]
    fn settled_ids_are_bounded() {
        let mut reg = Registry::default();
        let now = t0();
        for i in 0..(MAX_SETTLED + 50) {
            reg.cancel(
                &format!("t{i}"),
                now + Duration::from_millis(i as u64),
                |_| false,
            );
        }
        reg.register("trigger-prune", now + Duration::from_secs(10));
        assert!(reg.by_id.len() <= MAX_SETTLED + 1);
        assert!(reg.cancelled.len() <= MAX_SETTLED);
    }

    #[test]
    fn validate_id_accepts_safe_ids_and_rejects_others() {
        assert!(validate_id("hermes-0123abcd").is_ok());
        assert!(validate_id("a.b_c:d-E9").is_ok());
        assert!(validate_id("").is_err());
        assert!(validate_id(&"x".repeat(MAX_ID_LEN + 1)).is_err());
        assert!(validate_id("has space").is_err());
        assert!(validate_id("quote\"").is_err());
    }

    #[test]
    fn owner_stop_reaches_only_the_current_owner() {
        let owner = PlaybackOwner::new();
        let stops = AtomicUsize::new(0);
        assert!(owner.claim(7, || false, || {}));
        // A stop for a different request does nothing.
        assert!(!owner.stop_if_owner(8, || {
            stops.fetch_add(1, Ordering::SeqCst);
        }));
        // Request 7 still owns playback: its stop runs.
        assert!(owner.stop_if_owner(7, || {
            stops.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn newer_reading_takes_ownership_so_a_stale_stop_spares_it() {
        let owner = PlaybackOwner::new();
        owner.claim(7, || false, || {});
        // A later reading (request 9, or an unscoped one) starts.
        owner.claim(NO_REQUEST, || false, || {});
        let mut stopped = false;
        assert!(!owner.stop_if_owner(7, || stopped = true));
        assert!(!stopped, "a stale stop must not cut a newer reading");
        assert!(!owner.stop_if_owner(NO_REQUEST, || stopped = true));
        assert!(!stopped, "request 0 is never a scoped stop target");
    }

    #[test]
    fn aborted_claim_emits_nothing_and_keeps_the_owner() {
        let owner = PlaybackOwner::new();
        owner.claim(3, || false, || {});
        let mut emitted = false;
        assert!(!owner.claim(4, || true, || emitted = true));
        assert!(!emitted);
        let mut stopped = false;
        assert!(owner.stop_if_owner(3, || stopped = true));
        assert!(stopped);
    }

    /// `cancel`'s production shape (decide and stop under the owner lock)
    /// against LOCAL state, so these sequence tests never race the
    /// process-global owner the other suites use.
    fn cancel_on(
        owner: &PlaybackOwner,
        reg: &Mutex<Registry>,
        id: &str,
        stop: impl FnOnce(),
    ) -> StopOutcome {
        owner.with_current(|current| {
            let outcome = reg
                .lock()
                .unwrap()
                .cancel(id, Instant::now(), |seq| current == seq);
            if let StopOutcome::Cancelled {
                stopped_playback: true,
                ..
            } = outcome
            {
                stop();
            }
            outcome
        })
    }

    #[test]
    fn reading_start_takes_no_ownership_until_audio_is_audible() {
        // Production sequence: the previous reading's audio is audible, then
        // the next reading starts synthesizing (start signal only — the
        // guard does not claim). A stop for the reading that owns nothing
        // audible must leave the speakers alone; a stop for the audible one
        // must reach them.
        let owner = PlaybackOwner::new();
        let reg = Mutex::new(Registry::default());
        let now = Instant::now();
        let Registration::Proceed(prev) = reg.lock().unwrap().register("start-no-owner-prev", now)
        else {
            panic!("prev must register");
        };
        let Registration::Proceed(cur) = reg.lock().unwrap().register("start-no-owner-cur", now)
        else {
            panic!("cur must register");
        };
        // prev's first audible emit.
        owner.claim(prev, || false, || {});
        // cur starts synthesizing: no claim happens here (see SynthesisGuard).
        assert_eq!(
            cancel_on(&owner, &reg, "start-no-owner-cur", || panic!(
                "a request with no audible audio owns nothing"
            )),
            StopOutcome::Cancelled {
                seq: cur,
                stopped_playback: false
            }
        );
        let mut stopped = false;
        assert_eq!(
            cancel_on(&owner, &reg, "start-no-owner-prev", || stopped = true),
            StopOutcome::Cancelled {
                seq: prev,
                stopped_playback: true
            }
        );
        assert!(stopped);
    }

    #[test]
    fn stream_rearm_fires_once_at_first_audible_chunk() {
        let rearm = StreamRearm::new();
        let mut events: Vec<&'static str> = Vec::new();
        assert!(rearm.rearm(|| events.push("rearm")));
        assert!(!rearm.rearm(|| events.push("rearm")));
        assert_eq!(events, ["rearm"]);
    }

    /// A scoped stop for the previous owner legitimately re-sets the webview's
    /// stream gate (`_streamStopped`) after the next reading's start signal
    /// already cleared it. That next reading is innocent: its first audible
    /// chunk claims the speakers and re-arms the gate right before the chunk
    /// on the wire, so the webview's drop-everything gate does not silence it.
    #[test]
    fn first_audible_chunk_rearm_heals_a_scoped_stop_of_the_previous_owner() {
        let owner = PlaybackOwner::new();
        let reg = Mutex::new(Registry::default());
        let now = Instant::now();
        let Registration::Proceed(prev) = reg.lock().unwrap().register("rearm-heal-prev", now)
        else {
            panic!("prev must register");
        };
        let Registration::Proceed(cur) = reg.lock().unwrap().register("rearm-heal-cur", now) else {
            panic!("cur must register");
        };
        let events: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

        // prev streams and is audible.
        owner.claim(
            prev,
            || false,
            || {
                events.lock().unwrap().push("prev-chunk");
            },
        );
        // cur starts: the guard's start signal only (no ownership change).
        events.lock().unwrap().push("cur-start");
        // Barge-in: stop prev while it is audible.
        assert_eq!(
            cancel_on(&owner, &reg, "rearm-heal-prev", || events
                .lock()
                .unwrap()
                .push("stop")),
            StopOutcome::Cancelled {
                seq: prev,
                stopped_playback: true
            }
        );
        // cur's first audible chunk: claim, re-arm, chunk — in wire order.
        let rearm = StreamRearm::new();
        owner.claim(
            cur,
            || false,
            || {
                rearm.rearm(|| events.lock().unwrap().push("cur-rearm"));
                events.lock().unwrap().push("cur-chunk");
            },
        );

        assert_eq!(
            *events.lock().unwrap(),
            ["prev-chunk", "cur-start", "stop", "cur-rearm", "cur-chunk"]
        );
    }

    /// A cancelled reading claims nothing, so it rearms nothing: the webview's
    /// gate stays as the stop left it, and no event resurrects stopped
    /// playback.
    #[test]
    fn cancelled_reading_rearms_nothing_and_emits_nothing() {
        let owner = PlaybackOwner::new();
        let reg = Mutex::new(Registry::default());
        let Registration::Proceed(seq) = reg
            .lock()
            .unwrap()
            .register("rearm-cancelled-reading", Instant::now())
        else {
            panic!("must register");
        };
        cancel_on(&owner, &reg, "rearm-cancelled-reading", || {
            panic!("nothing audible yet")
        });
        let rearm = StreamRearm::new();
        let mut emitted = false;
        // The scope reads cancelled: the emit sites' claim emits nothing.
        owner.claim(
            seq,
            || true,
            || {
                rearm.rearm(|| emitted = true);
                emitted = true;
            },
        );
        assert!(!emitted);
    }

    /// The cancel-then-check order closes the emit race: once the stop has
    /// checked ownership, a concurrent emit for the cancelled request sees its
    /// scope cancelled and emits nothing, so no audio can follow the stop.
    #[test]
    fn concurrent_emit_never_lands_after_the_scoped_stop() {
        for _ in 0..200 {
            let owner = Arc::new(PlaybackOwner::new());
            let reg = Arc::new(Mutex::new(Registry::default()));
            let Registration::Proceed(seq) = reg.lock().unwrap().register("r", Instant::now())
            else {
                panic!("r must register");
            };
            let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));

            let emitter = {
                let (owner, reg, events) = (owner.clone(), reg.clone(), events.clone());
                std::thread::spawn(move || {
                    owner.claim(
                        seq,
                        || reg.lock().unwrap().is_cancelled(seq),
                        || events.lock().unwrap().push("audio"),
                    );
                })
            };
            let stopper = {
                let (owner, reg, events) = (owner.clone(), reg.clone(), events.clone());
                std::thread::spawn(move || {
                    // The same shape as `cancel`: decide and stop under the
                    // owner lock.
                    owner.with_current(|current| {
                        let outcome = reg
                            .lock()
                            .unwrap()
                            .cancel("r", Instant::now(), |seq| current == seq);
                        if let StopOutcome::Cancelled {
                            stopped_playback: true,
                            ..
                        } = outcome
                        {
                            events.lock().unwrap().push("stop");
                        }
                    });
                })
            };
            emitter.join().unwrap();
            stopper.join().unwrap();
            let events = events.lock().unwrap().clone();
            assert!(
                events.is_empty() || events == ["audio", "stop"],
                "audio emitted after its stop: {:?}",
                events
            );
        }
    }

    #[tokio::test]
    async fn until_cancelled_returns_none_once_cancelled() {
        let id = "until-cancelled-test";
        let Registration::Proceed(seq) = register(id) else {
            panic!("must register");
        };
        let pending = std::future::pending::<()>();
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            cancel(id, || {});
        });
        let started = Instant::now();
        assert_eq!(until_cancelled(seq, pending).await, None);
        assert!(started.elapsed() < Duration::from_secs(2));
        canceller.await.unwrap();
    }

    #[tokio::test]
    async fn until_cancelled_passes_through_completion() {
        let Registration::Proceed(seq) = register("until-cancelled-ok") else {
            panic!("must register");
        };
        assert_eq!(until_cancelled(seq, async { 5 }).await, Some(5));
    }

    #[tokio::test]
    async fn until_cancelled_or_aborted_returns_none_before_polling_when_aborted() {
        let started = Instant::now();
        assert_eq!(
            until_cancelled_or_aborted(NO_REQUEST, || true, std::future::pending::<()>()).await,
            None
        );
        assert!(started.elapsed() < Duration::from_millis(20));
    }

    #[tokio::test]
    async fn until_cancelled_or_aborted_stops_when_abort_lands_while_waiting() {
        let aborted = Arc::new(AtomicBool::new(false));
        let flip = aborted.clone();
        let flipper = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            flip.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        assert_eq!(
            until_cancelled_or_aborted(
                NO_REQUEST,
                || aborted.load(Ordering::Relaxed),
                std::future::pending::<()>(),
            )
            .await,
            None
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        flipper.await.unwrap();
    }

    #[tokio::test]
    async fn until_cancelled_or_aborted_passes_through_completion() {
        assert_eq!(
            until_cancelled_or_aborted(NO_REQUEST, || false, async { 7 }).await,
            Some(7)
        );
    }

    #[test]
    fn ticket_drop_finishes_the_request() {
        let id = "ticket-drop-test";
        let Registration::Proceed(seq) = register(id) else {
            panic!("must register");
        };
        drop(Ticket::new(seq));
        assert_eq!(
            cancel(id, || panic!("finished request must not stop")),
            StopOutcome::AlreadyFinished
        );
    }

    // ---- owner-gated dispatch (/pause, /resume, /seek) ----
    //
    // Local owner and registry instances keep these tests deterministic; the
    // process-global ones serve the app and the other suites.

    fn local_state() -> (PlaybackOwner, Mutex<Registry>) {
        (PlaybackOwner::new(), Mutex::new(Registry::default()))
    }

    fn register_live(reg: &Mutex<Registry>, id: &str) -> u64 {
        match reg.lock().unwrap().register(id, Instant::now()) {
            Registration::Proceed(seq) => seq,
            _ => panic!("{id} must register"),
        }
    }

    #[test]
    fn live_seq_reports_only_live_requests() {
        let mut reg = Registry::default();
        let now = Instant::now();
        let Registration::Proceed(live) = reg.register("seq-live", now) else {
            panic!("live must register");
        };
        assert_eq!(reg.live_seq("seq-live"), Some(live));
        assert_eq!(reg.live_seq("seq-unknown"), None);

        let Registration::Proceed(done) = reg.register("seq-done", now) else {
            panic!("done must register");
        };
        reg.finish(done, now);
        assert_eq!(reg.live_seq("seq-done"), None);

        assert!(matches!(
            reg.register("seq-cancelled", now),
            Registration::Proceed(_)
        ));
        reg.cancel("seq-cancelled", now, |_| false);
        assert_eq!(reg.live_seq("seq-cancelled"), None);
    }

    #[test]
    fn dispatch_runs_for_the_live_speaker_owner_only() {
        let (owner, reg) = local_state();
        let seq = register_live(&reg, "control-owner");
        owner.claim(seq, || false, || {});

        let mut seen = Vec::new();
        assert!(dispatch_to_owner_with(
            "control-owner",
            &owner,
            &reg,
            Duration::from_millis(200),
            Duration::from_millis(5),
            |seq| seen.push(seq),
        ));
        assert_eq!(seen, [seq]);

        // A newer unscoped reading took the speakers: the control is refused.
        owner.claim(NO_REQUEST, || false, || {});
        assert!(!dispatch_to_owner_with(
            "control-owner",
            &owner,
            &reg,
            Duration::from_millis(20),
            Duration::from_millis(5),
            |_| panic!("an owner mismatch must not emit"),
        ));
    }

    #[test]
    fn dispatch_waits_for_a_starting_reading_to_become_audible() {
        let owner = Arc::new(PlaybackOwner::new());
        let reg = Arc::new(Mutex::new(Registry::default()));
        let seq = register_live(&reg, "control-starting");
        let claimer = {
            let (owner, reg) = (Arc::clone(&owner), Arc::clone(&reg));
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(40));
                // The reading's first audible chunk claims the speakers.
                assert_eq!(reg.lock().unwrap().live_seq("control-starting"), Some(seq));
                owner.claim(seq, || false, || {});
            })
        };

        let mut seen = Vec::new();
        assert!(dispatch_to_owner_with(
            "control-starting",
            &owner,
            &reg,
            Duration::from_secs(2),
            Duration::from_millis(5),
            |seq| seen.push(seq),
        ));
        claimer.join().unwrap();
        assert_eq!(seen, [seq]);
    }

    #[test]
    fn dispatch_refuses_unknown_finished_and_cancelled_requests_at_once() {
        let (owner, reg) = local_state();
        let timeout = Duration::from_secs(2);
        let poll = Duration::from_millis(5);

        let started = Instant::now();
        assert!(!dispatch_to_owner_with(
            "control-unknown",
            &owner,
            &reg,
            timeout,
            poll,
            |_| panic!("an unknown request must not emit"),
        ));
        assert!(started.elapsed() < Duration::from_millis(500));

        let finished = register_live(&reg, "control-finished");
        reg.lock().unwrap().finish(finished, Instant::now());
        let started = Instant::now();
        assert!(!dispatch_to_owner_with(
            "control-finished",
            &owner,
            &reg,
            timeout,
            poll,
            |_| panic!("a finished request must not emit"),
        ));
        assert!(started.elapsed() < Duration::from_millis(500));

        let cancelled = register_live(&reg, "control-cancelled");
        assert_eq!(
            reg.lock()
                .unwrap()
                .cancel("control-cancelled", Instant::now(), |_| false),
            StopOutcome::Cancelled {
                seq: cancelled,
                stopped_playback: false
            }
        );
        assert!(!dispatch_to_owner_with(
            "control-cancelled",
            &owner,
            &reg,
            timeout,
            poll,
            |_| panic!("a cancelled request must not emit"),
        ));
    }

    #[test]
    fn dispatch_times_out_when_the_reading_never_owns_the_speakers() {
        let (owner, reg) = local_state();
        let _queued = register_live(&reg, "control-queued");
        let other = register_live(&reg, "control-other");
        owner.claim(other, || false, || {});

        let started = Instant::now();
        assert!(!dispatch_to_owner_with(
            "control-queued",
            &owner,
            &reg,
            Duration::from_millis(60),
            Duration::from_millis(5),
            |_| panic!("a queued reading owns nothing"),
        ));
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(60),
            "gave up early: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "waited too long: {elapsed:?}"
        );
    }

    /// The emit runs inside the owner-lock critical section: a control for the
    /// audible reading cannot interleave with a newer reading's claim.
    #[test]
    fn dispatch_emits_while_holding_the_owner_lock() {
        use std::sync::mpsc;

        let owner = Arc::new(PlaybackOwner::new());
        let reg = Arc::new(Mutex::new(Registry::default()));
        let seq = register_live(&reg, "control-lock");
        owner.claim(seq, || false, || {});

        let (run_started_tx, run_started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (claimed_tx, claimed_rx) = mpsc::channel::<()>();
        let dispatcher = {
            let (owner, reg) = (Arc::clone(&owner), Arc::clone(&reg));
            std::thread::spawn(move || {
                dispatch_to_owner_with(
                    "control-lock",
                    &owner,
                    &reg,
                    Duration::from_secs(2),
                    Duration::from_millis(5),
                    |_| {
                        run_started_tx.send(()).expect("signal run");
                        release_rx.recv().expect("released");
                    },
                )
            })
        };
        run_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("the control started emitting");

        let claimer = {
            let owner = Arc::clone(&owner);
            std::thread::spawn(move || {
                // A newer reading claims the speakers.
                owner.claim(NO_REQUEST, || false, || {});
                let _ = claimed_tx.send(());
            })
        };
        assert!(
            claimed_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "a claim interleaved with the control's emit"
        );
        release_tx.send(()).expect("release the emit");
        claimed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the claim proceeds after the emit");
        assert!(dispatcher.join().unwrap(), "the control applied");
        claimer.join().unwrap();
    }
}
