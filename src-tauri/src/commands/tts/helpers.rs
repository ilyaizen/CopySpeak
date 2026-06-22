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
        TtsEngine::Cartesia => Box::new(crate::tts::cartesia::CartesiaTtsBackend::new(
            tts_config.cartesia.clone(),
        )),
        TtsEngine::Http => Box::new(crate::tts::http::HttpTtsBackend::new(
            tts_config.http.clone(),
        )),
        TtsEngine::Google => Box::new(crate::tts::google::GoogleTtsBackend::new(
            tts_config.google.clone(),
        )),
        TtsEngine::Microsoft => Box::new(crate::tts::microsoft::MicrosoftTtsBackend::new(
            tts_config.microsoft.clone(),
        )),
    }
}

/// Get the voice string for the active backend.
pub(crate) fn voice_for_backend(active: &TtsEngine, tts_config: &TtsConfig) -> String {
    match active {
        TtsEngine::Local => tts_config.voice.clone(),
        TtsEngine::OpenAI => tts_config.openai.voice.clone(),
        TtsEngine::ElevenLabs => tts_config.elevenlabs.voice_id.clone(),
        TtsEngine::Cartesia => tts_config.cartesia.voice_id.clone(),
        TtsEngine::Http => tts_config.http.voice.clone(),
        TtsEngine::Google => tts_config.google.voice_name.clone(),
        TtsEngine::Microsoft => tts_config.microsoft.voice_name.clone(),
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
        TtsEngine::Cartesia => "cartesia".to_string(),
        TtsEngine::Http => "http".to_string(),
        TtsEngine::Google => "google".to_string(),
        TtsEngine::Microsoft => "microsoft".to_string(),
    }
}

fn slugify_filename_part(value: &str) -> String {
    value
        .split(" -")
        .next()
        .unwrap_or(value)
        .split_whitespace()
        .next()
        .unwrap_or(value)
        .to_lowercase()
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
                    slugify_filename_part(n)
                })
                .unwrap_or_else(|| {
                    crate::tts::elevenlabs::ElevenLabsTtsBackend::resolve_voice_name_static(
                        voice_id,
                    )
                })
        }
        TtsEngine::OpenAI => voice_id.to_lowercase(),
        TtsEngine::Cartesia => tts_config
            .cartesia
            .voice_name
            .as_deref()
            .map(slugify_filename_part)
            .unwrap_or_else(|| match voice_id {
                "f786b574-daa5-4673-aa0c-cbe3e8534c02" => "katie".to_string(),
                "a5136bf9-224c-4d76-b823-52bd5efcffcc" => "jameson".to_string(),
                _ => "voice".to_string(),
            }),
        TtsEngine::Local => {
            // For local engines, extract voice name from preset format (e.g., "en_US-joe-medium" -> "joe")
            voice_id
                .split('-')
                .nth(1)
                .unwrap_or(voice_id)
                .to_lowercase()
        }
        TtsEngine::Http | TtsEngine::Google | TtsEngine::Microsoft => {
            slugify_filename_part(voice_id)
        }
    }
}

// ── Effective profile resolution ─────────────────────────────────────────────

/// The resolved synthesis parameters for the active voice profile.
/// Built once per request so profile switching never mutates global config.
pub(crate) struct EffectiveTtsRequest {
    pub engine: TtsEngine,
    pub voice: String,
    #[allow(dead_code)]
    pub speed: f32,
    #[allow(dead_code)]
    pub pitch: f32,
    #[allow(dead_code)]
    pub effects: crate::config::ProfileEffects,
}

/// Resolve the active voice profile into an effective request.
///
/// The migrated `"default"` profile is a passthrough for the legacy single-engine
/// fields (active_backend + per-provider voice), so existing single-profile users
/// keep identical behavior and the existing engine tabs stay authoritative.
/// Any *named* profile is fully authoritative for engine/voice/speed/effects.
pub(crate) fn resolve_effective(tts_config: &TtsConfig) -> EffectiveTtsRequest {
    let legacy = || {
        let engine = tts_config.active_backend.clone();
        let voice = voice_for_backend(&engine, tts_config);
        EffectiveTtsRequest {
            engine,
            voice,
            speed: 1.0,
            pitch: 1.0,
            effects: crate::config::ProfileEffects::default(),
        }
    };

    let profile = tts_config
        .profiles
        .iter()
        .find(|p| p.id == tts_config.active_profile_id);

    match profile {
        Some(p) if p.id != "default" => {
            let voice = if p.voice.trim().is_empty() {
                voice_for_backend(&p.engine, tts_config)
            } else {
                p.voice.clone()
            };
            EffectiveTtsRequest {
                engine: p.engine.clone(),
                voice,
                speed: p.speed,
                pitch: p.pitch,
                effects: p.effects.clone(),
            }
        }
        _ => legacy(),
    }
}

/// Get the engine string identifier for a backend (legacy function, prefer engine_identifier).
pub(crate) fn engine_str(active: &TtsEngine) -> &'static str {
    match active {
        TtsEngine::Local => "local",
        TtsEngine::OpenAI => "openai",
        TtsEngine::ElevenLabs => "elevenlabs",
        TtsEngine::Cartesia => "cartesia",
        TtsEngine::Http => "http",
        TtsEngine::Google => "google",
        TtsEngine::Microsoft => "microsoft",
    }
}
