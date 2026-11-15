use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    reg.attach_reading(a, 10);
    reg.attach_reading(b, 20);
    assert_eq!(
        reg.cancel("a", now, |reading| reading == Some(10)),
        StopOutcome::Cancelled {
            seq: a,
            stopped_playback: true
        }
    );
    assert!(reg.stopped_playback(a));
    // b's reading does not own the speakers (a's does): its cancel stops
    // nothing.
    assert_eq!(
        reg.cancel("b", now, |reading| reading == Some(10)),
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
fn owner_stop_reaches_only_the_current_reading() {
    let owner = PlaybackOwner::new();
    let stops = AtomicUsize::new(0);
    assert!(owner.claim(7, || false, || {}));
    // A stop for a different reading does nothing.
    assert!(!owner.stop_if_owner(8, || {
        stops.fetch_add(1, Ordering::SeqCst);
    }));
    // Reading 7 still owns playback: its stop runs.
    assert!(owner.stop_if_owner(7, || {
        stops.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[test]
fn newer_reading_takes_ownership_so_a_stale_stop_spares_it() {
    let owner = PlaybackOwner::new();
    owner.claim(7, || false, || {});
    // A later reading (9) or an unscoped emit (0) takes the speakers.
    owner.claim(NO_REQUEST, || false, || {});
    let mut stopped = false;
    assert!(!owner.stop_if_owner(7, || stopped = true));
    assert!(!stopped, "a stale stop must not cut a newer reading");
    assert!(!owner.stop_if_owner(NO_REQUEST, || stopped = true));
    assert!(!stopped, "reading 0 is never a scoped stop target");
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

#[test]
fn audible_claim_and_release_follow_the_webview() {
    let owner = PlaybackOwner::new();
    assert_eq!(owner.with_current(|current| current), NO_REQUEST);

    owner.claim_audible(0);
    assert_eq!(
        owner.with_current(|current| current),
        NO_REQUEST,
        "reading 0 means no reading and changes nothing"
    );

    owner.claim_audible(11);
    assert_eq!(owner.with_current(|current| current), 11);

    // A stale finish for another reading leaves the owner alone.
    owner.release_audible(12);
    assert_eq!(owner.with_current(|current| current), 11);

    // The matching finish clears the owner.
    owner.release_audible(11);
    assert_eq!(owner.with_current(|current| current), NO_REQUEST);

    // A second finish for the same reading changes nothing.
    owner.release_audible(11);
    assert_eq!(owner.with_current(|current| current), NO_REQUEST);
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
            .cancel(id, Instant::now(), |reading| reading == Some(current));
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
fn scoped_cancel_reaches_only_the_audio_of_the_requests_reading() {
    let owner = PlaybackOwner::new();
    let reg = Mutex::new(Registry::default());
    let now = Instant::now();
    let Registration::Proceed(waiting) = reg.lock().unwrap().register("cancel-waiting", now) else {
        panic!("waiting must register");
    };
    let Registration::Proceed(audible) = reg.lock().unwrap().register("cancel-audible", now) else {
        panic!("audible must register");
    };
    reg.lock().unwrap().attach_reading(waiting, 21);
    reg.lock().unwrap().attach_reading(audible, 22);
    owner.claim_audible(22);

    // The audible reading's stop reaches playback.
    let mut stopped = false;
    assert_eq!(
        cancel_on(&owner, &reg, "cancel-audible", || stopped = true),
        StopOutcome::Cancelled {
            seq: audible,
            stopped_playback: true
        }
    );
    assert!(stopped);

    // The waiting reading stops without cutting the audible reading.
    owner.claim_audible(22);
    assert_eq!(
        cancel_on(&owner, &reg, "cancel-waiting", || {
            panic!("the waiting reading owns no audio")
        }),
        StopOutcome::Cancelled {
            seq: waiting,
            stopped_playback: false
        }
    );
}

#[test]
fn scoped_cancel_before_the_reading_begins_stops_nothing() {
    let owner = PlaybackOwner::new();
    let reg = Mutex::new(Registry::default());
    let Registration::Proceed(seq) = reg
        .lock()
        .unwrap()
        .register("cancel-no-reading", Instant::now())
    else {
        panic!("must register");
    };
    owner.claim_audible(31);
    assert_eq!(
        cancel_on(&owner, &reg, "cancel-no-reading", || panic!(
            "a request with no reading owns nothing"
        )),
        StopOutcome::Cancelled {
            seq,
            stopped_playback: false
        }
    );
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
    let Registration::Proceed(cur) = reg.lock().unwrap().register("start-no-owner-cur", now) else {
        panic!("cur must register");
    };
    reg.lock().unwrap().attach_reading(prev, 101);
    reg.lock().unwrap().attach_reading(cur, 102);
    // prev becomes audible: the webview's playback-started claims it.
    owner.claim_audible(101);
    // cur starts synthesizing: no claim happens here (see SynthesisGuard).
    assert_eq!(
        cancel_on(&owner, &reg, "start-no-owner-cur", || panic!(
            "a reading with no audible audio owns nothing"
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
fn a_cancelled_reading_that_never_emits_claims_nothing() {
    let owner = PlaybackOwner::new();
    let mut emitted = false;
    // The scope reads cancelled: the emit sites' claim emits nothing.
    owner.claim(5, || true, || emitted = true);
    assert!(!emitted);
    assert_eq!(
        owner.with_current(|current| current),
        NO_REQUEST,
        "an aborted claim must not take the speakers"
    );
}

/// The cancel-then-check order closes the emit race: once the stop has
/// checked ownership, a concurrent emit for the cancelled request sees its
/// scope cancelled and emits nothing, so no audio can follow the stop.
#[test]
fn concurrent_emit_never_lands_after_the_scoped_stop() {
    for _ in 0..200 {
        let owner = Arc::new(PlaybackOwner::new());
        let reg = Arc::new(Mutex::new(Registry::default()));
        let Registration::Proceed(seq) = reg.lock().unwrap().register("r", Instant::now()) else {
            panic!("r must register");
        };
        let reading = 77;
        reg.lock().unwrap().attach_reading(seq, reading);
        let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));

        let emitter = {
            let (owner, reg, events) = (owner.clone(), reg.clone(), events.clone());
            std::thread::spawn(move || {
                owner.claim(
                    reading,
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
                        .cancel("r", Instant::now(), |reading| reading == Some(current));
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
    reg.lock().unwrap().attach_reading(seq, 41);
    owner.claim_audible(41);

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
    reg.lock().unwrap().attach_reading(seq, 42);
    let claimer = {
        let (owner, reg) = (Arc::clone(&owner), Arc::clone(&reg));
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            // The webview reports the reading's audio audible.
            assert_eq!(reg.lock().unwrap().live_seq("control-starting"), Some(seq));
            owner.claim_audible(42);
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
    reg.lock().unwrap().attach_reading(other, 43);
    owner.claim_audible(43);

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
    reg.lock().unwrap().attach_reading(seq, 44);
    owner.claim_audible(44);

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
