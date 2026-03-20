// Credential validation commands (lightweight, no synthesis credits consumed).

use crate::config::AppConfig;
use std::sync::Mutex;
use tauri::State;

/// Result type for lightweight credential validation (no synthesis).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CredentialCheckResult {
    pub success: bool,
    pub message: String,
    pub error_type: Option<String>,
}

/// Validate an ElevenLabs API key via GET /v1/user (no synthesis credits consumed).
#[tauri::command]
pub fn check_elevenlabs_credentials(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<CredentialCheckResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_elevenlabs_credentials called");
    }

    let api_key = config.lock().unwrap().tts.elevenlabs.api_key.clone();

    if api_key.trim().is_empty() {
        return Ok(CredentialCheckResult {
            success: false,
            message: "API key is empty. Enter your ElevenLabs API key.".into(),
            error_type: Some("api_key_missing".into()),
        });
    }

    let client = reqwest::blocking::Client::new();
    match client
        .get("https://api.elevenlabs.io/v1/user")
        .header("xi-api-key", &api_key)
        .send()
    {
        Ok(resp) => match resp.status().as_u16() {
            200 => Ok(CredentialCheckResult {
                success: true,
                message: "ElevenLabs API key is valid.".into(),
                error_type: None,
            }),
            401 | 403 => Ok(CredentialCheckResult {
                success: false,
                message: "Authentication failed. Check your ElevenLabs API key.".into(),
                error_type: Some("auth_failed".into()),
            }),
            status => Ok(CredentialCheckResult {
                success: false,
                message: format!("ElevenLabs API returned unexpected status: {}", status),
                error_type: Some("http_error".into()),
            }),
        },
        Err(e) => Ok(CredentialCheckResult {
            success: false,
            message: format!("Network error: {}", e),
            error_type: Some("http_error".into()),
        }),
    }
}

/// Validate an OpenAI API key via GET /v1/models (no synthesis credits consumed).
#[tauri::command]
pub fn check_openai_credentials(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<CredentialCheckResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_openai_credentials called");
    }

    let api_key = config.lock().unwrap().tts.openai.api_key.clone();

    if api_key.trim().is_empty() {
        return Ok(CredentialCheckResult {
            success: false,
            message: "API key is empty. Enter your OpenAI API key.".into(),
            error_type: Some("api_key_missing".into()),
        });
    }

    let client = reqwest::blocking::Client::new();
    match client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(&api_key)
        .send()
    {
        Ok(resp) => match resp.status().as_u16() {
            200 => Ok(CredentialCheckResult {
                success: true,
                message: "OpenAI API key is valid.".into(),
                error_type: None,
            }),
            401 | 403 => Ok(CredentialCheckResult {
                success: false,
                message: "Authentication failed. Check your OpenAI API key.".into(),
                error_type: Some("auth_failed".into()),
            }),
            status => Ok(CredentialCheckResult {
                success: false,
                message: format!("OpenAI API returned unexpected status: {}", status),
                error_type: Some("http_error".into()),
            }),
        },
        Err(e) => Ok(CredentialCheckResult {
            success: false,
            message: format!("Network error: {}", e),
            error_type: Some("http_error".into()),
        }),
    }
}
