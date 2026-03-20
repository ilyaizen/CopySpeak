// Shared helpers used across TTS command sub-modules.

use crate::config::{TtsConfig, TtsEngine};
use crate::tts::cli::CliTtsBackend;
use crate::tts::TtsBackend;
use crate::JobStatus;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

// ── RAII guard ──────────────────────────────────────────────────────────────

/// RAII Guard to ensure is_synthesizing is reset robustly.
pub(crate) struct SynthesisGuard {
    app_handle: AppHandle,
}

impl SynthesisGuard {
    pub(crate) fn new(app: &AppHandle) -> Self {
        {
            let status = app.state::<JobStatus>();
            status.is_synthesizing.store(true, Ordering::Relaxed);
        }
        crate::update_tray_icon(app);
        if let Err(e) = app.emit("synthesis-state-change", true) {
            log::warn!("Failed to emit synthesis-state-change: {}", e);
        }
        Self {
            app_handle: app.clone(),
        }
    }
}

impl Drop for SynthesisGuard {
    fn drop(&mut self) {
        {
            let status = self.app_handle.state::<JobStatus>();
            status.is_synthesizing.store(false, Ordering::Relaxed);
        }
        crate::update_tray_icon(&self.app_handle);
        if let Err(e) = self.app_handle.emit("synthesis-state-change", false) {
            log::warn!("Failed to emit synthesis-state-change: {}", e);
        }
    }
}

// ── Backend / voice / engine helpers ────────────────────────────────────────

/// Create a TTS backend instance based on the active backend config.
pub(crate) fn create_backend(active: &TtsEngine, tts_config: &TtsConfig) -> Box<dyn TtsBackend> {
    match active {
        TtsEngine::Local => Box::new(CliTtsBackend::new(
            tts_config.command.clone(),
            tts_config.args_template.clone(),
        )),
        TtsEngine::OpenAI => Box::new(crate::tts::openai::OpenAiTtsBackend::new(
            tts_config.openai.clone(),
        )),
        TtsEngine::ElevenLabs => Box::new(crate::tts::elevenlabs::ElevenLabsTtsBackend::new(
            tts_config.elevenlabs.clone(),
        )),
    }
}

/// Get the voice string for the active backend.
pub(crate) fn voice_for_backend(active: &TtsEngine, tts_config: &TtsConfig) -> String {
    match active {
        TtsEngine::Local => tts_config.voice.clone(),
        TtsEngine::OpenAI => tts_config.openai.voice.clone(),
        TtsEngine::ElevenLabs => tts_config.elevenlabs.voice_id.clone(),
    }
}

/// Get the engine string identifier for a backend (used in history filenames).
/// Returns preset name for local engines (piper, kokoro, pocket), or engine name for cloud.
pub(crate) fn engine_identifier(active: &TtsEngine, tts_config: &TtsConfig) -> String {
    match active {
        TtsEngine::Local => {
            // Normalize preset names for filenames
            match tts_config.preset.as_str() {
                "kokoro-tts" => "kokoro".to_string(),
                "pocket-tts" => "pocket".to_string(),
                p => p.to_string(),
            }
        }
        TtsEngine::OpenAI => "openai".to_string(),
        TtsEngine::ElevenLabs => "elevenlabs".to_string(),
    }
}

/// Get the display name for a voice (used in history filenames and HUD).
/// For ElevenLabs, uses cached voice_name from config if available.
/// Cleans up voice names to remove descriptions (e.g., "Matilda - professional" -> "matilda").
pub(crate) fn voice_display_name(
    active: &TtsEngine,
    tts_config: &TtsConfig,
    voice_id: &str,
) -> String {
    match active {
        TtsEngine::ElevenLabs => {
            // Use cached voice_name from config if available
            tts_config
                .elevenlabs
                .voice_name
                .as_ref()
                .map(|n| {
                    // Clean up: extract just the name before " -" or take first word
                    n.split(" -")
                        .next()
                        .unwrap_or(n)
                        .split_whitespace()
                        .next()
                        .unwrap_or(n)
                        .to_lowercase()
                })
                .unwrap_or_else(|| {
                    crate::tts::elevenlabs::ElevenLabsTtsBackend::resolve_voice_name_static(
                        voice_id,
                    )
                })
        }
        TtsEngine::OpenAI => voice_id.to_lowercase(),
        TtsEngine::Local => {
            // For local engines, extract voice name from preset format (e.g., "en_US-joe-medium" -> "joe")
            voice_id
                .split('-')
                .nth(1)
                .unwrap_or(voice_id)
                .to_lowercase()
        }
    }
}

/// Get the engine string identifier for a backend (legacy function, prefer engine_identifier).
pub(crate) fn engine_str(active: &TtsEngine) -> &'static str {
    match active {
        TtsEngine::Local => "local",
        TtsEngine::OpenAI => "openai",
        TtsEngine::ElevenLabs => "elevenlabs",
    }
}
