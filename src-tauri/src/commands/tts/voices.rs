// ElevenLabs voice listing and output format commands.

use crate::config::AppConfig;
use crate::config::TtsEngine;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn list_tts_engines() -> Vec<crate::tts::catalog::EngineCatalogEntry> {
    crate::tts::catalog::list_engines()
}

#[tauri::command]
pub fn list_tts_voices(
    engine: TtsEngine,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<Vec<crate::tts::catalog::VoiceCatalogEntry>, String> {
    if engine == TtsEngine::ElevenLabs {
        let cfg = config.lock().unwrap();
        let backend = crate::tts::elevenlabs::ElevenLabsTtsBackend::new(cfg.tts.elevenlabs.clone());
        return backend
            .list_voices()
            .map(|voices| {
                voices
                    .into_iter()
                    .map(|voice| crate::tts::catalog::VoiceCatalogEntry {
                        id: voice.voice_id.clone(),
                        label: voice.name.unwrap_or(voice.voice_id),
                        language: voice.labels.as_ref().and_then(|labels| {
                            labels
                                .get("language")
                                .and_then(|language| language.as_str().map(str::to_string))
                        }),
                        description: voice.description,
                        gender: voice.labels.as_ref().and_then(|labels| {
                            labels
                                .get("gender")
                                .and_then(|gender| gender.as_str().map(str::to_string))
                        }),
                        preview_url: voice.preview_url,
                    })
                    .collect()
            })
            .map_err(|e| format!("Failed to fetch voices: {}", e));
    }

    if engine == TtsEngine::Cartesia {
        let cfg = config.lock().unwrap();
        let backend =
            crate::tts::cartesia::CartesiaTtsBackend::new(cfg.tts.cartesia.clone());
        return match backend.list_voices() {
            Ok(voices) => Ok(voices
                .into_iter()
                .map(|v| crate::tts::catalog::VoiceCatalogEntry {
                    id: v.id,
                    label: v.name.unwrap_or_else(|| "Unnamed voice".into()),
                    language: None,
                    description: v.description,
                    gender: None,
                    preview_url: None,
                })
                .collect()),
            Err(e) => {
                log::warn!("Cartesia voice refresh failed, using static list: {}", e);
                Ok(crate::tts::catalog::list_static_voices(&engine))
            }
        };
    }

    Ok(crate::tts::catalog::list_static_voices(&engine))
}

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
