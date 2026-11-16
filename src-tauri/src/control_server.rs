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
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const DEFAULT_ADDR: &str = "127.0.0.1:43117";
const MAX_BODY_BYTES: usize = 200_000;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for a blocking speak: one long paragraph can speak for
/// minutes; past this the server answers anyway (the user may have stopped
/// playback, which also resolves the wait from the client side).
const SPEAK_WAIT_TIMEOUT_SECS: u64 = 600;

// Scoped stops and the registration/interrupt decision share this lock.
static REGISTRATION_LOCK: Mutex<()> = Mutex::new(());

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

/// `POST /pause` and `POST /resume` body: the reading whose playback the
/// control targets. Scoped only, so a control can never reach another reading.
#[derive(Debug, Deserialize)]
struct ScopedControl {
    request_id: String,
}

/// `POST /seek` body: the reading to seek and the signed step in milliseconds
/// (negative rewinds, positive fast-forwards).
#[derive(Debug, Deserialize)]
struct SeekControl {
    request_id: String,
    delta_ms: i64,
}

/// Largest accepted seek step. The playback bar moves by ±5 s; the route lets
/// other local clients use up to a minute without a stray value jumping the
/// whole reading.
const MAX_SEEK_DELTA_MS: i64 = 60_000;

enum ControlRequest {
    Health,
    Status,
    Stop(StopRequest),
    Speak(SpeakRequest),
    Pause(String),
    Resume(String),
    Seek(SeekControl),
}

/// Which POST route the request line targets.
enum PostRoute {
    Stop,
    Speak,
    Pause,
    Resume,
    Seek,
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
        Ok(ControlRequest::Health) => http_response(200, "OK", r#"{"ok":true,"app":"CopySpeech"}"#),
        Ok(ControlRequest::Status) => http_response_cors(200, "OK", &status_body()),
        Ok(ControlRequest::Stop(StopRequest { request_id: None })) => {
            // Abort readings that have not reached playback as well.
            crate::do_abort_synthesis(&app);
            http_response(200, "OK", r#"{"ok":true}"#)
        }
        Ok(ControlRequest::Stop(StopRequest {
            request_id: Some(id),
        })) => {
            // Scoped stop: cancels exactly this request, whatever phase it is
            // in, and stops playback only while that request's reading is
            // audible. When the request has a reading, the webview drops that
            // reading's held run after the cancel: a waiting reading's buffered
            // audio must never play, while the audible reading keeps playing.
            let cancel_started = std::time::Instant::now();
            let outcome = {
                let _guard = REGISTRATION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                speak_requests::cancel(&id, || {
                    crate::commands::stop_all_audio(&app, &app.state::<Mutex<AudioPlayer>>());
                })
            };
            log::info!(
                "[Control] Stop for request {}: {} (handled in {} ms)",
                id,
                outcome.as_str(),
                cancel_started.elapsed().as_millis()
            );
            if let Some(reading) = speak_requests::reading_for_id(&id) {
                // After the cancel, so the store never re-plays a chunk whose
                // reading is already stopped.
                if let Err(e) = app.emit(
                    "playback-reading-dropped",
                    serde_json::json!({ "reading_id": reading }),
                ) {
                    log::warn!("Failed to emit playback-reading-dropped: {}", e);
                }
            }
            let body = serde_json::json!({
                "ok": true,
                "request_id": id,
                "outcome": outcome.as_str(),
            });
            http_response(200, "OK", &body.to_string())
        }
        Ok(ControlRequest::Pause(request_id)) => {
            // Scoped control: applies only while this request owns the
            // speakers. A reading that is still synthesizing or queued gets a
            // bounded wait, so a pause pressed during buffering lands on its
            // first audible chunk.
            let applied = speak_requests::dispatch_to_owner(&request_id, |_| {
                crate::commands::emit_playback_pause(&app);
            });
            http_response(200, "OK", &control_body("paused", &request_id, applied))
        }
        Ok(ControlRequest::Resume(request_id)) => {
            let applied = speak_requests::dispatch_to_owner(&request_id, |_| {
                crate::commands::emit_playback_resume(&app);
            });
            http_response(200, "OK", &control_body("resumed", &request_id, applied))
        }
        Ok(ControlRequest::Seek(SeekControl {
            request_id,
            delta_ms,
        })) => {
            let applied = speak_requests::dispatch_to_owner(&request_id, |_| {
                crate::commands::emit_playback_seek(&app, delta_ms);
            });
            http_response(200, "OK", &control_body("seeked", &request_id, applied))
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
    validate_speak_options(&request).map_err(|error| (500, error))?;
    let wait = request.wait.unwrap_or(false);
    let request_id = request.request_id.clone();
    if let Some(id) = request_id.as_deref() {
        log::info!("[Control] Speak {} received (wait: {})", id, wait);
    }
    // Interrupt mode: the newest /speak wins. Cut whatever plays (and any
    // synthesis still running for it) so this request starts at once; queue
    // mode, the default, keeps requests FIFO instead. File output plays
    // nothing, so an interrupt there would cut audio for no reading.
    let interrupt = {
        let config_state: State<Mutex<AppConfig>> = app.state();
        let cfg = config_state.lock();
        let wants_interrupt = match cfg {
            Ok(cfg) => {
                cfg.playback.on_retrigger == crate::config::RetriggerMode::Interrupt
                    && !(cfg.output.enabled && !cfg.output.directory.is_empty())
            }
            // A poisoned lock must not turn a speak into a 500; queue mode
            // is the safe default.
            Err(_) => false,
        };
        wants_interrupt
    };
    let interrupt_app = app.clone();
    let wait_request_id = request_id.clone();
    run_speak_with_interrupt(
        request_id.as_deref(),
        wait,
        &wait_lock,
        abort_generation,
        move || {
            if interrupt {
                log::info!("[Control] Interrupt mode: stopping current playback for the new speak");
                crate::do_abort_synthesis(&interrupt_app);
            }
            interrupt
        },
        move |seq, generation| speak(app, request, seq, generation),
        move |_| {
            let waiter = crate::playback_signal::PlaybackWait::new(wait_request_id.as_deref());
            async move { waiter.wait(Duration::from_secs(SPEAK_WAIT_TIMEOUT_SECS)).await }
        },
    )
    .await
}

fn validate_speak_options(request: &SpeakRequest) -> Result<(), String> {
    if let Some(engine) = request.engine.as_deref() {
        parse_engine(engine)?;
    }
    if let Some(effect) = request.effect.as_deref() {
        parse_effect(effect)?;
    }
    Ok(())
}

/// Log one `/speak` lifecycle stage that took long enough to matter for the
/// cached-replay start latency. Fast stages stay silent.
fn log_slow_stage(request_id: Option<&str>, stage: &str, started: Instant) {
    let elapsed = started.elapsed();
    if elapsed >= Duration::from_millis(20) {
        log::info!(
            "[Control] Speak {}: {} took {} ms",
            request_id.unwrap_or("-"),
            stage,
            elapsed.as_millis()
        );
    }
}

/// Test wrapper with interrupt mode disabled.
#[cfg(test)]
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
    run_speak_with_interrupt(
        request_id,
        wait,
        wait_lock,
        abort_generation,
        || false,
        move |seq, _| speak(seq),
        wait_finished,
    ).await
}

fn abort_generation() -> u64 {
    crate::ABORT_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

async fn run_speak_with_interrupt<I, S, SF, W, WF>(
    request_id: Option<&str>,
    wait: bool,
    wait_lock: &tokio::sync::Mutex<()>,
    read_generation: fn() -> u64,
    interrupt: I,
    speak: S,
    wait_finished: W,
) -> Result<SpeakOutcome, (u16, String)>
where
    I: FnOnce() -> bool,
    S: FnOnce(u64, u64) -> SF,
    SF: Future<Output = Result<(), String>>,
    W: FnOnce(u64) -> WF + Send + 'static,
    WF: Future<Output = bool> + Send + 'static,
{
    let (ticket, generation) = {
        let _guard = REGISTRATION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        let generation = read_generation();
        let generation = generation.wrapping_add(u64::from(interrupt()));
        (ticket, generation)
    };
    let seq = ticket.as_ref().map_or(NO_REQUEST, Ticket::seq);
    let globally_aborted = move || read_generation() != generation;
    // Owned for the detached non-wait bookkeeping task below, which outlives
    // this call's borrows.
    let request_label = request_id.map(str::to_owned);
    let started = Instant::now();

    // Keep the lock future's place while watching scoped and global stops.
    let _wait_guard = if wait {
        match speak_requests::until_cancelled_or_aborted(seq, globally_aborted, wait_lock.lock())
            .await
        {
            Some(guard) => {
                log_slow_stage(
                    request_label.as_deref(),
                    "wait for the previous reading",
                    started,
                );
                Some(guard)
            }
            None => return Ok(SpeakOutcome::Cancelled),
        }
    } else {
        None
    };
    // Subscribe before synthesis: cached readings can finish during it.
    let mut finished = Box::pin(wait_finished(generation));
    if globally_aborted() || speak_requests::is_cancelled(seq) {
        return Ok(SpeakOutcome::Cancelled);
    }
    let spoken = speak(seq, generation).await;
    log_slow_stage(
        request_label.as_deref(),
        "synthesis and cache replay emit",
        started,
    );
    // A scoped cancellation that stopped playback still absorbs its finish.
    // A cancelled synthesis may surface as an error ("Reading cancelled");
    // either way the request was stopped, not failed.
    if let Some(ticket) = ticket.as_ref().filter(|t| t.is_cancelled()) {
        return Ok(settle_cancelled(request_label.as_deref(), ticket, &mut finished).await);
    }
    if globally_aborted() {
        return Ok(SpeakOutcome::Cancelled);
    }
    spoken.map_err(|error| (500, error))?;

    if wait {
        match speak_requests::until_cancelled_or_aborted(seq, globally_aborted, &mut finished).await {
            None => match ticket.as_ref().filter(|t| t.is_cancelled()) {
                Some(ticket) => {
                    Ok(settle_cancelled(request_label.as_deref(), ticket, &mut finished).await)
                }
                None => Ok(SpeakOutcome::Cancelled),
            },
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
                if speak_requests::until_cancelled_or_aborted(
                    ticket.seq(),
                    globally_aborted,
                    &mut finished,
                )
                .await
                .is_none()
                    && ticket.is_cancelled()
                {
                    settle_cancelled(request_label.as_deref(), &ticket, &mut finished).await;
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

/// Wait briefly for the stopped playback's finish before releasing the
/// serialized wait; a following unscoped request still accepts any finish.
async fn settle_cancelled<F: Future<Output = bool> + Unpin>(
    request_id: Option<&str>,
    ticket: &Ticket,
    finished: &mut F,
) -> SpeakOutcome {
    let started = Instant::now();
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
    log_slow_stage(
        request_id,
        "absorbing the stop's playback-finished",
        started,
    );
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

/// Answer a scoped transport route. The outcome names the action when the
/// ownership-gated emit ran; every refused case (unknown, finished, cancelled,
/// queued or another reading's playback) answers `not_playing`.
fn control_body(action: &str, request_id: &str, applied: bool) -> String {
    let outcome = if applied { action } else { "not_playing" };
    log::info!(
        "[Control] {} for request {}: {}",
        action,
        request_id,
        outcome
    );
    serde_json::json!({
        "ok": true,
        "request_id": request_id,
        "outcome": outcome,
    })
    .to_string()
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
    if request_line.starts_with("GET /status ") {
        return Ok(ControlRequest::Status);
    }
    let route = if request_line.starts_with("POST /stop ") {
        PostRoute::Stop
    } else if request_line.starts_with("POST /speak ") {
        PostRoute::Speak
    } else if request_line.starts_with("POST /pause ") {
        PostRoute::Pause
    } else if request_line.starts_with("POST /resume ") {
        PostRoute::Resume
    } else if request_line.starts_with("POST /seek ") {
        PostRoute::Seek
    } else {
        return Err((
            404,
            "expected GET /health, GET /status, POST /stop, POST /speak, POST /pause, POST /resume or POST /seek"
                .to_string(),
        ));
    };

    let content_length = content_length(&headers).unwrap_or(0);
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err((400, "incomplete body".to_string()));
    }
    let body = &buffer[body_start..body_end];

    match route {
        PostRoute::Stop => parse_stop(body),
        PostRoute::Speak => parse_speak(body),
        PostRoute::Pause => parse_scoped_request_id(body).map(ControlRequest::Pause),
        PostRoute::Resume => parse_scoped_request_id(body).map(ControlRequest::Resume),
        PostRoute::Seek => parse_seek(body),
    }
}

/// `POST /stop`: a bodyless stop stays the legacy global stop.
fn parse_stop(body: &[u8]) -> Result<ControlRequest, (u16, String)> {
    let stop: StopRequest = if body.iter().all(u8::is_ascii_whitespace) {
        StopRequest::default()
    } else {
        serde_json::from_slice(body).map_err(|error| (400, format!("invalid JSON: {}", error)))?
    };
    if let Some(id) = stop.request_id.as_deref() {
        speak_requests::validate_id(id).map_err(|error| (400, error))?;
    }
    Ok(ControlRequest::Stop(stop))
}

fn parse_speak(body: &[u8]) -> Result<ControlRequest, (u16, String)> {
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

/// `POST /pause` and `POST /resume`: exactly one required `request_id`.
fn parse_scoped_request_id(body: &[u8]) -> Result<String, (u16, String)> {
    let control: ScopedControl =
        serde_json::from_slice(body).map_err(|error| (400, format!("invalid JSON: {}", error)))?;
    speak_requests::validate_id(&control.request_id).map_err(|error| (400, error))?;
    Ok(control.request_id)
}

/// `POST /seek`: a required `request_id` and a signed, non-zero step.
fn parse_seek(body: &[u8]) -> Result<ControlRequest, (u16, String)> {
    let seek: SeekControl =
        serde_json::from_slice(body).map_err(|error| (400, format!("invalid JSON: {}", error)))?;
    speak_requests::validate_id(&seek.request_id).map_err(|error| (400, error))?;
    if seek.delta_ms == 0 {
        return Err((400, "delta_ms must be non-zero".to_string()));
    }
    if seek.delta_ms.unsigned_abs() > MAX_SEEK_DELTA_MS as u64 {
        return Err((
            400,
            format!("delta_ms must be within -{MAX_SEEK_DELTA_MS}..={MAX_SEEK_DELTA_MS}"),
        ));
    }
    Ok(ControlRequest::Seek(seek))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn speak(
    app: AppHandle,
    request: SpeakRequest,
    request_seq: u64,
    generation: u64,
) -> Result<(), String> {
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
        generation,
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

/// `GET /status` body: the webview's last caption while it is fresh, else the
/// inactive fallback the client reads as "hide captions".
fn status_body() -> String {
    crate::playback_signal::current_caption().unwrap_or_else(|| r#"{"active":false}"#.to_string())
}

/// Response for the read-only `GET /status` route. The DSH client fetches it
/// from a different origin, so it needs an open CORS header. The shared
/// `http_response` stays unchanged for the existing routes.
fn http_response_cors(status: u16, reason: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        reason,
        body.len(),
        body
    )
}

#[cfg(test)]
#[path = "control_server_tests.rs"]
mod tests;
