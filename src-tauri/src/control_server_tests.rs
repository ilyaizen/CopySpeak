use super::*;

#[test]
fn loopback_only_accepts_loopback_addresses() {
    assert!(is_loopback_addr("127.0.0.1:43117"));
    assert!(is_loopback_addr("127.9.9.9:8080"));
    assert!(is_loopback_addr("[::1]:43117"));
    assert!(is_loopback_addr("localhost:43117"));
    assert!(is_loopback_addr("LOCALHOST:43117"));
}

#[test]
fn loopback_rejects_non_loopback_and_garbage() {
    assert!(!is_loopback_addr("0.0.0.0:43117"));
    assert!(!is_loopback_addr("192.168.1.5:43117"));
    assert!(!is_loopback_addr("[::]:43117"));
    assert!(!is_loopback_addr("example.com:43117"));
    assert!(!is_loopback_addr("10.0.0.1:43117"));
    // Malformed / unresolvable values fail closed.
    assert!(!is_loopback_addr(""));
    assert!(!is_loopback_addr("not an address"));
}

#[test]
fn parse_request_routes_stop_without_body() {
    let raw = b"POST /stop HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
    assert!(matches!(
        parse_request(raw),
        Ok(ControlRequest::Stop(StopRequest { request_id: None }))
    ));
}

#[test]
fn parse_request_routes_status() {
    assert!(matches!(
        parse_request(b"GET /status HTTP/1.1\r\nHost: 127.0.0.1:43117\r\n\r\n"),
        Ok(ControlRequest::Status)
    ));
}

/// A reported caption is served verbatim, and the response opens CORS so
/// the DSH client can read it from its own origin.
#[test]
fn status_body_serves_the_reported_caption_with_cors() {
    let caption = r#"{"text":"hi","captions":null,"position_ms":42,"duration_ms":1000,"paused":false,"active":true}"#;
    crate::playback_signal::remember_caption(caption.to_string());
    assert_eq!(status_body(), caption);
    let response = http_response_cors(200, "OK", &status_body());
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: application/json\r\n"));
    assert!(response.contains("Access-Control-Allow-Origin: *\r\n"));
    assert!(response.ends_with(caption));
    // The existing routes keep the shared helper and stay CORS-free.
    let plain = http_response(200, "OK", r#"{"ok":true}"#);
    assert!(!plain.contains("Access-Control-Allow-Origin"));
}

fn stop_request(body: &str) -> Vec<u8> {
    format!(
        "POST /stop HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

/// Build a POST request for any control route, with a matching length.
fn post_request(path: &str, body: &str) -> Vec<u8> {
    format!(
        "POST {} HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
        path,
        body.len(),
        body
    )
    .into_bytes()
}

#[test]
fn parse_request_routes_scoped_stop() {
    match parse_request(&stop_request(r#"{"request_id":"hermes-abc123"}"#)) {
        Ok(ControlRequest::Stop(StopRequest {
            request_id: Some(id),
        })) => assert_eq!(id, "hermes-abc123"),
        _ => panic!("expected a scoped stop"),
    }
    // An empty JSON object is the legacy global stop.
    assert!(matches!(
        parse_request(&stop_request("{}")),
        Ok(ControlRequest::Stop(StopRequest { request_id: None }))
    ));
}

#[test]
fn parse_request_rejects_bad_request_ids() {
    assert!(matches!(
        parse_request(&stop_request(r#"{"request_id":"bad id"}"#)),
        Err((400, _))
    ));
    assert!(matches!(
        parse_request(&stop_request("not json")),
        Err((400, _))
    ));
    let speak = serde_json::json!({ "text": "hi", "request_id": "" });
    assert!(matches!(
        parse_request(&speak_request(&speak)),
        Err((400, _))
    ));
}

#[test]
fn parse_request_routes_scoped_pause_and_resume() {
    match parse_request(&post_request("/pause", r#"{"request_id":"hermes-p"}"#)) {
        Ok(ControlRequest::Pause(id)) => assert_eq!(id, "hermes-p"),
        _ => panic!("expected a pause control"),
    }
    match parse_request(&post_request("/resume", r#"{"request_id":"hermes-r"}"#)) {
        Ok(ControlRequest::Resume(id)) => assert_eq!(id, "hermes-r"),
        _ => panic!("expected a resume control"),
    }
}

#[test]
fn parse_request_routes_seek_with_signed_delta() {
    for (body, expected) in [
        (r#"{"request_id":"hermes-s","delta_ms":5000}"#, 5000i64),
        (r#"{"request_id":"hermes-s","delta_ms":-5000}"#, -5000),
        (r#"{"request_id":"hermes-s","delta_ms":60000}"#, 60_000),
        (r#"{"request_id":"hermes-s","delta_ms":-60000}"#, -60_000),
    ] {
        match parse_request(&post_request("/seek", body)) {
            Ok(ControlRequest::Seek(SeekControl {
                request_id,
                delta_ms,
            })) => {
                assert_eq!(request_id, "hermes-s");
                assert_eq!(delta_ms, expected);
            }
            _ => panic!("expected a seek control for {body}"),
        }
    }
}

#[test]
fn parse_request_rejects_incomplete_or_invalid_controls() {
    // Missing or invalid request ids.
    assert!(matches!(
        parse_request(&post_request("/pause", "{}")),
        Err((400, _))
    ));
    assert!(matches!(
        parse_request(&post_request("/pause", r#"{"request_id":"bad id"}"#)),
        Err((400, _))
    ));
    assert!(matches!(
        parse_request(&post_request("/resume", "not json")),
        Err((400, _))
    ));
    // Seek needs a non-zero integer step inside the accepted range.
    for body in [
        r#"{"request_id":"hermes-s"}"#,
        r#"{"request_id":"hermes-s","delta_ms":0}"#,
        r#"{"request_id":"hermes-s","delta_ms":1.5}"#,
        r#"{"request_id":"hermes-s","delta_ms":60001}"#,
        r#"{"request_id":"hermes-s","delta_ms":-60001}"#,
        r#"{"request_id":"bad id","delta_ms":5000}"#,
    ] {
        assert!(
            matches!(parse_request(&post_request("/seek", body)), Err((400, _))),
            "{body} must be rejected"
        );
    }
}

#[test]
fn parse_request_404_names_every_control_route() {
    match parse_request(&post_request("/play", "{}")) {
        Err((404, message)) => {
            for path in ["/pause", "/resume", "/seek", "/stop", "/speak"] {
                assert!(message.contains(path), "message misses {path}: {message}");
            }
        }
        _ => panic!("an unknown route must 404"),
    }
}

#[test]
fn control_body_reports_the_action_or_not_playing() {
    let applied: serde_json::Value =
        serde_json::from_str(&control_body("paused", "hermes-p", true)).expect("valid JSON");
    assert_eq!(applied["ok"], true);
    assert_eq!(applied["request_id"], "hermes-p");
    assert_eq!(applied["outcome"], "paused");

    let refused: serde_json::Value =
        serde_json::from_str(&control_body("seeked", "hermes-s", false)).expect("valid JSON");
    assert_eq!(refused["ok"], true);
    assert_eq!(refused["request_id"], "hermes-s");
    assert_eq!(refused["outcome"], "not_playing");
}

#[test]
fn speak_request_parses_request_id() {
    let request = serde_json::json!({ "text": "hello", "request_id": "hermes-1", "wait": true });
    match parse_request(&speak_request(&request)) {
        Ok(ControlRequest::Speak(speak)) => {
            assert_eq!(speak.request_id.as_deref(), Some("hermes-1"));
        }
        _ => panic!("expected a speak request"),
    }
}

// ---- request lifecycle (run_speak) ----
//
// `speak` and `wait_finished` are test doubles for synthesis and the
// webview's playback-finished signal; the registry, the wait lock and the
// cancellation polling are the real ones. Ids are unique per test because
// the registry is process-global.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::oneshot;

fn unique_id(name: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!("{}-{}", name, NEXT.fetch_add(1, Ordering::SeqCst))
}

/// A finish that never comes (models a wait the test must not rely on).
fn never_finishes(_generation: u64) -> std::future::Pending<bool> {
    std::future::pending()
}

/// Bound every lifecycle run: a hang is the legacy failure mode.
async fn bounded<F: Future>(fut: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(3), fut)
        .await
        .expect("speak request hung instead of settling")
}

#[tokio::test]
async fn stop_before_speak_registers_never_speaks() {
    let id = unique_id("pre");
    assert_eq!(
        speak_requests::cancel(&id, || panic!("nothing plays yet")),
        speak_requests::StopOutcome::Preempted
    );
    let lock = tokio::sync::Mutex::new(());
    let spoken = AtomicUsize::new(0);
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |_| {
            spoken.fetch_add(1, Ordering::SeqCst);
            async { Ok(()) }
        },
        never_finishes,
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
    assert_eq!(
        spoken.load(Ordering::SeqCst),
        0,
        "a pre-empted request spoke"
    );
}

#[test]
fn invalid_speak_options_are_rejected_before_interrupt() {
    let mut request: SpeakRequest =
        serde_json::from_str(r#"{"text":"hello","engine":"not-an-engine"}"#).unwrap();
    assert!(validate_speak_options(&request).is_err());
    request.engine = None;
    request.effect = Some("not-an-effect".to_string());
    assert!(validate_speak_options(&request).is_err());
}

#[tokio::test]
async fn preempted_and_duplicate_requests_do_not_interrupt_playback() {
    let lock = tokio::sync::Mutex::new(());
    let interrupts = AtomicUsize::new(0);
    let preempted = unique_id("interrupt-preempted");
    speak_requests::cancel(&preempted, || panic!("nothing plays"));
    let result = run_speak_with_interrupt(
        Some(&preempted),
        true,
        &lock,
        abort_generation,
        || {
            interrupts.fetch_add(1, Ordering::SeqCst);
            false
        },
        |_, _| async { Ok(()) },
        never_finishes,
    )
    .await;
    assert_eq!(result, Ok(SpeakOutcome::Cancelled));

    let duplicate = unique_id("interrupt-duplicate");
    let Registration::Proceed(seq) = speak_requests::register(&duplicate) else {
        panic!("request must register");
    };
    let _ticket = Ticket::new(seq);
    let result = run_speak_with_interrupt(
        Some(&duplicate),
        true,
        &lock,
        abort_generation,
        || {
            interrupts.fetch_add(1, Ordering::SeqCst);
            false
        },
        |_, _| async { Ok(()) },
        never_finishes,
    )
    .await;
    assert!(matches!(result, Err((409, _))));
    assert_eq!(interrupts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn global_abort_during_synthesis_does_not_wait_for_playback() {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn generation() -> u64 {
        GENERATION.load(Ordering::SeqCst)
    }
    let lock = tokio::sync::Mutex::new(());
    let result = bounded(run_speak_with_interrupt(
        None,
        true,
        &lock,
        generation,
        || false,
        |_, _| async {
            GENERATION.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        never_finishes,
    ))
    .await;
    assert_eq!(result, Ok(SpeakOutcome::Cancelled));
}

#[tokio::test]
async fn global_abort_releases_request_queued_before_synthesis() {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn generation() -> u64 {
        GENERATION.load(Ordering::SeqCst)
    }
    let lock = tokio::sync::Mutex::new(());
    let _held = lock.lock().await;
    let spoken = AtomicBool::new(false);
    let (result, ()) = tokio::join!(
        bounded(run_speak_with_interrupt(
            None,
            true,
            &lock,
            generation,
            || false,
            |_, _| {
                spoken.store(true, Ordering::SeqCst);
                async { Ok(()) }
            },
            never_finishes,
        )),
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            GENERATION.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(result, Ok(SpeakOutcome::Cancelled));
    assert!(!spoken.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_requests_own_interrupt_does_not_cancel_it() {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn generation() -> u64 {
        GENERATION.load(Ordering::SeqCst)
    }
    let lock = tokio::sync::Mutex::new(());
    let result = bounded(run_speak_with_interrupt(
        None,
        true,
        &lock,
        generation,
        || {
            GENERATION.fetch_add(1, Ordering::SeqCst);
            true
        },
        |_, captured| {
            assert_eq!(captured, GENERATION.load(Ordering::SeqCst));
            async { Ok(()) }
        },
        |_| async { true },
    ))
    .await;
    assert_eq!(result, Ok(SpeakOutcome::Spoken));
}

#[tokio::test]
async fn global_stop_racing_with_own_interrupt_cancels_the_request() {
    static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn generation() -> u64 {
        GENERATION.load(Ordering::SeqCst)
    }
    let lock = tokio::sync::Mutex::new(());
    let spoken = AtomicBool::new(false);
    let result = bounded(run_speak_with_interrupt(
        None,
        true,
        &lock,
        generation,
        || {
            GENERATION.fetch_add(2, Ordering::SeqCst);
            true
        },
        |_, _| {
            spoken.store(true, Ordering::SeqCst);
            async { Ok(()) }
        },
        never_finishes,
    ))
    .await;
    assert_eq!(result, Ok(SpeakOutcome::Cancelled));
    assert!(!spoken.load(Ordering::SeqCst));
}

#[tokio::test]
async fn stop_while_queued_behind_another_wait_never_speaks() {
    let id = unique_id("queued");
    let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let held = lock.clone().lock_owned().await; // an earlier request is playing
    let spoken = std::sync::Arc::new(AtomicUsize::new(0));
    let task = {
        let (id, lock, spoken) = (id.clone(), lock.clone(), spoken.clone());
        tokio::spawn(async move {
            run_speak(
                Some(&id),
                true,
                &lock,
                |_| {
                    spoken.fetch_add(1, Ordering::SeqCst);
                    async { Ok(()) }
                },
                never_finishes,
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(
        !task.is_finished(),
        "queued request settled before its stop"
    );
    assert!(matches!(
        speak_requests::cancel(&id, || panic!("nothing of this request plays")),
        speak_requests::StopOutcome::Cancelled { .. }
    ));
    let outcome = bounded(task).await.expect("join");
    drop(held);
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
    assert_eq!(
        spoken.load(Ordering::SeqCst),
        0,
        "a cancelled queued request spoke"
    );
}

/// The reported race: the stop lands while the request is synthesizing.
/// The request must settle cancelled at once instead of waiting for a
/// playback whose audio its cancelled scope never emits.
#[tokio::test]
async fn stop_during_synthesis_settles_cancelled_without_waiting() {
    let id = unique_id("synth");
    let lock = tokio::sync::Mutex::new(());
    let (started_tx, started_rx) = oneshot::channel::<()>();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let canceller = {
        let id = id.clone();
        tokio::spawn(async move {
            started_rx.await.expect("speak started");
            let outcome = speak_requests::cancel(&id, || {});
            release_tx.send(()).expect("release synthesis");
            outcome
        })
    };
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |seq| async move {
            assert_ne!(seq, NO_REQUEST, "speak must run under the request's scope");
            started_tx.send(()).expect("signal start");
            release_rx.await.expect("released");
            // Synthesis observes the cancellation through its scope.
            assert!(crate::AbortScope::begin_for_request(seq).aborted());
            Ok(())
        },
        never_finishes,
    ))
    .await;
    assert!(matches!(
        canceller.await.expect("join"),
        speak_requests::StopOutcome::Cancelled { .. }
    ));
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
}

/// A stopped playback's finish must arrive before the next unscoped
/// waiter, which accepts any finish, can start.
#[tokio::test]
async fn cancel_that_stopped_playback_absorbs_its_own_finish() {
    let id = unique_id("absorb");
    let lock = tokio::sync::Mutex::new(());
    let (finish_tx, finish_rx) = oneshot::channel::<()>();
    let finish_tx = std::sync::Mutex::new(Some(finish_tx));
    let finish_seen = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let canceller = {
        let id = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            speak_requests::cancel(&id, || {
                // The webview stops and reports the finish ~150 ms later.
                let tx = finish_tx.lock().unwrap().take().expect("one stop");
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(150));
                    let _ = tx.send(());
                });
            })
        })
    };
    let seen = finish_seen.clone();
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |seq| async move {
            // The reading started and the webview reported it audible: it
            // owns the speakers, and a scoped stop compares its reading.
            let reading = 9001;
            speak_requests::attach_reading(seq, reading);
            speak_requests::claim_audible(reading);
            Ok(())
        },
        move |_| async move {
            let finished = finish_rx.await.is_ok();
            seen.store(finished, Ordering::SeqCst);
            finished
        },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
    assert!(matches!(
        canceller.await.expect("join"),
        speak_requests::StopOutcome::Cancelled {
            stopped_playback: true,
            ..
        }
    ));
    assert!(
        finish_seen.load(Ordering::SeqCst),
        "answered before its own stop's playback-finished landed"
    );
}

/// A synthesis that fails because it was cancelled is a cancellation,
/// not a 500.
#[tokio::test]
async fn cancelled_synthesis_error_is_reported_as_cancelled() {
    let id = unique_id("synth-err");
    let lock = tokio::sync::Mutex::new(());
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |_| {
            let id = id.clone();
            async move {
                speak_requests::cancel(&id, || {});
                Err("Reading cancelled".to_string())
            }
        },
        never_finishes,
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
}

/// Stop while the request's audio plays: the blocked waiter is released
/// immediately rather than at the next playback-finished.
#[tokio::test]
async fn stop_during_playback_wait_releases_the_waiter() {
    let id = unique_id("playing");
    let lock = tokio::sync::Mutex::new(());
    let canceller = {
        let id = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            speak_requests::cancel(&id, || {})
        })
    };
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |_| async { Ok(()) },
        never_finishes,
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Cancelled));
    assert!(matches!(
        canceller.await.expect("join"),
        speak_requests::StopOutcome::Cancelled { .. }
    ));
}

/// The reported race: replies read back to back. A foreign
/// playback-finished (the previous reply's late stop, a double-copy
/// drain) fires while this request is still synthesizing. The wait must
/// match this request's own reading and ignore every foreign finish —
/// an early release resets the plugin's action row while its audio has
/// not even started.
#[tokio::test]
async fn a_foreign_finish_does_not_satisfy_the_requests_wait() {
    let _guard = crate::playback_signal::playback_wait_test_lock();
    let id = unique_id("foreign-finish");
    let lock = tokio::sync::Mutex::new(());
    // A foreign playback run ends 60 ms in, mid-synthesis of this request.
    let foreign_fired = std::sync::Arc::new(AtomicBool::new(false));
    {
        let foreign_fired = foreign_fired.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            foreign_fired.store(true, Ordering::SeqCst);
            crate::playback_signal::signal_finished_for(None);
        });
    }
    // This request's own playback ends 150 ms after the foreign finish.
    {
        let id = id.clone();
        tokio::spawn(async move {
            let reading = loop {
                if let Some(reading) = speak_requests::reading_for_id(&id) {
                    break reading;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            };
            tokio::time::sleep(Duration::from_millis(150)).await;
            crate::playback_signal::signal_finished_for(Some(reading));
        });
    }
    let started = std::time::Instant::now();
    let wait_id = id.clone();
    let outcome = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |seq| async move {
            // Synthesis begins the reading (attaching it) and takes long
            // enough that the foreign finish lands inside it.
            crate::AbortScope::begin_reading_for_request(seq);
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        },
        move |_| {
            let waiter = crate::playback_signal::PlaybackWait::new(Some(&wait_id));
            async move {
                waiter
                    .wait(Duration::from_secs(SPEAK_WAIT_TIMEOUT_SECS))
                    .await
            }
        },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    assert!(
        foreign_fired.load(Ordering::SeqCst),
        "the foreign finish must land during the run for this test to bite"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "resolved on the foreign finish after {:?}, not on this request's own playback",
        started.elapsed()
    );
}

/// A stop that arrives after its request finished must not cancel, delay
/// or stop a later request.
#[tokio::test]
async fn late_stop_spares_the_next_request() {
    let first = unique_id("first");
    let second = unique_id("second");
    let lock = tokio::sync::Mutex::new(());
    let outcome = bounded(run_speak(
        Some(&first),
        true,
        &lock,
        |_| async { Ok(()) },
        |_| async { true },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    assert_eq!(
        speak_requests::cancel(&first, || panic!("a finished request owns nothing")),
        speak_requests::StopOutcome::AlreadyFinished
    );
    let spoken = AtomicUsize::new(0);
    let outcome = bounded(run_speak(
        Some(&second),
        true,
        &lock,
        |seq| {
            spoken.fetch_add(1, Ordering::SeqCst);
            async move {
                assert!(!crate::AbortScope::begin_for_request(seq).aborted());
                Ok(())
            }
        },
        |_| async { true },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    assert_eq!(spoken.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reused_request_id_is_rejected() {
    let id = unique_id("dup");
    let lock = tokio::sync::Mutex::new(());
    let first = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |_| async { Ok(()) },
        |_| async { true },
    ))
    .await;
    assert_eq!(first, Ok(SpeakOutcome::Spoken));
    let spoken = AtomicUsize::new(0);
    let second = bounded(run_speak(
        Some(&id),
        true,
        &lock,
        |_| {
            spoken.fetch_add(1, Ordering::SeqCst);
            async { Ok(()) }
        },
        |_| async { true },
    ))
    .await;
    assert!(matches!(second, Err((409, _))), "{:?}", second);
    assert_eq!(spoken.load(Ordering::SeqCst), 0);
}

/// No request id: the legacy lifecycle (speak, then wait for the finish).
#[tokio::test]
async fn unscoped_speak_keeps_the_legacy_lifecycle() {
    let lock = tokio::sync::Mutex::new(());
    let waited = std::sync::Arc::new(AtomicUsize::new(0));
    let waited_in = waited.clone();
    let outcome = bounded(run_speak(
        None,
        true,
        &lock,
        |seq| async move {
            assert_eq!(seq, NO_REQUEST);
            Ok(())
        },
        move |_| {
            waited_in.fetch_add(1, Ordering::SeqCst);
            async { true }
        },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    assert_eq!(waited.load(Ordering::SeqCst), 1);
    assert!(matches!(
        bounded(run_speak(
            None,
            true,
            &lock,
            |_| async { Err("boom".to_string()) },
            never_finishes
        ))
        .await,
        Err((500, _))
    ));
}

/// A non-waiting speak answers at playback start but stays stoppable
/// until its playback finishes; after that a stop is a no-op.
#[tokio::test]
async fn non_wait_request_stays_stoppable_until_playback_ends() {
    let playing = unique_id("nowait-playing");
    let lock = tokio::sync::Mutex::new(());
    let outcome = bounded(run_speak(
        Some(&playing),
        false,
        &lock,
        |_| async { Ok(()) },
        never_finishes,
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    assert!(matches!(
        speak_requests::cancel(&playing, || {}),
        speak_requests::StopOutcome::Cancelled { .. }
    ));

    let ended = unique_id("nowait-ended");
    let (finish_tx, finish_rx) = oneshot::channel::<()>();
    let outcome = bounded(run_speak(
        Some(&ended),
        false,
        &lock,
        |_| async { Ok(()) },
        move |_| async move { finish_rx.await.is_ok() },
    ))
    .await;
    assert_eq!(outcome, Ok(SpeakOutcome::Spoken));
    finish_tx.send(()).expect("finish playback");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while speak_requests::is_live(&ended) {
        assert!(
            std::time::Instant::now() < deadline,
            "non-wait request never finished"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        speak_requests::cancel(&ended, || panic!("an ended request owns nothing")),
        speak_requests::StopOutcome::AlreadyFinished
    );
}

/// A known-slow client changes its mind and disconnects instead of
/// completing the request; the reader must time out and free the
/// connection rather than blocking forever.
#[test]
fn slow_stalled_client_is_dropped_after_read_timeout() {
    let addr = "127.0.0.1:0";
    let listener = TcpListener::bind(addr).expect("bind ephemeral port");
    let server_addr = listener.local_addr().expect("local addr");

    let judge = std::thread::spawn(move || {
        let mut established = listener
            .incoming()
            .next()
            .expect("one connection")
            .expect("accept");
        let _ = established.set_read_timeout(Some(READ_TIMEOUT));
        let started = std::time::Instant::now();
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0_u8; 4096];
        // The partial body arrives quickly; keep reading until the
        // read-timeout fires while the client holds the connection open.
        let error = loop {
            match established.read(&mut chunk) {
                Ok(0) => panic!("client EOF before the read timeout fired"),
                Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                Err(error) => break error,
            }
        };
        let timed_out = error.kind() == std::io::ErrorKind::WouldBlock
            || error.kind() == std::io::ErrorKind::TimedOut;
        assert!(timed_out, "read failed with a non-timeout error: {}", error);
        let elapsed = started.elapsed();
        assert!(
            elapsed >= READ_TIMEOUT,
            "reader gave up early after {:?}",
            elapsed
        );
        assert!(
            elapsed < READ_TIMEOUT + Duration::from_secs(3),
            "reader lingered {:?} past its timeout",
            elapsed - READ_TIMEOUT
        );
    });

    // The client half: connect, send a partial body, then stall — never
    // send the remaining 8 declared bytes and never close the socket.
    let _stall_client = std::thread::spawn(move || {
        let mut client = TcpStream::connect(server_addr).expect("connect");
        client
            .write_all(
                b"POST /speak HTTP/1.1
Content-Length: 10

ab",
            )
            .expect("write partial body");
        client.flush().expect("flush");
        // Hold the connection open well beyond READ_TIMEOUT so the
        // server-side read timeout is what severs the request.
        std::thread::sleep(READ_TIMEOUT + Duration::from_secs(3));
        drop(client);
    });

    judge.join().expect("server-side assertions hold");
}

/// Build a POST /speak request whose Content-Length always matches.
fn speak_request(body: &serde_json::Value) -> Vec<u8> {
    let body = body.to_string();
    format!(
        "POST /speak HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

#[test]
fn speak_request_parses_wait_field() {
    let request = serde_json::json!({ "text": "hello", "wait": true });
    match parse_request(&speak_request(&request)) {
        Ok(ControlRequest::Speak(speak)) => {
            assert_eq!(speak.text, "hello");
            assert_eq!(speak.wait, Some(true));
        }
        _ => panic!("expected a speak request"),
    }
}

#[test]
fn speak_request_wait_defaults_to_false() {
    let request = serde_json::json!({ "text": "hello" });
    match parse_request(&speak_request(&request)) {
        Ok(ControlRequest::Speak(speak)) => {
            assert_eq!(speak.wait, None);
            assert_eq!(speak.post_process, None);
        }
        _ => panic!("expected a speak request"),
    }
}

/// Legacy callers send profile/persist_selection fields the server drops;
/// parsing must keep accepting them alongside the post_process/wait
/// fields. `condense` is the retired wire name and must keep working.
#[test]
fn speak_request_tolerates_unknown_fields() {
    let request = serde_json::json!({
        "text": "hello",
        "profile": "pi",
        "persist_selection": true,
        "condense": true,
        "wait": false
    });
    match parse_request(&speak_request(&request)) {
        Ok(ControlRequest::Speak(speak)) => {
            assert_eq!(speak.post_process, Some(true));
            assert_eq!(speak.wait, Some(false));
        }
        _ => panic!("expected a speak request"),
    }
}

/// The current wire name parses too.
#[test]
fn speak_request_accepts_post_process_field() {
    let request = serde_json::json!({ "text": "hello", "post_process": true });
    match parse_request(&speak_request(&request)) {
        Ok(ControlRequest::Speak(speak)) => {
            assert_eq!(speak.post_process, Some(true));
        }
        _ => panic!("expected a speak request"),
    }
}
