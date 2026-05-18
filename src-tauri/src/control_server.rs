use crate::audio::AudioPlayer;
use crate::config::{self, AppConfig, EffectId, TtsEngine};
use crate::history::HistoryLog;
use crate::telemetry;
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

const DEFAULT_ADDR: &str = "127.0.0.1:43117";

#[derive(Debug, Deserialize)]
struct SpeakRequest {
    text: String,
    engine: Option<String>,
    effect: Option<String>,
}

enum ControlRequest {
    Health,
    Speak(SpeakRequest),
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
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                if request_complete(&buffer) {
                    break;
                }
            }
            Err(error) => {
                log::warn!("[Control] Read failed: {}", error);
                return;
            }
        }
    }

    let response = match parse_request(&buffer) {
        Ok(ControlRequest::Health) => http_response(200, "OK", r#"{"ok":true,"app":"CopySpeak"}"#),
        Ok(ControlRequest::Speak(request)) => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = speak(app_clone, request).await {
                    log::error!("[Control] Speak failed: {}", error);
                }
            });
            http_response(202, "Accepted", r#"{"ok":true}"#)
        }
        Err((status, message)) => {
            http_response(status, "Error", &format!(r#"{{"error":{:?}}}"#, message))
        }
    };

    let _ = stream.write_all(response.as_bytes());
}

fn request_complete(buffer: &[u8]) -> bool {
    let header_end = find_header_end(buffer);
    let Some(header_end) = header_end else {
        return false;
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let content_length = content_length(&headers).unwrap_or(0);
    buffer.len() >= header_end + 4 + content_length
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
    if !request_line.starts_with("POST /speak ") {
        return Err((404, "expected GET /health or POST /speak".to_string()));
    }

    let content_length = content_length(&headers).unwrap_or(0);

    if content_length > 200_000 {
        return Err((413, "request too large".to_string()));
    }

    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err((400, "incomplete body".to_string()));
    }

    let request: SpeakRequest = serde_json::from_slice(&buffer[body_start..body_end])
        .map_err(|error| (400, format!("invalid JSON: {}", error)))?;
    if request.text.trim().is_empty() {
        return Err((400, "text is required".to_string()));
    }
    Ok(ControlRequest::Speak(request))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn speak(app: AppHandle, request: SpeakRequest) -> Result<(), String> {
    if request.engine.is_some() || request.effect.is_some() {
        let config_state: State<Mutex<AppConfig>> = app.state();
        let mut cfg = config_state.lock().unwrap();
        if let Some(engine) = request.engine.as_deref() {
            cfg.tts.active_backend = parse_engine(engine)?;
        }
        if matches!(request.effect.as_deref(), Some("walkie_talkie")) {
            cfg.effects.enabled = true;
            cfg.effects.active_effect = EffectId::WalkieTalkie;
        }
        config::save(&cfg)?;
    }

    let config_state: State<Mutex<AppConfig>> = app.state();
    let player: State<Mutex<AudioPlayer>> = app.state();
    let history: State<Mutex<HistoryLog>> = app.state();
    let telemetry_state: State<Mutex<telemetry::TelemetryLog>> = app.state();
    crate::commands::speak_now(
        app.clone(),
        config_state,
        player,
        history,
        telemetry_state,
        Some(request.text),
    )
    .await
}

fn parse_engine(engine: &str) -> Result<TtsEngine, String> {
    match engine.to_ascii_lowercase().as_str() {
        "cartesia" => Ok(TtsEngine::Cartesia),
        "openai" => Ok(TtsEngine::OpenAI),
        "elevenlabs" | "eleven_labs" => Ok(TtsEngine::ElevenLabs),
        "local" => Ok(TtsEngine::Local),
        _ => Err(format!("unsupported engine: {}", engine)),
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
