// Credential validation for the active LLM post-processing provider.

use crate::commands::CredentialCheckResult;
use crate::config::AppConfig;
use std::sync::Mutex;
use tauri::State;

/// Validate the active provider's API key via GET {base_url}/models
/// (no completion credits consumed).
#[tauri::command]
pub fn check_post_process_credentials(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<CredentialCheckResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_post_process_credentials called");
    }

    let cfg = config.lock().unwrap();
    let provider = match cfg.post_process.active_provider() {
        Some(p) => p.clone(),
        None => {
            return Ok(CredentialCheckResult {
                success: false,
                message: "No post-process provider configured.".into(),
                error_type: Some("provider_missing".into()),
            });
        }
    };
    let label = provider.label.clone();
    let api_key = crate::secrets::resolve(&provider.api_key, &["POST_PROCESS_API_KEY"]);

    if api_key.trim().is_empty() {
        return Ok(CredentialCheckResult {
            success: false,
            message: format!("API key is empty. Enter your {} API key.", label),
            error_type: Some("api_key_missing".into()),
        });
    }

    let client = reqwest::blocking::Client::new();
    match client
        .get(format!("{}/models", provider.base_url.trim()))
        .bearer_auth(&api_key)
        .send()
    {
        Ok(resp) => match resp.status().as_u16() {
            200 => Ok(CredentialCheckResult {
                success: true,
                message: format!("{} API key is valid.", label),
                error_type: None,
            }),
            401 | 403 => Ok(CredentialCheckResult {
                success: false,
                message: format!("Authentication failed. Check your {} API key.", label),
                error_type: Some("auth_failed".into()),
            }),
            status => Ok(CredentialCheckResult {
                success: false,
                message: format!("{} API returned unexpected status: {}", label, status),
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
