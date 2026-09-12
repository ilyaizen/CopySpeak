// Browser companion bridge: named pipe server accepting native host connections.
// Forwards protocol messages between browser extension and desktop playback.
//
// Word-highlight pipeline (all exact, no fuzzy matching):
//   raw selection (extension UTF-16 space)
//     → sanitize_text → spoken text          [TextAlignment: raw ↔ spoken words]
//     → paginate_text → fragment table        [per-fragment UTF-16 spans]
//     → synthesis events carry per-fragment CaptionAlignment (fragment-local)
//     → frontend reports (fragment_index, position_ms) from the audible clock
//   word = captions word at position → fragment-local UTF-16
//        → + fragment.text_start = spoken-page UTF-16
//        → TextAlignment.source_span_utf16 = raw UTF-16 sent to the extension.
//
// Degradation is per word and per fragment, never per reading: a word the
// sanitizer invented simply has no source span, and a fragment whose engine
// sent no usable captions falls back to passage-only while the rest of the
// reading keeps highlighting.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};

use crate::sanitize::tts_normalize::punct_equivalent;
use crate::text_map::{align, TextAlignment};
use crate::tts::captions::{CaptionAlignment, WordTiming};

const PIPE_NAME: &str = r"\\.\pipe\copyspeak-browser";
const MAX_FRAME: usize = 256 * 1024;
/// Sentinel for "no active fragment".
const NO_FRAGMENT: usize = usize::MAX;
/// How often the state ticker re-evaluates the projected word.
const TICK_MS: u64 = 50;

/// Lifecycle status, stored as u8 so it can live in an AtomicU8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionStatus {
    Buffering = 0,
    Playing = 1,
    Paused = 2,
    Completed = 3,
    Cancelled = 4,
    Error = 5,
}

impl SessionStatus {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => SessionStatus::Buffering,
            1 => SessionStatus::Playing,
            2 => SessionStatus::Paused,
            3 => SessionStatus::Completed,
            4 => SessionStatus::Cancelled,
            5 => SessionStatus::Error,
            _ => return None,
        })
    }

    fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Buffering => "buffering",
            SessionStatus::Playing => "playing",
            SessionStatus::Paused => "paused",
            SessionStatus::Completed => "completed",
            SessionStatus::Cancelled => "cancelled",
            SessionStatus::Error => "error",
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            SessionStatus::Completed | SessionStatus::Cancelled | SessionStatus::Error
        )
    }
}

/// One pagination tile of the spoken text, with its UTF-16 span.
#[derive(Debug, Clone)]
struct FragmentEntry {
    /// UTF-16 [start, end) of the fragment text inside the spoken text.
    text_start: usize,
    text_end: usize,
    /// The exact text the pagination pass produced for this fragment.
    expected_text: String,
    /// Caption alignment received from synthesis events (fragment-local).
    captions: Option<CaptionAlignment>,
    /// True once captions arrived and their text matched `expected_text`.
    captions_ok: bool,
}

struct BrowserSession {
    reading_id: String,
    sequence: AtomicU64,
    /// Sanitized text CopySpeak actually speaks.
    spoken_text: String,
    /// Raw ↔ spoken word alignment; always present.
    source_map: TextAlignment,
    fragments: Mutex<Vec<FragmentEntry>>,
    active_fragment: AtomicUsize,
    /// Audible-clock position inside the active fragment (ms).
    position_ms: AtomicU64,
    status: AtomicU8,
    cancelled: AtomicBool,
}

// HANDLE wrapper for Send+Sync
#[derive(Clone)]
struct SafeHandle(HANDLE);
unsafe impl Send for SafeHandle {}
unsafe impl Sync for SafeHandle {}

/// True when a failed I/O call means "started, still pending" (ERROR_IO_PENDING).
fn is_pending(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(997)
}

/// Run one blocking overlapped READ to completion; returns the bytes read.
/// An empty result means the peer disconnected (broken pipe / EOF).
/// Overlapped I/O is what lets the state ticker write while this read is
/// parked: sync-mode handles allow only ONE outstanding I/O per file object,
/// which deadlocked the duplex relay (2026-09-09).
unsafe fn overlapped_read(handle: HANDLE) -> std::io::Result<Vec<u8>> {
    let event = CreateEventW(None, true, false, None)
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event;

    let mut chunk = vec![0u8; MAX_FRAME];
    let mut got: u32 = 0;
    let result = ReadFile(
        handle,
        Some(chunk.as_mut_slice()),
        Some(&mut got),
        Some(&mut overlapped),
    );
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        if !is_pending(&err) {
            return Err(err);
        }
    }
    if WaitForSingleObject(event, INFINITE) != WAIT_OBJECT_0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut moved: u32 = 0;
    if GetOverlappedResult(handle, &overlapped, &mut moved, false).is_err() {
        return Err(std::io::Error::last_os_error());
    }
    chunk.truncate(moved as usize);
    Ok(chunk)
}

/// Run one blocking overlapped WRITE to completion.
unsafe fn overlapped_write(handle: HANDLE, buf: &[u8]) -> std::io::Result<()> {
    let event = CreateEventW(None, true, false, None)
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event;

    let mut written: u32 = 0;
    let result = WriteFile(handle, Some(buf), Some(&mut written), Some(&mut overlapped));
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        if !is_pending(&err) {
            return Err(err);
        }
    }
    if WaitForSingleObject(event, INFINITE) != WAIT_OBJECT_0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut moved: u32 = 0;
    if GetOverlappedResult(handle, &overlapped, &mut moved, false).is_err() {
        return Err(std::io::Error::last_os_error());
    }
    if moved as usize != buf.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WriteZero,
            "Short overlapped write",
        ));
    }
    Ok(())
}

pub struct BrowserBridge {
    sessions: Mutex<HashMap<String, Arc<BrowserSession>>>,
    pipe_handle: Mutex<Option<SafeHandle>>,
    automatic_mode: AtomicBool,
}

impl BrowserBridge {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            pipe_handle: Mutex::new(None),
            automatic_mode: AtomicBool::new(false),
        }
    }
}

pub fn start_browser_bridge(app: AppHandle) -> Result<(), String> {
    let bridge = app.state::<BrowserBridge>();
    let pipe_name_wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    // Create named pipe with per-user ACL (default security descriptor).
    let handle = unsafe {
        let h = CreateNamedPipeW(
            windows::core::PCWSTR(pipe_name_wide.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
            windows::Win32::System::Pipes::NAMED_PIPE_MODE(0),
            1,
            MAX_FRAME as u32,
            MAX_FRAME as u32,
            0,
            None,
        );
        if h.is_invalid() {
            return Err(format!(
                "Failed to create browser pipe: {}",
                std::io::Error::last_os_error()
            ));
        }
        h
    };

    *bridge.pipe_handle.lock().unwrap() = Some(SafeHandle(handle));

    log::info!("[Browser] Named pipe server listening on {}", PIPE_NAME);

    // Synthesis events carry per-fragment captions; record them onto the
    // active browser session so word projection has exact timings.
    let app_for_events = app.clone();
    app.listen("audio-fragment-ready", move |event| {
        handle_fragment_event(&app_for_events, event.payload());
    });
    let app_for_stream = app.clone();
    app.listen("audio-stream-chunk", move |event| {
        handle_stream_chunk_event(&app_for_stream, event.payload());
    });

    // Spawn connection acceptor loop.
    let app_clone = app.clone();
    let handle_for_thread = SafeHandle(handle);
    std::thread::spawn(move || {
        if let Err(e) = accept_connections(app_clone, handle_for_thread) {
            log::error!("[Browser] Pipe server error: {}", e);
        }
    });
    Ok(())
}

/// Record captions from an `audio-fragment-ready` payload onto the active
/// session, verifying the fragment text tiles the expected spoken text.
fn handle_fragment_event(app: &AppHandle, payload: &str) {
    let Ok(event) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    let index = event
        .get("fragment_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let text = event.get("text").and_then(|v| v.as_str());
    let captions = event.get("captions").and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::from_value::<CaptionAlignment>(v.clone()).ok()
        }
    });
    if let Some(index) = index {
        record_fragment_captions(app, index, text, captions);
    }
}

/// Same for the streaming path: chunks repeat per-fragment captions.
fn handle_stream_chunk_event(app: &AppHandle, payload: &str) {
    let Ok(event) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    let index = event
        .get("fragment_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let text = event.get("text").and_then(|v| v.as_str());
    let captions = event.get("captions").and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::from_value::<CaptionAlignment>(v.clone()).ok()
        }
    });
    if let Some(index) = index {
        record_fragment_captions(app, index, text, captions);
    }
}

fn record_fragment_captions(
    app: &AppHandle,
    index: usize,
    text: Option<&str>,
    captions: Option<CaptionAlignment>,
) {
    let bridge = app.state::<BrowserBridge>();
    let sessions = bridge.sessions.lock().unwrap();
    // A single active browser session is the norm; project onto whichever
    // session is live. The tile check below rejects foreign readings' text.
    let Some(session) = sessions.values().next().cloned() else {
        return;
    };
    drop(sessions);

    let mut fragments = session.fragments.lock().unwrap();
    apply_fragment_event(&mut fragments, index, text, captions);
}

/// Core caption/tile bookkeeping for one synthesis event, as it maps onto the
/// fragment table. Pure so tests can replay the exact wire sequence.
///
/// Event shapes on the `audio-stream-chunk` wire (2026-09-09):
/// - text chunk: `text=Some(fragment text)`, `captions=None`
/// - caption item: `text=None`, `captions=Some` (ElevenLabs emits captions as
///   a standalone stream item, never paired with chunk text)
fn apply_fragment_event(
    fragments: &mut [FragmentEntry],
    index: usize,
    text: Option<&str>,
    captions: Option<CaptionAlignment>,
) {
    let Some(entry) = fragments.get_mut(index) else {
        return;
    };
    // Caption-only chunk events arrive with `text: None`, so only verify the
    // tile when an event actually carries text. An event whose text is not
    // this tile describes someone else's reading (or a post-processing
    // rewrite): ignore it, rather than degrading a session it never named.
    if text.is_some_and(|text| text != entry.expected_text) {
        return;
    }
    let Some(caps) = captions else {
        return;
    };
    match rebased_captions(caps, &entry.expected_text) {
        Some(rebased) if rebased.validate().is_ok() => {
            entry.captions = Some(rebased);
            entry.captions_ok = true;
        }
        _ => entry.captions_ok = false,
    }
}

/// Rebase caption word offsets from `caps.text` into `expected`'s coordinate
/// space when the two texts differ only in EDGE whitespace, or in same-length
/// typographic-punctuation substitutions. Two proven ElevenLabs rewrites
/// (2026-09-09, history sidecars):
/// 1. alignment text is edge-padded by one space, each pad char carrying its
///    own timing entry, so `caps.text == expected` never holds verbatim;
/// 2. curly quotes in the tile come back straight in the alignment
///    (“platform.” → "platform.").
/// Word timings are untouched — audio time is audio time; only UTF-16 text
/// offsets shift, and punctuation substitutions are 1 UTF-16 unit ↔ 1 unit,
/// so aligned positions keep indexing the same word.
///
/// Returns `None` for any other divergence: the identity-mapping policy
/// still degrades that fragment to passage-only.
fn rebased_captions(caps: CaptionAlignment, expected: &str) -> Option<CaptionAlignment> {
    // Fast path: engines whose alignment text is already the exact fragment.
    if caps.text == expected {
        return Some(caps);
    }
    let caps_u16: Vec<u16> = caps.text.encode_utf16().collect();
    // Leading pad width in UTF-16 units (not bytes — multibyte whitespace
    // would desync every offset; see AGENTS.md UTF-16 failure log).
    let prefix_u16 = caps
        .text
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(|ch| ch.len_utf16())
        .sum();
    let trimmed = caps.text.trim();
    if trimmed != expected {
        // Interior rewrite: allowed only when same-length and confined to
        // punctuation-equivalent units, so offsets stay position-aligned.
        let expected_u16: Vec<u16> = expected.encode_utf16().collect();
        let content = caps_u16
            .get(prefix_u16..prefix_u16 + expected_u16.len())
            .or_else(|| caps_u16.get(prefix_u16..))?;
        if content.len() != expected_u16.len()
            || !content
                .iter()
                .zip(&expected_u16)
                .all(|(c, e)| c == e || punct_equivalent(*c, *e))
        {
            return None;
        }
    }
    let prefix = prefix_u16 as i64;
    let content_end = prefix + expected.encode_utf16().count() as i64;
    let mut words = Vec::with_capacity(caps.words.len());
    for w in caps.words {
        let (s, e) = (w.text_start as i64, w.text_end as i64);
        if e <= prefix || s >= content_end {
            continue; // pad-only "word" (the pad chars get their own timings)
        }
        if s < prefix || e > content_end {
            return None; // word crosses the pad boundary: unexpected shape
        }
        words.push(WordTiming {
            text_start: (s - prefix) as usize,
            text_end: (e - prefix) as usize,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
        });
    }
    Some(CaptionAlignment {
        text: expected.to_string(),
        words,
    })
}

fn accept_connections(app: AppHandle, pipe: SafeHandle) -> Result<(), String> {
    loop {
        unsafe {
            // Wait for client connection (overlapped; handle is FILE_FLAG_OVERLAPPED).
            let event = match CreateEventW(None, true, false, None) {
                Ok(e) => e,
                Err(e) => return Err(format!("CreateEventW failed: {}", e)),
            };
            let mut overlapped = OVERLAPPED::default();
            overlapped.hEvent = event;
            let connected = ConnectNamedPipe(pipe.0, Some(&mut overlapped));
            if connected.is_err() {
                let error = std::io::Error::last_os_error();
                let raw = error.raw_os_error().unwrap_or(0);
                if raw != 997 && raw != 0 {
                    // Not ERROR_IO_PENDING / ERROR_PIPE_CONNECTED: real failure.
                    log::warn!("[Browser] ConnectNamedPipe failed: {}", error);
                    continue;
                }
            }
            if WaitForSingleObject(event, INFINITE) != WAIT_OBJECT_0 {
                log::warn!(
                    "[Browser] Connect wait failed: {}",
                    std::io::Error::last_os_error()
                );
                continue;
            }

            log::info!("[Browser] Native host connected");

            // Handle messages until disconnect.
            if let Err(e) = handle_session(app.clone(), pipe.clone()) {
                log::warn!("[Browser] Session error: {}", e);
            }

            DisconnectNamedPipe(pipe.0).ok();
        }
    }
}

fn handle_session(app: AppHandle, pipe: SafeHandle) -> Result<(), String> {
    // Byte-stream framing: reads may coalesce multiple frames or split one.
    // Accumulate into `buffer`, then parse every complete frame available,
    // keeping the remainder for the next read.
    let mut buffer: Vec<u8> = Vec::with_capacity(MAX_FRAME);
    loop {
        // Overlapped read: the state ticker must be able to write to this
        // handle while the read is parked (sync handles allow one outstanding
        // I/O per file object and deadlocked the duplex relay).
        let chunk = unsafe { overlapped_read(pipe.0).map_err(|e| e.to_string())? };
        if chunk.is_empty() {
            log::info!("[Browser] Native host disconnected");
            return Ok(());
        }
        buffer.extend_from_slice(&chunk);

        loop {
            if buffer.len() < 4 {
                break;
            }
            let length = u32::from_le_bytes(buffer[..4].try_into().unwrap()) as usize;
            if length > MAX_FRAME - 4 {
                return Err("Frame size out of bounds".into());
            }
            if buffer.len() < 4 + length {
                break;
            }
            let message: Value = serde_json::from_slice(&buffer[4..4 + length])
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            buffer.drain(..4 + length);
            handle_message(app.clone(), &pipe, message)?;
        }
    }
}

fn handle_message(app: AppHandle, pipe: &SafeHandle, message: Value) -> Result<(), String> {
    let bridge = app.state::<BrowserBridge>();

    // Validate protocol.
    let v = message.get("v").and_then(|v| v.as_u64());
    if v != Some(1) {
        send_error(pipe, "Unsupported protocol version")?;
        return Ok(());
    }

    let msg_type = message
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("Missing message type")?;

    match msg_type {
        "hello" => {
            let automatic = message
                .get("automatic")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            bridge.automatic_mode.store(automatic, Ordering::Relaxed);
            log::info!("[Browser] Hello: automatic={}", automatic);
        }
        "start" | "capture" => {
            let request_id = message
                .get("request_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing request_id")?
                .to_string();
            let raw_text = message
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing text")?
                .to_string();
            log::info!(
                "[Browser] Start: request_id={} chars={}",
                request_id,
                raw_text.chars().count()
            );

            if raw_text.trim().is_empty() || raw_text.encode_utf16().count() > 65536 {
                send_rejected(&pipe, &request_id, "Invalid text")?;
                return Ok(());
            }

            start_browser_reading(app, pipe, request_id, raw_text)?;
        }
        "control" => {
            let reading_id = message
                .get("reading_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing reading_id")?;
            let action = message
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or("Missing action")?;

            match action {
                "pause" | "resume" => {
                    let _ = app.emit("playback-toggle-pause", ());
                }
                "stop" => {
                    let _ = app.emit("playback-stop", ());
                    // The playback store also reports the cancelled status via
                    // browser_reading_finished; cancel eagerly here so the
                    // session ends even if the webview is unresponsive.
                    let session_arc = {
                        let sessions = bridge.sessions.lock().unwrap();
                        sessions.get(reading_id).cloned()
                    };
                    if let Some(session) = session_arc {
                        finish_session(&app, &session, SessionStatus::Cancelled);
                    }
                }
                _ => {
                    log::warn!("[Browser] Unknown control action: {}", action);
                }
            }
        }
        "snapshot" => {
            let reading_id = message
                .get("reading_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing reading_id")?;

            let snapshot = {
                let sessions = bridge.sessions.lock().unwrap();
                sessions
                    .get(reading_id)
                    .map(|s| (s.clone(), project_state(s)))
            };
            if let Some((session, (status, word, reason))) = snapshot {
                let seq = session.sequence.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = send_state(&bridge, reading_id, seq, status, word, reason);
            }
        }
        _ => {
            log::warn!("[Browser] Unknown message type: {}", msg_type);
        }
    }

    Ok(())
}

/// Create the reading session, then trigger synthesis via speak_queued.
fn start_browser_reading(
    app: AppHandle,
    pipe: &SafeHandle,
    request_id: String,
    raw_text: String,
) -> Result<(), String> {
    let bridge = app.state::<BrowserBridge>();

    // Sanitize exactly like a desktop reading; the browser extension keeps
    // highlighting the RAW selection through the exact whitespace map.
    let spoken_text = {
        let config: State<Mutex<crate::config::AppConfig>> = app.state();
        let sanitization_config = config.lock().unwrap().sanitization.clone();
        crate::sanitize::sanitize_text(&raw_text, &sanitization_config)
    };
    if spoken_text.trim().is_empty() {
        send_rejected(pipe, &request_id, "Text is empty after sanitization")?;
        return Ok(());
    }
    let source_map = align(&raw_text, &spoken_text);

    // Pre-paginate with the same config speak_queued will use, so synthesis
    // events can be verified tile-by-tile against this table.
    let pagination_config = {
        let config: State<Mutex<crate::config::AppConfig>> = app.state();
        let cfg = config.lock().unwrap();
        cfg.pagination.clone()
    };
    let paginated = crate::pagination::paginate_text(&spoken_text, &pagination_config);
    // Tiles come straight from pagination's exact UTF-16 source spans.
    let mut fragments = Vec::with_capacity(paginated.len());
    for fragment in &paginated {
        fragments.push(FragmentEntry {
            text_start: fragment.source_start,
            text_end: fragment.source_end,
            expected_text: fragment.text.clone(),
            captions: None,
            captions_ok: false,
        });
    }
    let reading_id = format!("browser-{}", request_id);

    let session = Arc::new(BrowserSession {
        reading_id: reading_id.clone(),
        sequence: AtomicU64::new(0),
        spoken_text,
        source_map,
        fragments: Mutex::new(fragments),
        active_fragment: AtomicUsize::new(NO_FRAGMENT),
        position_ms: AtomicU64::new(0),
        status: AtomicU8::new(SessionStatus::Buffering as u8),
        cancelled: AtomicBool::new(false),
    });

    bridge
        .sessions
        .lock()
        .unwrap()
        .insert(reading_id.clone(), session.clone());

    // Tell the frontend a browser reading is active so the playback store
    // reports audible-clock positions to us.
    let _ = app.emit("browser-reading", json!({ "active": true }));

    // Send acceptance.
    send_accepted(pipe, &request_id, &reading_id)?;

    // Start the state ticker before synthesis: speak_queued only returns once
    // the whole reading is generated, and the ticker is the only writer of
    // `state` frames, so starting it later would pin the panel at "buffering"
    // for the entire generation even while audio already plays.
    spawn_state_ticker(app.clone(), session.clone());

    // Spawn synthesis via the existing speak_queued command.
    let app_clone = app.clone();
    let reading_id_clone = reading_id.clone();
    let text_clone = session.spoken_text.clone();
    tauri::async_runtime::spawn(async move {
        let config: State<Mutex<crate::config::AppConfig>> = app_clone.state();
        let player: State<Mutex<crate::audio::AudioPlayer>> = app_clone.state();
        let history: State<Mutex<crate::history::HistoryLog>> = app_clone.state();
        let queue: State<Mutex<crate::fragment_queue::FragmentQueue>> = app_clone.state();
        let telemetry_state: State<Mutex<crate::telemetry::TelemetryLog>> = app_clone.state();

        let result = crate::commands::speak_queued(
            app_clone.clone(),
            config,
            player,
            history,
            queue,
            telemetry_state,
            Some(text_clone),
        )
        .await;

        if let Err(e) = result {
            log::error!("[Browser] speak_queued failed: {}", e);
            let bridge = app_clone.state::<BrowserBridge>();
            let session_arc = {
                let sessions = bridge.sessions.lock().unwrap();
                sessions.get(&reading_id_clone).cloned()
            };
            if let Some(session) = session_arc {
                finish_session(&app_clone, &session, SessionStatus::Error);
            }
        }
    });

    Ok(())
}

/// Frontend audible-clock progress for the active browser reading.
#[tauri::command]
pub async fn browser_reading_progress(
    app: AppHandle,
    fragment_index: usize,
    position_ms: u64,
    paused: bool,
) -> Result<(), String> {
    let bridge = app.state::<BrowserBridge>();
    let sessions = bridge.sessions.lock().unwrap();
    let Some(session) = sessions.values().next().cloned() else {
        return Ok(());
    };
    drop(sessions);

    session.position_ms.store(position_ms, Ordering::Relaxed);
    session
        .active_fragment
        .store(fragment_index, Ordering::Relaxed);
    let current = SessionStatus::from_u8(session.status.load(Ordering::Relaxed));
    if paused {
        if current == Some(SessionStatus::Playing) || current == Some(SessionStatus::Buffering) {
            session
                .status
                .store(SessionStatus::Paused as u8, Ordering::Relaxed);
        }
    } else if current == Some(SessionStatus::Buffering) || current == Some(SessionStatus::Paused) {
        session
            .status
            .store(SessionStatus::Playing as u8, Ordering::Relaxed);
    }
    Ok(())
}

/// Frontend reports the reading reached a terminal state.
#[tauri::command]
pub async fn browser_reading_finished(app: AppHandle, status: String) -> Result<(), String> {
    let session_status = match status.as_str() {
        "completed" => SessionStatus::Completed,
        _ => SessionStatus::Cancelled,
    };
    let bridge = app.state::<BrowserBridge>();
    let sessions = bridge.sessions.lock().unwrap();
    let Some(session) = sessions.values().next().cloned() else {
        return Ok(());
    };
    drop(sessions);
    finish_session(&app, &session, session_status);
    Ok(())
}

/// Push a terminal state, notify the frontend, and drop the session.
/// Idempotent: only the first terminal transition wins (via `cancelled`).
fn finish_session(app: &AppHandle, session: &Arc<BrowserSession>, status: SessionStatus) {
    if session.cancelled.swap(true, Ordering::Relaxed) {
        return;
    }
    session.status.store(status as u8, Ordering::Relaxed);
    let _ = app.emit("browser-reading", json!({ "active": false }));

    let bridge = app.state::<BrowserBridge>();
    let seq = session.sequence.fetch_add(1, Ordering::Relaxed) + 1;
    // Terminal frames report the reading's final highlight verdict.
    let reason = degrade_reason(session);
    let _ = send_state(
        &bridge,
        &session.reading_id,
        seq,
        status.as_str(),
        None,
        reason,
    );
    bridge.sessions.lock().unwrap().remove(&session.reading_id);
}

/// Background loop that pushes `state` messages whenever the projected word
/// or the playback status changes.
fn spawn_state_ticker(app: AppHandle, session: Arc<BrowserSession>) {
    std::thread::spawn(move || {
        let bridge = app.state::<BrowserBridge>();
        let mut last_word: Option<Option<(usize, usize)>> = None;
        let mut last_status: Option<u8> = None;
        // Outer None = nothing logged yet; inner = current degrade reason.
        let mut last_reason: Option<Option<&'static str>> = None;
        loop {
            if session.cancelled.load(Ordering::Relaxed) {
                return;
            }
            let status_u8 = session.status.load(Ordering::Relaxed);
            let (status, word, reason) = project_state(&session);
            if last_reason.as_ref() != Some(&reason) {
                match reason {
                    Some(why) => {
                        log::info!("[Browser] Word highlighting degraded: reason={}", why)
                    }
                    None => log::info!("[Browser] Word highlighting available"),
                }
                last_reason = Some(reason);
            }
            let changed = last_status != Some(status_u8) || last_word.as_ref() != Some(&word);
            if changed {
                let seq = session.sequence.fetch_add(1, Ordering::Relaxed) + 1;
                if send_state(
                    &bridge,
                    &session.reading_id,
                    seq,
                    status,
                    word.clone(),
                    reason,
                )
                .is_err()
                {
                    // Pipe is gone; stop ticking. The next connection re-snapshots.
                    return;
                }
                last_status = Some(status_u8);
                last_word = Some(word);
            }
            if SessionStatus::from_u8(status_u8).is_some_and(|s| s.is_terminal()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(TICK_MS));
        }
    });
}

/// Why word highlighting degrades to passage-only, judged for the fragment
/// that is currently audible: its engine sent no captions, or captions that
/// did not match the fragment's own text. `None` = word highlighting
/// available. Single source for the `word_available` verdict.
///
/// The verdict is per fragment, not per reading: one caption-less fragment
/// must not switch highlighting off for the fragments around it.
fn degrade_reason(session: &BrowserSession) -> Option<&'static str> {
    let index = session.active_fragment.load(Ordering::Relaxed);
    if index == NO_FRAGMENT {
        // Nothing audible yet — no verdict to report.
        return None;
    }
    let fragments = session.fragments.lock().unwrap();
    match fragments.get(index) {
        Some(entry) if entry.captions_ok => None,
        _ => Some("captions_unavailable"),
    }
}

/// Current (status, word-in-raw-space, degrade-reason) for a session.
/// `word_available` is derived downstream as `reason.is_none()`.
fn project_state(
    session: &BrowserSession,
) -> (&'static str, Option<(usize, usize)>, Option<&'static str>) {
    let status = SessionStatus::from_u8(session.status.load(Ordering::Relaxed))
        .unwrap_or(SessionStatus::Buffering)
        .as_str();
    let word = current_word(session);
    let reason = degrade_reason(session);
    (status, word, reason)
}

/// Projection of the audible word into the raw selection's UTF-16 space.
fn current_word(session: &BrowserSession) -> Option<(usize, usize)> {
    let fragment_index = session.active_fragment.load(Ordering::Relaxed);
    if fragment_index == NO_FRAGMENT {
        return None;
    }
    let fragments = session.fragments.lock().unwrap();
    let entry = fragments.get(fragment_index)?;
    if !entry.captions_ok {
        return None;
    }
    let captions = entry.captions.as_ref()?;
    let position_ms = session.position_ms.load(Ordering::Relaxed) as f64;
    let (local_start, local_end) = find_word_at_position(&captions.words, position_ms)?;
    // Fragment-local UTF-16 → spoken-page UTF-16 → raw UTF-16.
    let page_start = entry.text_start + local_start;
    let page_end = entry.text_start + local_end;
    if page_end > entry.text_end {
        return None;
    }
    session.source_map.source_span_utf16(page_start, page_end)
}

fn send_accepted(pipe: &SafeHandle, request_id: &str, reading_id: &str) -> Result<(), String> {
    let message = json!({
        "v": 1,
        "type": "accepted",
        "request_id": request_id,
        "reading_id": reading_id
    });
    send_frame(pipe, &message)
}

fn send_rejected(pipe: &SafeHandle, request_id: &str, reason: &str) -> Result<(), String> {
    log::info!(
        "[Browser] Rejected: request_id={} reason={}",
        request_id,
        reason
    );
    let message = json!({
        "v": 1,
        "type": "rejected",
        "request_id": request_id,
        "reason": reason
    });
    send_frame(pipe, &message)
}

fn send_state(
    bridge: &State<'_, BrowserBridge>,
    reading_id: &str,
    seq: u64,
    status: &str,
    word: Option<(usize, usize)>,
    reason: Option<&str>,
) -> Result<(), String> {
    let message = json!({
        "v": 1,
        "type": "state",
        "reading_id": reading_id,
        "seq": seq,
        "status": status,
        "word": word.map(|(start, end)| json!({"start": start, "end": end})),
        // Derived from the reason so the two can never contradict.
        "word_available": reason.is_none(),
        "word_degrade_reason": reason
    });
    if let Some(pipe_handle) = bridge.pipe_handle.lock().unwrap().as_ref() {
        return send_frame(pipe_handle, &message);
    }
    Ok(())
}

fn send_error(pipe: &SafeHandle, message: &str) -> Result<(), String> {
    let payload = json!({"v": 1, "type": "error", "message": message});
    send_frame(pipe, &payload)
}

fn send_frame(pipe: &SafeHandle, message: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(message).map_err(|e| format!("Serialize failed: {}", e))?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME - 4 {
        return Err("Frame size out of bounds".into());
    }

    let mut frame = Vec::with_capacity(4 + bytes.len());
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(&bytes);

    // Overlapped write: the session read is parked while this runs, so the
    // write MUST go through on the same overlapped handle.
    unsafe {
        overlapped_write(pipe.0, &frame).map_err(|e| format!("WriteFile failed: {}", e))?;
    }

    Ok(())
}

/// Find the word at a given playback position (ms) from caption timings.
fn find_word_at_position(
    words: &[crate::tts::captions::WordTiming],
    position_ms: f64,
) -> Option<(usize, usize)> {
    for word in words {
        if position_ms >= word.start_ms && position_ms < word.end_ms {
            return Some((word.text_start, word.text_end));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::captions::WordTiming;

    fn word(text_start: usize, text_end: usize) -> WordTiming {
        WordTiming {
            text_start,
            text_end,
            start_ms: 0.0,
            end_ms: 100.0,
        }
    }

    fn entry(text: &str, text_start: usize, text_end: usize) -> FragmentEntry {
        FragmentEntry {
            text_start,
            text_end,
            expected_text: text.to_string(),
            captions: None,
            captions_ok: false,
        }
    }

    fn session_with(
        raw: &str,
        spoken: &str,
        fragments: Vec<(&str, usize, usize)>,
        captions: Option<CaptionAlignment>,
    ) -> BrowserSession {
        let entries: Vec<FragmentEntry> = fragments
            .into_iter()
            .map(|(text, start, end)| FragmentEntry {
                captions: captions.clone(),
                captions_ok: captions.is_some(),
                ..entry(text, start, end)
            })
            .collect();
        BrowserSession {
            reading_id: "r".into(),
            sequence: AtomicU64::new(0),
            spoken_text: spoken.to_string(),
            source_map: align(raw, spoken),
            fragments: Mutex::new(entries),
            active_fragment: AtomicUsize::new(0),
            position_ms: AtomicU64::new(0),
            status: AtomicU8::new(SessionStatus::Playing as u8),
            cancelled: AtomicBool::new(false),
        }
    }

    #[test]
    fn word_projects_through_fragment_and_source_alignment() {
        let raw = "Hello world.\n\nSecond sentence";
        let spoken = "Hello world. Second sentence";
        // One fragment covering all of spoken; caption word "Second" at
        // fragment-local 13..19 (UTF-16), i.e. spoken-page 13..19.
        let captions = CaptionAlignment {
            text: spoken.to_string(),
            words: vec![word(13, 19)],
        };
        let session = session_with(
            raw,
            spoken,
            vec![(spoken, 0, spoken.encode_utf16().count())],
            Some(captions),
        );
        let (s, e) = current_word(&session).unwrap();
        assert_eq!(&raw[s..e], "Second");
    }

    #[test]
    fn repeated_words_map_to_right_occurrence() {
        let raw = "buffalo buffalo\nbuffalo";
        let spoken = "buffalo buffalo buffalo";
        let captions = CaptionAlignment {
            text: spoken.to_string(),
            words: vec![word(16, 23)],
        };
        let session = session_with(
            raw,
            spoken,
            vec![(spoken, 0, spoken.encode_utf16().count())],
            Some(captions),
        );
        let (s, e) = current_word(&session).unwrap();
        assert_eq!(s, 16);
        assert_eq!(&raw[s..e], "buffalo");
    }

    /// Regression for the 2026-09-10 live failure: one `%` in the selection
    /// expanded to " percent" during sanitization, and the old exact-replay
    /// map rejected the whole reading. Highlighting must survive it.
    #[test]
    fn sanitizer_expansion_keeps_the_reading_highlighting() {
        let raw = "Around 10-20% of measles cases result in hospitalization.";
        let spoken =
            crate::sanitize::sanitize_text(raw, &crate::config::SanitizationConfig::default());
        let start = spoken.find("measles").unwrap();
        let captions = CaptionAlignment {
            text: spoken.clone(),
            words: vec![word(start, start + "measles".len())],
        };
        let session = session_with(
            raw,
            &spoken,
            vec![(spoken.as_str(), 0, spoken.encode_utf16().count())],
            Some(captions),
        );
        let (_, projected, reason) = project_state(&session);
        assert_eq!(reason, None, "expansion must not degrade the reading");
        let (s, e) = projected.unwrap();
        assert_eq!(&raw[s..e], "measles");
    }

    #[test]
    fn missing_captions_report_captions_unavailable() {
        let spoken = "plain text";
        let session = session_with(spoken, spoken, vec![(spoken, 0, 10)], None);
        let (_, projected, reason) = project_state(&session);
        assert!(projected.is_none());
        assert_eq!(reason, Some("captions_unavailable"));
    }

    /// The caption verdict is per fragment: a fragment whose engine sent no
    /// captions must not switch highlighting off for the one being read.
    #[test]
    fn caption_verdict_follows_the_audible_fragment() {
        let spoken = "first tile second tile";
        let second = "second tile";
        let start = spoken.find(second).unwrap();
        let captions = CaptionAlignment {
            text: second.to_string(),
            words: vec![word(0, 6)],
        };
        let session = session_with(
            spoken,
            spoken,
            vec![("first tile", 0, 10), (second, start, start + second.len())],
            None,
        );
        {
            let mut fragments = session.fragments.lock().unwrap();
            fragments[1].captions = Some(captions);
            fragments[1].captions_ok = true;
        }
        assert_eq!(degrade_reason(&session), Some("captions_unavailable"));
        session.active_fragment.store(1, Ordering::Relaxed);
        assert_eq!(degrade_reason(&session), None);
        let (s, e) = current_word(&session).unwrap();
        assert_eq!(&spoken[s..e], "second");
    }

    /// Nothing is audible yet during buffering, so there is no verdict to
    /// report and the panel must not claim highlighting is unavailable.
    #[test]
    fn buffering_session_reports_no_degrade_reason() {
        let spoken = "plain text";
        let session = session_with(spoken, spoken, vec![(spoken, 0, 10)], None);
        session.active_fragment.store(NO_FRAGMENT, Ordering::Relaxed);
        let (_, projected, reason) = project_state(&session);
        assert!(projected.is_none());
        assert_eq!(reason, None);
    }

    #[test]
    fn clean_session_has_no_degrade_reason() {
        let spoken = "plain text";
        let caps = CaptionAlignment {
            text: spoken.to_string(),
            words: vec![word(0, 5)],
        };
        let session = session_with(spoken, spoken, vec![(spoken, 0, 10)], Some(caps));
        let (_, _, reason) = project_state(&session);
        assert_eq!(reason, None);
    }

    #[test]
    fn fragment_local_offsets_shift_by_tile_start() {
        let raw = "Part one here. Part two word";
        let spoken = "Part one here. Part two word";
        let f2 = "Part two word";
        let start = spoken.find(f2).unwrap();
        let captions = CaptionAlignment {
            text: f2.to_string(),
            words: vec![word(5, 8)],
        };
        let session = session_with(
            raw,
            spoken,
            vec![("Part one here.", 0, 14), (f2, start, start + f2.len())],
            Some(captions),
        );
        session.active_fragment.store(1, Ordering::Relaxed);
        let (s, e) = current_word(&session).unwrap();
        assert_eq!(&raw[s..e], "two");
    }

    /// The wire sends a text chunk (captions=None) BEFORE the caption item
    /// (text=None); the fragment only becomes usable once captions land.
    #[test]
    fn late_captions_make_the_fragment_usable() {
        let text = "hello world";
        let mut fragments = vec![entry(text, 0, text.len())];
        let caps = CaptionAlignment {
            text: text.to_string(),
            words: vec![word(0, 5)],
        };

        apply_fragment_event(&mut fragments, 0, Some(text), None);
        assert!(!fragments[0].captions_ok);
        apply_fragment_event(&mut fragments, 0, None, Some(caps));

        assert!(fragments[0].captions_ok, "late captions must land");
    }

    /// A standalone caption item (text=None) must NOT be mistaken for foreign
    /// text — the tile check runs only when an event carries text.
    #[test]
    fn caption_only_event_skips_tile_check() {
        let text = "hello world";
        let mut fragments = vec![entry(text, 0, text.len())];
        let caps = CaptionAlignment {
            text: text.to_string(),
            words: vec![word(0, 5)],
        };
        apply_fragment_event(&mut fragments, 0, None, Some(caps));

        assert!(fragments[0].captions_ok);
    }

    /// A concurrent desktop reading's events name a different tile. They are
    /// ignored, leaving captions this fragment already has intact.
    #[test]
    fn foreign_tile_events_are_ignored_not_fatal() {
        let text = "hello world";
        let mut fragments = vec![entry(text, 0, text.len())];
        let caps = CaptionAlignment {
            text: text.to_string(),
            words: vec![word(0, 5)],
        };
        apply_fragment_event(&mut fragments, 0, None, Some(caps));
        apply_fragment_event(
            &mut fragments,
            0,
            Some("a different reading"),
            Some(CaptionAlignment {
                text: "a different reading".to_string(),
                words: vec![word(0, 8)],
            }),
        );

        assert!(fragments[0].captions_ok, "foreign events must not degrade");
        assert_eq!(fragments[0].captions.as_ref().unwrap().text, text);
    }

    /// Captions whose text diverges from their own tile still degrade that
    /// fragment — the identity-mapping policy is unchanged.
    #[test]
    fn mismatched_captions_still_degrade() {
        let text = "hello world";
        let mut fragments = vec![entry(text, 0, text.len())];
        let caps = CaptionAlignment {
            text: "different text".to_string(),
            words: vec![word(0, 5)],
        };
        apply_fragment_event(&mut fragments, 0, Some(text), None);
        apply_fragment_event(&mut fragments, 0, Some(text), Some(caps));

        assert!(!fragments[0].captions_ok);
    }

    /// Regression for the 2026-09-09 live-click failure: ElevenLabs pads its
    /// alignment text with one leading/trailing space (each pad char gets its
    /// own timing entry), so exact-identity matching degraded every reading.
    #[test]
    fn edge_padded_captions_are_rebased_to_tile() {
        let expected = "The Great Fiction-Nonfiction Inversion";
        let padded = format!(" {expected} ");
        let content_u16 = expected.encode_utf16().count();
        let mut fragments = vec![entry(expected, 0, content_u16)];
        let caps = CaptionAlignment {
            text: padded,
            words: vec![
                // Leading pad "word" with its own timing (sidecar shape).
                WordTiming {
                    text_start: 0,
                    text_end: 1,
                    start_ms: 0.0,
                    end_ms: 58.0,
                },
                // "The" at padded offsets 1..5.
                WordTiming {
                    text_start: 1,
                    text_end: 5,
                    start_ms: 58.0,
                    end_ms: 200.0,
                },
                // Trailing pad "word".
                WordTiming {
                    text_start: content_u16 + 1,
                    text_end: content_u16 + 2,
                    start_ms: 300.0,
                    end_ms: 350.0,
                },
            ],
        };
        apply_fragment_event(&mut fragments, 0, Some(expected), None);
        apply_fragment_event(&mut fragments, 0, None, Some(caps));

        assert!(fragments[0].captions_ok, "padded captions must rebase");
        let caps = fragments[0].captions.as_ref().unwrap();
        assert_eq!(caps.text, expected);
        assert_eq!(caps.words.len(), 1, "pad-only words dropped");
        assert_eq!((caps.words[0].text_start, caps.words[0].text_end), (0, 4));
        assert_eq!(caps.words[0].start_ms, 58.0, "audio timings untouched");
    }

    /// Interior divergence (rewritten caption text) still degrades the
    /// fragment — the identity-mapping policy is unchanged.
    #[test]
    fn interior_divergent_captions_still_degrade() {
        let mut fragments = vec![entry("hello world!", 0, 12)];
        let caps = CaptionAlignment {
            text: " hello worlds! ".to_string(),
            words: vec![word(1, 6)],
        };
        apply_fragment_event(&mut fragments, 0, Some("hello world!"), None);
        apply_fragment_event(&mut fragments, 0, None, Some(caps));

        assert!(!fragments[0].captions_ok);
    }

    /// The proven fragment-2 shape: edge padding PLUS curly→straight quote
    /// normalization, same UTF-16 length, offsets stay position-aligned.
    #[test]
    fn padded_and_quote_normalized_captions_rebase() {
        let expected = "says \u{201C}platform.\u{201D} done";
        let padded = format!(
            " {}",
            expected.replace('\u{201C}', "\"").replace('\u{201D}', "\"")
        );
        let content_u16 = expected.encode_utf16().count();
        let mut fragments = vec![entry(expected, 0, content_u16)];
        let mut padded_u16 = padded.encode_utf16().collect::<Vec<_>>();
        padded_u16.push(0x20); // trailing pad
        let caps = CaptionAlignment {
            text: String::from_utf16(&padded_u16).unwrap(),
            words: vec![
                WordTiming {
                    text_start: 0,
                    text_end: 1,
                    start_ms: 0.0,
                    end_ms: 20.0,
                }, // lead pad
                WordTiming {
                    text_start: 1,
                    text_end: 1 + content_u16,
                    start_ms: 20.0,
                    end_ms: 500.0,
                },
                WordTiming {
                    text_start: 1 + content_u16,
                    text_end: 2 + content_u16,
                    start_ms: 500.0,
                    end_ms: 520.0,
                }, // trail pad
            ],
        };
        apply_fragment_event(&mut fragments, 0, Some(expected), None);
        apply_fragment_event(&mut fragments, 0, None, Some(caps));

        assert!(fragments[0].captions_ok, "quote-normalized padding rebases");
        let caps = fragments[0].captions.as_ref().unwrap();
        assert_eq!(caps.text, expected);
        assert_eq!(caps.words.len(), 1);
        assert_eq!(
            (caps.words[0].text_start, caps.words[0].text_end),
            (0, content_u16)
        );
    }
}
