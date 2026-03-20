// ElevenLabs voice listing and output format commands.

use crate::config::AppConfig;
use std::sync::Mutex;
use tauri::State;

/// List available ElevenLabs voices.
/// Requires valid API key in config.
#[tauri::command]
pub fn list_elevenlabs_voices(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<Vec<crate::tts::elevenlabs::ElevenLabsVoice>, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] list_elevenlabs_voices called");
    }

    let cfg = config.lock().unwrap();
    let backend = crate::tts::elevenlabs::ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());

    match backend.list_voices() {
        Ok(voices) => Ok(voices),
        Err(e) => Err(format!("Failed to fetch voices: {}", e)),
    }
}

/// Get voice details by ID from ElevenLabs API.
/// Useful for validating manually entered voice IDs.
#[tauri::command]
pub fn get_elevenlabs_voice_by_id(
    voice_id: String,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<crate::tts::elevenlabs::ElevenLabsVoice, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_elevenlabs_voice_by_id called for: {}", voice_id);
    }

    let cfg = config.lock().unwrap();
    let backend = crate::tts::elevenlabs::ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());

    match backend.get_voice_by_id(&voice_id) {
        Ok(voice) => Ok(voice),
        Err(e) => Err(format!("Failed to fetch voice: {}", e)),
    }
}

/// Get available ElevenLabs output formats for the frontend.
#[tauri::command]
pub fn get_elevenlabs_output_formats() -> Vec<(String, String)> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] get_elevenlabs_output_formats called");
    }
    use crate::tts::elevenlabs::ElevenLabsOutputFormat;

    ElevenLabsOutputFormat::all()
        .iter()
        .map(|fmt| (format!("{:?}", fmt), fmt.label().to_string()))
        .collect()
}
