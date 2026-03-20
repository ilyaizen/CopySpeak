// TTS engine health check commands.

use crate::config::AppConfig;
use crate::tts::TtsError;
use std::sync::Mutex;
use tauri::State;

use super::helpers::create_backend;

/// Result of a TTS engine health check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TtsHealthResult {
    pub success: bool,
    pub message: String,
    pub error_type: Option<String>,
}

/// Result of checking if a command exists in PATH.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandExistsResult {
    pub available: bool,
}

/// Check if a command exists in the system PATH.
/// This is used to check if local TTS engines are installed without fully testing them.
#[tauri::command]
pub fn check_command_exists(command: String) -> Result<CommandExistsResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_command_exists called for: {}", command);
    }

    // Try to find the command in PATH using `which` on Unix or `where` on Windows
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("where")
            .arg(&command)
            .output()
    } else {
        std::process::Command::new("which")
            .arg(&command)
            .output()
    };

    match result {
        Ok(output) => {
            let available = output.status.success();
            if crate::logging::is_debug_mode() {
                log::debug!("[IPC] check_command_exists({}): {}", command, available);
            }
            Ok(CommandExistsResult { available })
        }
        Err(e) => {
            log::warn!("[IPC] check_command_exists failed for {}: {}", command, e);
            Ok(CommandExistsResult { available: false })
        }
    }
}

#[tauri::command]
pub fn test_tts_engine(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<TtsHealthResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] test_tts_engine called");
    }

    let (active_backend, tts_config) = {
        let cfg = config.lock().unwrap();
        (cfg.tts.active_backend.clone(), cfg.tts.clone())
    };

    let backend: Box<dyn crate::tts::TtsBackend> = create_backend(&active_backend, &tts_config);

    let backend_name = match active_backend {
        crate::config::TtsEngine::Local => tts_config.command.clone(),
        crate::config::TtsEngine::OpenAI => format!("OpenAI ({})", tts_config.openai.model),
        crate::config::TtsEngine::ElevenLabs => format!("ElevenLabs ({})", tts_config.elevenlabs.model_id),
    };

    match backend.health_check() {
        Ok(()) => {
            log::info!("TTS engine health check passed: {}", backend_name);
            Ok(TtsHealthResult {
                success: true,
                message: format!("{} is available and configured correctly", backend_name),
                error_type: None,
            })
        }
        Err(e) => {
            log::warn!("TTS engine health check failed: {}", e);
            let (message, error_type) = match &e {
                TtsError::Unavailable(msg) => {
                    if msg.contains("API key") {
                        (format!("{} - API key is missing or invalid", backend_name), "api_key_missing")
                    } else if msg.contains("not found") || msg.contains("The system cannot find") {
                        (format!("Command '{}' not found. Please ensure the TTS engine is installed and in PATH.", backend_name), "not_found")
                    } else if msg.contains("Access is denied") || msg.contains("permission") {
                        (format!("Permission denied accessing '{}'. Check permissions.", backend_name), "permission_denied")
                    } else {
                        (format!("{} unavailable: {}", backend_name, msg), "unavailable")
                    }
                }
                TtsError::Http(msg) => {
                    if msg.contains("401") || msg.contains("403") {
                        (format!("{} - Authentication failed. Check your API key.", backend_name), "auth_failed")
                    } else if msg.contains("429") {
                        (format!("{} - Rate limit exceeded. Please try again later.", backend_name), "rate_limit")
                    } else {
                        (format!("{} - Network error: {}", backend_name, msg), "http_error")
                    }
                }
                TtsError::Io(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        (format!("Command '{}' not found. Please ensure the TTS engine is installed.", backend_name), "not_found")
                    } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                        (format!("Permission denied running '{}'. Check file permissions.", backend_name), "permission_denied")
                    } else {
                        (format!("IO error: {}", e), "io_error")
                    }
                }
                _ => (format!("TTS engine check failed: {}", e), "unknown"),
            };
            Ok(TtsHealthResult {
                success: false,
                message,
                error_type: Some(error_type.to_string()),
            })
        }
    }
}
