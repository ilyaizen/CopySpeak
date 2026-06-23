use crate::audio::AudioPlayer;
use crate::config::{self, AppConfig, EffectId, TtsEngine};
use crate::history::HistoryLog;
use crate::telemetry;
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const DEFAULT_ADDR: &str = "127.0.0.1:43117";
const MAX_BODY_BYTES: usize = 200_000;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct SpeakRequest {
    text: String,
    engine: Option<String>,
    effect: Option<String>,
    profile: Option<String>,
    persist_selection: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ActiveProfileRequest {
    profile: String,
}

enum ControlRequest {
    Health,
    Speak(SpeakRequest),
    Profiles,
    Profile(String),
    SetActiveProfile(ActiveProfileRequest),
    Engines,
    Voices(TtsEngine),
}

pub fn start(app: AppHandle) {
    let addr = std::env::var("COPYSPEAK_CONTROL_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(&addr) {
            Ok(listener) => listener,
            Err(error) => {
                log::warn!("[Control] Failed to bind {}: {}", addr, error);
                return;
            }
        };

        log::info!("[Control] Listening on http://{}", addr);
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, app.clone()),
                Err(error) => log::warn!("[Control] Connection failed: {}", error),
            }
        }
    });
}

fn handle_connection(mut stream: TcpStream, app: AppHandle) {
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
        Ok(ControlRequest::Profiles) => json_response(profile_list(&app)),
        Ok(ControlRequest::Profile(id)) => json_response(profile_detail(&app, &id)),
        Ok(ControlRequest::SetActiveProfile(request)) => {
            match set_active_profile(&app, &request.profile) {
                Ok(()) => http_response(200, "OK", r#"{"ok":true}"#),
                Err(error) => http_response(400, "Error", &json_error(&error)),
            }
        }
        Ok(ControlRequest::Engines) => {
            json_response(Ok(serde_json::json!(crate::tts::catalog::list_engines())))
        }
        Ok(ControlRequest::Voices(engine)) => json_response(voices_for_engine(&app, &engine)),
        Ok(ControlRequest::Speak(request)) => {
            match tauri::async_runtime::block_on(speak(app.clone(), request)) {
                Ok(()) => http_response(200, "OK", r#"{"ok":true}"#),
                Err(error) => {
                    log::error!("[Control] Speak failed: {}", error);
                    http_response(500, "Error", &json_error(&error))
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
    serde_json::json!({ "error": message }).to_string()
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
    let request_line = headers.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    match (method, path) {
        ("GET", "/health") => Ok(ControlRequest::Health),
        ("GET", "/profiles") => Ok(ControlRequest::Profiles),
        ("GET", "/engines") => Ok(ControlRequest::Engines),
        ("POST", "/profiles/active") => {
            let request: ActiveProfileRequest =
                serde_json::from_slice(request_body(buffer, header_end, &headers)?)
                    .map_err(|error| (400, format!("invalid JSON: {}", error)))?;
            Ok(ControlRequest::SetActiveProfile(request))
        }
        ("POST", "/speak") => {
            let request: SpeakRequest =
                serde_json::from_slice(request_body(buffer, header_end, &headers)?)
                    .map_err(|error| (400, format!("invalid JSON: {}", error)))?;
            if request.text.trim().is_empty() {
                return Err((400, "text is required".to_string()));
            }
            Ok(ControlRequest::Speak(request))
        }
        ("GET", path) if path.starts_with("/profiles/") => Ok(ControlRequest::Profile(
            path.trim_start_matches("/profiles/").to_string(),
        )),
        ("GET", path) if path.starts_with("/engines/") && path.ends_with("/voices") => {
            let engine = path
                .trim_start_matches("/engines/")
                .trim_end_matches("/voices")
                .trim_end_matches('/');
            parse_engine(engine)
                .map(ControlRequest::Voices)
                .map_err(|error| (400, error))
        }
        _ => Err((404, "unsupported endpoint".to_string())),
    }
}

fn request_body<'a>(
    buffer: &'a [u8],
    header_end: usize,
    headers: &str,
) -> Result<&'a [u8], (u16, String)> {
    let content_length = content_length(headers).unwrap_or(0);
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err((400, "incomplete body".to_string()));
    }
    Ok(&buffer[body_start..body_end])
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn speak(app: AppHandle, request: SpeakRequest) -> Result<(), String> {
    if request.persist_selection == Some(true)
        && (request.engine.is_some() || request.effect.is_some() || request.profile.is_some())
    {
        let config_state: State<Mutex<AppConfig>> = app.state();
        let mut cfg = config_state.lock().map_err(|e| e.to_string())?;
        if let Some(profile_id) = request.profile.as_deref() {
            if !cfg.tts.profiles.iter().any(|p| p.id == profile_id) {
                return Err(format!("unknown profile: {}", profile_id));
            }
            cfg.tts.active_profile_id = profile_id.to_string();
            config::sync_active_backend_mirror(&mut cfg.tts);
        }
        if let Some(engine) = request.engine.as_deref() {
            let _ = parse_engine(engine)?;
        }
        if let Some(effect) = request.effect.as_deref() {
            let effect_id = parse_effect(effect)?;
            cfg.effects.enabled = effect_id != EffectId::None;
            cfg.effects.active_effect = effect_id;
        }
        config::save(&cfg)?;
    }

    let config_state: State<Mutex<AppConfig>> = app.state();
    let player: State<Mutex<AudioPlayer>> = app.state();
    let history: State<Mutex<HistoryLog>> = app.state();
    let telemetry_state: State<Mutex<telemetry::TelemetryLog>> = app.state();
    crate::commands::speak_now_with_profile(
        app.clone(),
        config_state,
        player,
        history,
        telemetry_state,
        Some(request.text),
        request.profile,
    )
    .await
}

fn set_active_profile(app: &AppHandle, profile_id: &str) -> Result<(), String> {
    let config_state: State<Mutex<AppConfig>> = app.state();
    let mut cfg = config_state.lock().map_err(|e| e.to_string())?;
    if !cfg
        .tts
        .profiles
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err(format!("unknown profile: {}", profile_id));
    }
    cfg.tts.active_profile_id = profile_id.to_string();
    config::sync_active_backend_mirror(&mut cfg.tts);
    config::save(&cfg)
}

fn profile_list(app: &AppHandle) -> Result<serde_json::Value, String> {
    let config_state: State<Mutex<AppConfig>> = app.state();
    let cfg = config_state.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::Value::Array(
        cfg.tts
            .profiles
            .iter()
            .map(|profile| {
                serde_json::json!({
                    "id": profile.id,
                    "name": profile.name,
                    "engine": profile.engine,
                    "voice": profile.voice,
                    "voice_label": profile.voice_label,
                    "active": profile.id == cfg.tts.active_profile_id
                })
            })
            .collect(),
    ))
}

fn profile_detail(app: &AppHandle, profile_id: &str) -> Result<serde_json::Value, String> {
    let config_state: State<Mutex<AppConfig>> = app.state();
    let cfg = config_state.lock().map_err(|e| e.to_string())?;
    cfg.tts
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| serde_json::json!(profile))
        .ok_or_else(|| format!("unknown profile: {}", profile_id))
}

fn voices_for_engine(app: &AppHandle, engine: &TtsEngine) -> Result<serde_json::Value, String> {
    if engine == &TtsEngine::ElevenLabs {
        let config_state: State<Mutex<AppConfig>> = app.state();
        let cfg = config_state.lock().map_err(|e| e.to_string())?;
        let backend = crate::tts::elevenlabs::ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());
        let voices = backend
            .list_voices()
            .map_err(|error| format!("Failed to fetch voices: {}", error))?
            .into_iter()
            .map(|voice| {
                serde_json::json!({
                    "id": voice.voice_id,
                    "label": voice.name.unwrap_or_default(),
                    "description": voice.description,
                    "preview_url": voice.preview_url
                })
            })
            .collect::<Vec<_>>();
        return Ok(serde_json::json!(voices));
    }
    Ok(serde_json::json!(crate::tts::catalog::list_static_voices(
        engine
    )))
}

fn json_response(value: Result<serde_json::Value, String>) -> String {
    match value {
        Ok(value) => http_response(200, "OK", &value.to_string()),
        Err(error) => http_response(400, "Error", &json_error(&error)),
    }
}

fn parse_engine(engine: &str) -> Result<TtsEngine, String> {
    match engine.to_ascii_lowercase().as_str() {
        "cartesia" => Ok(TtsEngine::Cartesia),
        "openai" => Ok(TtsEngine::OpenAI),
        "elevenlabs" | "eleven_labs" => Ok(TtsEngine::ElevenLabs),
        "local" => Ok(TtsEngine::Local),
        "http" => Ok(TtsEngine::Http),
        "google" | "gemini" => Ok(TtsEngine::Google),
        "microsoft" | "mai" => Ok(TtsEngine::Microsoft),
        _ => Err(format!("unsupported engine: {}", engine)),
    }
}

fn parse_effect(effect: &str) -> Result<EffectId, String> {
    match effect.to_ascii_lowercase().as_str() {
        "none" | "" => Ok(EffectId::None),
        "walkie_talkie" => Ok(EffectId::WalkieTalkie),
        "game_boy" => Ok(EffectId::GameBoy),
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
