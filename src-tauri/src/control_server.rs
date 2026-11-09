use crate::audio::AudioPlayer;
use crate::config::{self, AppConfig, EffectId, TtsEngine};
use crate::history::HistoryLog;
use crate::speak_requests::{self, Registration, Ticket, NO_REQUEST};
use crate::telemetry;
use serde::Deserialize;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const DEFAULT_ADDR: &str = "127.0.0.1:43117";
const MAX_BODY_BYTES: usize = 200_000;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for a blocking speak: one long paragraph can speak for
/// minutes; past this the server answers anyway (the user may have stopped
/// playback, which also resolves the wait from the client side).
const SPEAK_WAIT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Deserialize)]
struct SpeakRequest {
    text: String,
    engine: Option<String>,
    effect: Option<String>,
    /// Ask the post-process LLM to run for this request even when the global
    /// `require_request_flag` gate would otherwise skip it (Hermes callers).
    /// `condense` is the pre-0.2.15 wire name, kept as an alias.
    #[serde(alias = "condense")]
    post_process: Option<bool>,
    /// Block until webview playback finishes (or the user stops it) before
    /// answering; serializes sentence-by-sentence Hermes callers.
    wait: Option<bool>,
    /// Caller-chosen id that a later `POST /stop {"request_id"}` cancels.
    /// The stop may arrive before this speak does; see `speak_requests`.
    request_id: Option<String>,
}

/// `POST /stop` body. Without `request_id` (or without a body) the stop is
/// the legacy global one: stop whatever plays now.
#[derive(Debug, Default, Deserialize)]
struct StopRequest {
    request_id: Option<String>,
}

enum ControlRequest {
    Health,
    Stop(StopRequest),
    Speak(SpeakRequest),
}

/// How a speak request ended.
#[derive(Debug, PartialEq, Eq)]
enum SpeakOutcome {
    Spoken,
    /// A scoped stop cancelled it (before it started, while it was queued or
    /// synthesizing, or while it played).
    Cancelled,
}

pub fn start(app: AppHandle) {
    let addr = std::env::var("COPYSPEAK_CONTROL_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    if !is_loopback_addr(&addr) {
        log::error!(
            "[Control] Refusing to bind control server to non-loopback address {}.",
            addr
        );
        return;
    }
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(&addr) {
            Ok(listener) => listener,
            Err(error) => {
                log::warn!("[Control] Failed to bind {}: {}", addr, error);
                return;
            }
        };

        log::info!("[Control] Listening on http://{}", addr);
        // One wait at a time: a new blocking speak must not race the previous
        // request's completion wait. Non-wait callers are unaffected.
        let wait_lock: Arc<tokio::sync::Mutex<()>> = Arc::new(tokio::sync::Mutex::new(()));
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app = app.clone();
                    let wait_lock = Arc::clone(&wait_lock);
                    // Handle connections on their own threads so a blocking
                    // speak leaves the accept loop free (health checks and
                    // later speaks keep working while one waits).
                    std::thread::spawn(move || handle_connection(stream, app, wait_lock));
                }
                Err(error) => log::warn!("[Control] Connection failed: {}", error),
            }
        }
    });
}

fn handle_connection(
    mut stream: TcpStream,
    app: AppHandle,
    wait_lock: Arc<tokio::sync::Mutex<()>>,
) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));

    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    let read_result = loop {
        match stream.read(&mut chunk) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                match request_state(&buffer) {
                    RequestState::Incomplete => continue,
                    RequestState::Complete => break Ok(()),
                    RequestState::TooLarge => break Err((413, "request too large".to_string())),
                }
            }
            Err(error) => {
                log::warn!("[Control] Read failed: {}", error);
                return;
            }
        }
    };

    let response = match read_result.and_then(|()| parse_request(&buffer)) {
        Ok(ControlRequest::Health) => http_response(200, "OK", r#"{"ok":true,"app":"CopySpeak"}"#),
        Ok(ControlRequest::Stop(StopRequest { request_id: None })) => {
            // Same path as the UI stop: webview stop event + Rodio net. The
            // webview's handleStop emits the terminal playback signal, which
            // releases any blocking `/speak --wait` caller (Hermes barge-in).
            crate::commands::stop_all_audio(&app, &app.state::<Mutex<AudioPlayer>>());
            http_response(200, "OK", r#"{"ok":true}"#)
        }
        Ok(ControlRequest::Stop(StopRequest {
            request_id: Some(id),
        })) => {
            // Scoped stop: cancels exactly this request, whatever phase it is
            // in, and stops playback only while that request owns it.
            let outcome = speak_requests::cancel(&id, || {
                crate::commands::stop_all_audio(&app, &app.state::<Mutex<AudioPlayer>>());
            });
            log::info!("[Control] Stop for request {}: {}", id, outcome.as_str());
            let body = serde_json::json!({
                "ok": true,
                "request_id": id,
                "outcome": outcome.as_str(),
            });
            http_response(200, "OK", &body.to_string())
        }
        Ok(ControlRequest::Speak(request)) => {
            match tauri::async_runtime::block_on(handle_speak(app, request, wait_lock)) {
                Ok(SpeakOutcome::Spoken) => http_response(200, "OK", r#"{"ok":true}"#),
                Ok(SpeakOutcome::Cancelled) => {
                    http_response(200, "OK", r#"{"ok":true,"cancelled":true}"#)
                }
                Err((status, error)) => {
                    log::error!("[Control] Speak failed: {}", error);
                    http_response(status, "Error", &json_error(&error))
                }
            }
        }
        Err((status, message)) => http_response(status, "Error", &json_error(&message)),
    };

    let _ = stream.write_all(response.as_bytes());
}

enum RequestState {
    Incomplete,
    Complete,
    TooLarge,
}

/// Run one speak, optionally blocking until playback finishes. `speak_now`
/// resolves at playback START (the webview plays asynchronously), so the wait
/// rides on the frontend's `playback-finished` signal. User stop and synthesis
/// abort also terminate playback, so a wait timeout is not an error.
async fn handle_speak(
    app: AppHandle,
    request: SpeakRequest,
    wait_lock: Arc<tokio::sync::Mutex<()>>,
) -> Result<SpeakOutcome, (u16, String)> {
    let wait = request.wait.unwrap_or(false);
    let request_id = request.request_id.clone();
    run_speak(
        request_id.as_deref(),
        wait,
        &wait_lock,
        move |seq| speak(app, request, seq),
        |generation| {
            crate::playback_signal::wait_until(
                generation,
                Duration::from_secs(SPEAK_WAIT_TIMEOUT_SECS),
            )
        },
    )
    .await
}

/// The request lifecycle behind `/speak`, with synthesis (`speak`) and the
/// playback-finished wait (`wait_finished`) injected so it can be tested
/// without a webview. `speak` receives the request's sequence (or
/// `NO_REQUEST`); `wait_finished` receives the playback generation taken
/// before the speak and resolves true on a finish, false on its timeout.
async fn run_speak<S, SF, W, WF>(
    request_id: Option<&str>,
    wait: bool,
    wait_lock: &tokio::sync::Mutex<()>,
    speak: S,
    wait_finished: W,
) -> Result<SpeakOutcome, (u16, String)>
where
    S: FnOnce(u64) -> SF,
    SF: Future<Output = Result<(), String>>,
    W: FnOnce(u64) -> WF + Send + 'static,
    WF: Future<Output = bool> + Send + 'static,
{
    let ticket = match request_id {
        None => None,
        Some(id) => match speak_requests::register(id) {
            Registration::Proceed(seq) => Some(Ticket::new(seq)),
            Registration::Cancelled => {
                log::info!("[Control] Speak {} was stopped before it arrived", id);
                return Ok(SpeakOutcome::Cancelled);
            }
            Registration::Duplicate => {
                return Err((409, format!("request_id {} is already in use", id)));
            }
        },
    };
    let seq = ticket.as_ref().map_or(NO_REQUEST, Ticket::seq);

    // Serialize blocking speaks, but stay stoppable while queued: the lock
    // future is kept across cancellation polls, so it keeps its queue place.
    let _wait_guard = if wait {
        match speak_requests::until_cancelled(seq, wait_lock.lock()).await {
            Some(guard) => Some(guard),
            None => return Ok(SpeakOutcome::Cancelled),
        }
    } else {
        None
    };
    let generation = crate::playback_signal::generation();
    let spoken = speak(seq).await;
    // Created once and kept: after a cancel it still has to absorb the finish
    // of the stop (see `settle_cancelled`).
    let mut finished = Box::pin(wait_finished(generation));
    // A cancelled synthesis may surface as an error ("Reading cancelled");
    // either way the request was stopped, not failed.
    if let Some(ticket) = ticket.as_ref().filter(|t| t.is_cancelled()) {
        return Ok(settle_cancelled(ticket, &mut finished).await);
    }
    spoken.map_err(|error| (500, error))?;

    if wait {
        match speak_requests::until_cancelled(seq, &mut finished).await {
            None => {
                let ticket = ticket.as_ref().expect("only a request id can be cancelled");
                Ok(settle_cancelled(ticket, &mut finished).await)
            }
            Some(finished) => {
                if !finished {
                    log::warn!(
                        "[Control] Speak wait timed out after {}s; answering anyway",
                        SPEAK_WAIT_TIMEOUT_SECS
                    );
                }
                Ok(SpeakOutcome::Spoken)
            }
        }
    } else {
        // Answer at playback start, but keep the request stoppable until its
        // playback ends; the ticket's drop then marks it finished, so a stop
        // that arrives later is a no-op.
        if let Some(ticket) = ticket {
            tauri::async_runtime::spawn(async move {
                if speak_requests::until_cancelled(ticket.seq(), &mut finished)
                    .await
                    .is_none()
                {
                    settle_cancelled(&ticket, &mut finished).await;
                }
                drop(ticket);
            });
        }
        Ok(SpeakOutcome::Spoken)
    }
}

/// Upper bound for absorbing the `playback-finished` a scoped stop causes.
/// The webview answers a stop within milliseconds; the bound only guards a
/// webview that never answers.
const STOP_SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// Finish a cancelled request. When its cancel stopped playback it owned, wait
/// (bounded) for the webview's `playback-finished` for that stop: answering
/// first would let the stale finish satisfy the next request's wait, since
/// waits match finishes by timestamp, releasing that caller mid-playback.
async fn settle_cancelled<F: Future<Output = bool> + Unpin>(
    ticket: &Ticket,
    finished: &mut F,
) -> SpeakOutcome {
    if ticket.stopped_playback()
        && tokio::time::timeout(STOP_SETTLE_TIMEOUT, finished)
            .await
            .is_err()
    {
        log::warn!(
            "[Control] No playback-finished within {:?} of a scoped stop",
            STOP_SETTLE_TIMEOUT
        );
    }
    SpeakOutcome::Cancelled
}

fn request_state(buffer: &[u8]) -> RequestState {
    let Some(header_end) = find_header_end(buffer) else {
        if buffer.len() > MAX_BODY_BYTES {
            return RequestState::TooLarge;
        }
        return RequestState::Incomplete;
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let content_length = content_length(&headers).unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return RequestState::TooLarge;
    }
    if buffer.len() >= header_end + 4 + content_length {
        RequestState::Complete
    } else {
        RequestState::Incomplete
    }
}

fn json_error(message: &str) -> String {
    let value = serde_json::json!({ "error": message });
    value.to_string()
}

fn content_length(headers: &str) -> Option<usize> {
    headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
}

fn parse_request(buffer: &[u8]) -> Result<ControlRequest, (u16, String)> {
    let header_end = find_header_end(buffer).ok_or((400, "missing HTTP headers".to_string()))?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or_default();
    if request_line.starts_with("GET /health ") {
        return Ok(ControlRequest::Health);
    }
    let is_stop = request_line.starts_with("POST /stop ");
    if !is_stop && !request_line.starts_with("POST /speak ") {
        return Err((
            404,
            "expected GET /health, POST /stop or POST /speak".to_string(),
        ));
    }

    let content_length = content_length(&headers).unwrap_or(0);
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err((400, "incomplete body".to_string()));
    }
    let body = &buffer[body_start..body_end];

    if is_stop {
        // A bodyless stop stays the legacy global stop.
        let stop: StopRequest = if body.iter().all(u8::is_ascii_whitespace) {
            StopRequest::default()
        } else {
            serde_json::from_slice(body)
                .map_err(|error| (400, format!("invalid JSON: {}", error)))?
        };
        if let Some(id) = stop.request_id.as_deref() {
            speak_requests::validate_id(id).map_err(|error| (400, error))?;
        }
        return Ok(ControlRequest::Stop(stop));
    }

    let request: SpeakRequest =
        serde_json::from_slice(body).map_err(|error| (400, format!("invalid JSON: {}", error)))?;
    if request.text.trim().is_empty() {
        return Err((400, "text is required".to_string()));
    }
    if let Some(id) = request.request_id.as_deref() {
        speak_requests::validate_id(id).map_err(|error| (400, error))?;
    }
    Ok(ControlRequest::Speak(request))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn speak(app: AppHandle, request: SpeakRequest, request_seq: u64) -> Result<(), String> {
    if request.engine.is_some() || request.effect.is_some() {
        let config_state: State<Mutex<AppConfig>> = app.state();
        let mut cfg = config_state.lock().map_err(|e| e.to_string())?;
        if let Some(engine) = request.engine.as_deref() {
            let _ = parse_engine(engine)?;
        }
        if let Some(effect) = request.effect.as_deref() {
            let effect_id = parse_effect(effect)?;
            // Apply effect to the active profile
            let active_id = cfg.tts.active_profile_id.clone();
            if let Some(profile) = cfg.tts.profiles.iter_mut().find(|p| p.id == active_id) {
                profile.effects.enabled = effect_id != EffectId::None;
                profile.effects.active_effect = effect_id;
            }
        }
        config::save(&cfg)?;
    }

    let config_state: State<Mutex<AppConfig>> = app.state();
    // speak_now sanitizes; only the length cap is applied here.
    let text = {
        let cfg = config_state.lock().map_err(|e| e.to_string())?;
        let text = request.text;
        if text.chars().count() > cfg.trigger.max_text_length as usize {
            text.chars()
                .take(cfg.trigger.max_text_length as usize)
                .collect()
        } else {
            text
        }
    };

    let player: State<Mutex<AudioPlayer>> = app.state();
    let history: State<Mutex<HistoryLog>> = app.state();
    let telemetry_state: State<Mutex<telemetry::TelemetryLog>> = app.state();
    // Control-server speaks never surface in pages (no browser trigger).
    crate::commands::speak_now_for_request(
        app.clone(),
        config_state,
        player,
        history,
        telemetry_state,
        text,
        request.post_process.unwrap_or(false),
        request_seq,
    )
    .await
}

/// Whether a `host:port` socket address targets loopback.
/// Unresolvable or malformed addresses fail closed (refuse to bind).
pub fn is_loopback_addr(addr: &str) -> bool {
    let host = addr.rsplit_once(':').map(|(host, _)| host).unwrap_or(addr);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if urls_like_local_name(host) {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

fn urls_like_local_name(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
}

fn parse_engine(engine: &str) -> Result<TtsEngine, String> {
    match engine.to_ascii_lowercase().as_str() {
        "cartesia" => Ok(TtsEngine::Cartesia),
        "fish" => Ok(TtsEngine::Fish),
        "xai" => Ok(TtsEngine::Xai),
        "mistral" => Ok(TtsEngine::Mistral),
        "openai" => Ok(TtsEngine::OpenAI),
        "elevenlabs" | "eleven_labs" => Ok(TtsEngine::ElevenLabs),
        "local" => Ok(TtsEngine::Local),
        _ => Err(format!("unsupported engine: {}", engine)),
    }
}

fn parse_effect(effect: &str) -> Result<EffectId, String> {
    match effect.to_ascii_lowercase().as_str() {
        "none" | "" => Ok(EffectId::None),
        "walkie_talkie" => Ok(EffectId::WalkieTalkie),
        "game_boy" => Ok(EffectId::GameBoy),
        "cyberpunk" => Ok(EffectId::Cyberpunk),
        _ => Err(format!("unsupported effect: {}", effect)),
    }
}

fn http_response(status: u16, reason: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        reason,
        body.len(),
        body
    )
}

#[cfg(test)]
mod tests {
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

    fn stop_request(body: &str) -> Vec<u8> {
        format!(
            "POST /stop HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
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
    fn speak_request_parses_request_id() {
        let request =
            serde_json::json!({ "text": "hello", "request_id": "hermes-1", "wait": true });
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

    use std::sync::atomic::{AtomicUsize, Ordering};
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

    /// When the cancel stopped this request's own playback, the webview's
    /// `playback-finished` for that stop is still in flight. Answering before
    /// it lands would let it satisfy the NEXT request's wait (generations
    /// are timestamps), releasing that caller while its audio still plays.
    /// The cancelled request must absorb its own finish first.
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
                // The reading started: it owns the speakers.
                speak_requests::claim_playback(seq, || false, || {});
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
}
