// Credential validation for the LLM post-processing provider (Groq Cloud).

use crate::commands::CredentialCheckResult;
use crate::config::{AppConfig, GROQ_BASE_URL};
use std::sync::Mutex;
use tauri::State;

/// Validate a Groq API key via GET /models (no completion credits consumed).
#[tauri::command]
pub fn check_groq_credentials(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<CredentialCheckResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_groq_credentials called");
    }

    let api_key = crate::secrets::resolve(
        &config.lock().unwrap().post_process.api_key,
        &["POST_PROCESS_API_KEY"],
    );

    if api_key.trim().is_empty() {
        return Ok(CredentialCheckResult {
            success: false,
            message: "API key is empty. Enter your Groq API key.".into(),
            error_type: Some("api_key_missing".into()),
        });
    }

    let client = reqwest::blocking::Client::new();
    match client
        .get(format!("{}/models", GROQ_BASE_URL))
        .bearer_auth(&api_key)
        .send()
    {
        Ok(resp) => match resp.status().as_u16() {
            200 => Ok(CredentialCheckResult {
                success: true,
                message: "Groq API key is valid.".into(),
                error_type: None,
            }),
            401 | 403 => Ok(CredentialCheckResult {
                success: false,
                message: "Authentication failed. Check your Groq API key.".into(),
                error_type: Some("auth_failed".into()),
            }),
            status => Ok(CredentialCheckResult {
                success: false,
                message: format!("Groq API returned unexpected status: {}", status),
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
