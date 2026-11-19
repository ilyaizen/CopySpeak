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
//! Playback is stopped only while the cancelled request's reading still owns
//! the speakers ([`PlaybackOwner`]): the webview reports which reading became
//! audible, and Rust claims that reading then. Any newer reading takes
//! ownership when its audio becomes audible, and the stale stop leaves it
//! alone. A reading that has only started synthesizing owns nothing, so
//! stopping it never cuts the previous reading.

use std::collections::{HashMap, HashSet};
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
    /// The reading this request serves: the process-wide playback identity its
    /// events carry. `None` until the reading's [`AbortScope`] begins, so a
    /// stop that arrives earlier owns nothing.
    ///
    /// [`AbortScope`]: crate::AbortScope
    reading: Option<u64>,
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
                reading: None,
                stopped_playback: false,
                touched: now,
            },
        );
        Registration::Proceed(seq)
    }

    /// Attach the reading `seq` serves, once its [`AbortScope`] allocated it.
    /// A reading that emits nothing (a failed or cancelled reading) still
    /// carries its id, so a later scoped stop can name it and drop its held
    /// audio.
    ///
    /// [`AbortScope`]: crate::AbortScope
    pub fn attach_reading(&mut self, seq: u64, reading: u64) {
        if reading == 0 {
            return;
        }
        if let Some(entry) = self.by_id.values_mut().find(|entry| entry.seq == seq) {
            entry.reading = Some(reading);
        }
    }

    /// The reading `seq` serves, or `None` when the request registered but has
    /// not begun a reading yet (or never did).
    pub fn reading_of(&self, seq: u64) -> Option<u64> {
        self.by_id
            .values()
            .find(|entry| entry.seq == seq)
            .and_then(|entry| entry.reading)
    }

    /// The reading of request `id`, whatever phase the request is in.
    pub fn reading_for_id(&self, id: &str) -> Option<u64> {
        self.by_id.get(id).and_then(|entry| entry.reading)
    }

    /// The sequence of `id` while its request is live. Unknown, finished and
    /// cancelled ids return `None`.
    pub fn live_seq(&self, id: &str) -> Option<u64> {
        self.by_id
            .get(id)
            .filter(|entry| entry.phase == Phase::Live)
            .map(|entry| entry.seq)
    }

    /// Cancel `id`. `owns_playback(reading)` reports whether that live
    /// request's reading still owns the speakers; the answer is recorded with
    /// the cancellation in one step, so a waiter never sees "cancelled"
    /// without it. A request whose reading has not begun passes `None`.
    pub fn cancel(
        &mut self,
        id: &str,
        now: Instant,
        owns_playback: impl FnOnce(Option<u64>) -> bool,
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
                        reading: None,
                        stopped_playback: false,
                        touched: now,
                    },
                );
                StopOutcome::Preempted
            }
            Some(entry) => match entry.phase {
                Phase::Live => {
                    let stopped_playback = owns_playback(entry.reading);
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

/// Which reading currently owns the speakers: the reading the webview last
/// reported audible. The value 0 means "no reading owns the speakers".
/// Claims and scoped stops serialize on this lock, so a stop emitted for a
/// request always reaches the webview before any later reading's start or
/// audio, never after it.
pub struct PlaybackOwner {
    current: Mutex<u64>,
}

impl PlaybackOwner {
    pub const fn new() -> Self {
        Self {
            current: Mutex::new(NO_REQUEST),
        }
    }

    /// Take ownership for `reading` and run `emit` under the lock, unless
    /// `aborted()` already reads true (then nothing is emitted). Reading 0
    /// clears the owner: audio that belongs to no reading takes the speakers
    /// away from any reading.
    pub fn claim(&self, reading: u64, aborted: impl FnOnce() -> bool, emit: impl FnOnce()) -> bool {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if aborted() {
            return false;
        }
        *current = reading;
        emit();
        true
    }

    /// The webview reports `reading` audible: that reading now owns the
    /// speakers. Reading 0 means "no reading" and changes nothing.
    pub fn claim_audible(&self, reading: u64) {
        if reading == NO_REQUEST {
            return;
        }
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        *current = reading;
    }

    /// The webview reports `reading` finished: clear the owner only when that
    /// reading still holds it, so a stale finish never takes the speakers from
    /// a newer reading.
    pub fn release_audible(&self, reading: u64) {
        if reading == NO_REQUEST {
            return;
        }
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if *current == reading {
            *current = NO_REQUEST;
        }
    }

    /// Run `f` with the current owner while holding the owner lock, so no
    /// claim (and no emit) can interleave with it.
    pub fn with_current<R>(&self, f: impl FnOnce(u64) -> R) -> R {
        let current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        f(*current)
    }

    /// Run `stop` only while `reading` still owns the speakers. Production
    /// stops go through [`cancel`], which decides and stops in one critical
    /// section; this is the owner rule on its own, for tests.
    #[cfg(test)]
    pub fn stop_if_owner(&self, reading: u64, stop: impl FnOnce()) -> bool {
        let current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if reading == NO_REQUEST || *current != reading {
            return false;
        }
        stop();
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

/// Attach the reading a request serves (see [`Registry::attach_reading`]).
pub fn attach_reading(request: u64, reading: u64) {
    if request == NO_REQUEST {
        return;
    }
    registry().attach_reading(request, reading);
}

/// The reading of request `id`, whatever phase it is in. The control server
/// uses this to name a stopped reading in `playback-reading-dropped`.
pub fn reading_for_id(id: &str) -> Option<u64> {
    registry().reading_for_id(id)
}

/// The webview reports `reading` audible: claim the speakers for it.
pub fn claim_audible(reading: u64) {
    OWNER.claim_audible(reading);
}

/// The webview reports `reading` finished: release the speakers when it still
/// holds them.
pub fn release_audible(reading: u64) {
    OWNER.release_audible(reading);
}

/// Cancel `id`; when the request is live and its reading still owns playback,
/// run `stop`.
///
/// Everything happens under the owner lock (lock order owner → registry, the
/// same as an emit's claim): an emit for this reading either ran before (its
/// audio is then stopped) or runs after and sees the cancelled scope, and no
/// newer reading can become audible between the owner check and `stop`.
pub fn cancel(id: &str, stop: impl FnOnce()) -> StopOutcome {
    OWNER.with_current(|owner| {
        let outcome = registry().cancel(id, Instant::now(), |reading| reading == Some(owner));
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

/// Claim the speakers for `reading` and emit under the owner lock. Reading 0
/// belongs to no reading (a helper scope): claiming it takes the speakers away
/// from any reading.
pub fn claim_playback(reading: u64, aborted: impl FnOnce() -> bool, emit: impl FnOnce()) -> bool {
    OWNER.claim(reading, aborted, emit)
}

/// Whether request `seq`'s reading owns the speakers, that is, the webview
/// reported it audible. Lock order owner -> registry, the same as [`cancel`].
pub fn is_audible(seq: u64) -> bool {
    OWNER.with_current(|current| {
        current != NO_REQUEST && registry().reading_of(seq) == Some(current)
    })
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
/// past this bound it is answered as not playing. Two seconds gives a
/// synthesizing reading time to emit its first chunk without blocking the
/// control connection thread excessively.
const CONTROL_OWNER_WAIT: Duration = Duration::from_millis(2000);
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
/// owner with the live request's reading, then run under the owner lock (lock
/// order owner -> registry, the same as [`cancel`]). A request whose reading
/// did not begin yet is not the owner. The registry guard drops before `run`,
/// so the emit never holds the registry lock.
fn try_run_if_owner(
    id: &str,
    owner: &PlaybackOwner,
    registry: &Mutex<Registry>,
    run: &mut impl FnMut(u64),
) -> OwnerCheck {
    owner.with_current(|current| {
        let (seq, reading) = {
            let registry = registry.lock().unwrap_or_else(|e| e.into_inner());
            match registry.live_seq(id) {
                None => return OwnerCheck::NotLive,
                Some(seq) => (seq, registry.reading_of(seq)),
            }
        };
        match reading {
            Some(reading) if reading == current => {
                run(seq);
                OwnerCheck::Ran
            }
            _ => OwnerCheck::NotOwner,
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
#[path = "speak_requests_tests.rs"]
mod tests;
