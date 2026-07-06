// Shared helpers used across TTS command sub-modules.

use crate::config::{ProfileEngineOptions, ProfileTextProcessing, TtsConfig, TtsEngine};
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
        TtsEngine::Edge => Box::new(crate::tts::edge::EdgeTtsBackend::new(
            tts_config.edge.clone(),
        )),
    }
}

pub(crate) fn create_backend_from_effective(
    eff: &EffectiveTtsRequest,
    tts_config: &TtsConfig,
) -> Box<dyn TtsBackend> {
    match &eff.engine {
        TtsEngine::Local => {
            let opts = eff.engine_options.local();
            let command = opts
                .and_then(|o| o.command.clone())
                .unwrap_or_else(|| tts_config.command.clone());
            let args_template = opts
                .and_then(|o| o.args_template.clone())
                .filter(|items| !items.is_empty())
                .unwrap_or_else(|| tts_config.args_template.clone());
            Box::new(CliTtsBackend::new(command, args_template))
        }
        TtsEngine::OpenAI => {
            let mut config = tts_config.openai.clone();
            config.voice = eff.voice.clone();
            if let Some(o) = eff.engine_options.openai() {
                if let Some(model) = o.model.clone() {
                    config.model = model;
                }
                if let Some(rf) = o.response_format.clone() {
                    config.response_format = rf;
                }
                config.instructions = o.instructions.clone().or(config.instructions);
            }
            Box::new(crate::tts::openai::OpenAiTtsBackend::new(config))
        }
        TtsEngine::ElevenLabs => {
            let mut config = tts_config.elevenlabs.clone();
            config.voice_id = eff.voice.clone();
            config.voice_name = eff.voice_label.clone().or(config.voice_name);
            if let Some(o) = eff.engine_options.elevenlabs() {
                if let Some(model_id) = o.model_id.clone() {
                    config.model_id = model_id;
                }
                if let Some(output_format) = o.output_format.as_deref() {
                    if let Ok(parsed) = serde_json::from_value(serde_json::json!(output_format)) {
                        config.output_format = parsed;
                    }
                }
                if let Some(stability) = o.stability {
                    config.voice_stability = stability;
                }
                if let Some(similarity) = o.similarity_boost {
                    config.voice_similarity_boost = similarity;
                }
                config.voice_style = o.style.or(config.voice_style);
                config.use_speaker_boost = o.use_speaker_boost.or(config.use_speaker_boost);
            }
            Box::new(crate::tts::elevenlabs::ElevenLabsTtsBackend::new(config))
        }
        TtsEngine::Cartesia => {
            let mut config = tts_config.cartesia.clone();
            config.voice_id = eff.voice.clone();
            config.voice_name = eff.voice_label.clone().or(config.voice_name);
            if let Some(o) = eff.engine_options.cartesia() {
                if let Some(model_id) = o.model_id.clone() {
                    config.model_id = model_id;
                }
                if let Some(output_format) = o.output_format.clone() {
                    config.output_format = output_format;
                }
                config.encoding = o.encoding.clone().or(config.encoding);
                config.sample_rate = o.sample_rate.or(config.sample_rate);
            }
            Box::new(crate::tts::cartesia::CartesiaTtsBackend::new(config))
        }
        TtsEngine::Http => {
            let mut config = tts_config.http.clone();
            config.voice = eff.voice.clone();
            if let Some(o) = eff.engine_options.http() {
                if let Some(url_template) = o.url_template.clone() {
                    config.url_template = url_template;
                }
                if let Some(method) = o.method.clone() {
                    config.method = method;
                }
                config.body_template = o.body_template.clone().or(config.body_template);
                if let Some(rf) = o.response_format.clone() {
                    config.response_format = rf;
                }
                if let Some(timeout) = o.timeout_secs {
                    config.timeout_secs = timeout;
                }
            }
            Box::new(crate::tts::http::HttpTtsBackend::new(config))
        }
        TtsEngine::Google => {
            let mut config = tts_config.google.clone();
            config.voice_name = eff.voice.clone();
            if let Some(o) = eff.engine_options.google() {
                if let Some(model) = o.model.clone() {
                    config.model = model;
                }
                if let Some(output_format) = o.output_format.clone() {
                    config.output_format = output_format;
                }
            }
            Box::new(crate::tts::google::GoogleTtsBackend::new(config))
        }
        TtsEngine::Microsoft => {
            let mut config = tts_config.microsoft.clone();
            config.voice_name = eff.voice.clone();
            if let Some(o) = eff.engine_options.microsoft() {
                if let Some(endpoint) = o.endpoint.clone() {
                    config.endpoint = endpoint;
                }
                if let Some(model) = o.model.clone() {
                    config.model = model;
                }
                if let Some(output_format) = o.output_format.clone() {
                    config.output_format = output_format;
                }
            }
            Box::new(crate::tts::microsoft::MicrosoftTtsBackend::new(config))
        }
        TtsEngine::Edge => {
            let mut config = tts_config.edge.clone();
            config.voice = eff.voice.clone();
            if let Some(o) = eff.engine_options.edge() {
                if let Some(voice) = o.voice.clone() {
                    config.voice = voice;
                }
            }
            Box::new(crate::tts::edge::EdgeTtsBackend::new(config))
        }
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
        TtsEngine::Edge => tts_config.edge.voice.clone(),
    }
}

/// Get the engine string identifier for a backend (used in history filenames).
/// Returns preset name for local engines (piper, kokoro), or engine name for cloud.
pub(crate) fn engine_identifier(active: &TtsEngine, tts_config: &TtsConfig) -> String {
    match active {
        TtsEngine::Local => {
            // Preset lives on the active profile's local options now; the
            // top-level tts_config.preset is a legacy field that's empty for
            // profile-based configs (would yield a leading-dash filename).
            let preset = resolve_effective(tts_config)
                .engine_options
                .local()
                .and_then(|o| o.preset.clone())
                .unwrap_or_else(|| tts_config.preset.trim().to_string());
            match preset.as_str() {
                "kokoro-tts" => "kokoro".to_string(),
                "pocket-tts" => "pocket".to_string(),
                "" => "local".to_string(),
                p => p.to_string(),
            }
        }
        TtsEngine::OpenAI => "openai".to_string(),
        TtsEngine::ElevenLabs => "elevenlabs".to_string(),
        TtsEngine::Cartesia => "cartesia".to_string(),
        TtsEngine::Http => "http".to_string(),
        TtsEngine::Google => "google".to_string(),
        TtsEngine::Microsoft => "microsoft".to_string(),
        TtsEngine::Edge => "edge".to_string(),
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
        TtsEngine::Edge => {
            // "en-US-EmmaMultilingualNeural" → "emma"
            voice_id
                .split('-')
                .nth(2)
                .unwrap_or(voice_id)
                .trim_end_matches("Neural")
                .to_lowercase()
        }
    }
}

// ── Effective profile resolution ─────────────────────────────────────────────

/// The resolved synthesis parameters for a voice profile.
#[allow(dead_code)]
pub(crate) struct EffectiveTtsRequest {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub engine: TtsEngine,
    pub voice: String,
    pub voice_label: Option<String>,
    pub pitch: f32,
    pub effects: crate::config::ProfileEffects,
    pub text_processing: ProfileTextProcessing,
    pub engine_options: ProfileEngineOptions,
}

pub(crate) fn resolve_effective_for_profile(
    tts_config: &TtsConfig,
    profile_id: Option<&str>,
) -> Result<EffectiveTtsRequest, String> {
    let selected_id = profile_id.unwrap_or(&tts_config.active_profile_id);
    let profile = tts_config
        .profiles
        .iter()
        .find(|p| p.id == selected_id)
        .ok_or_else(|| format!("unknown profile: {}", selected_id))?;
    let voice = if profile.voice.trim().is_empty() {
        voice_for_backend(&profile.engine, tts_config)
    } else {
        profile.voice.clone()
    };

    Ok(EffectiveTtsRequest {
        profile_id: Some(profile.id.clone()),
        profile_name: Some(profile.name.clone()),
        engine: profile.engine.clone(),
        voice,
        voice_label: profile.voice_label.clone(),
        pitch: profile.pitch,
        effects: profile.effects.clone(),
        text_processing: profile.text_processing.clone(),
        engine_options: profile.engine_options.clone(),
    })
}

pub(crate) fn active_engine(tts_config: &TtsConfig) -> TtsEngine {
    resolve_effective(tts_config).engine
}

pub(crate) fn resolve_effective(tts_config: &TtsConfig) -> EffectiveTtsRequest {
    resolve_effective_for_profile(tts_config, None).unwrap_or_else(|_| {
        let engine = tts_config.active_backend.clone();
        let engine_options = ProfileEngineOptions::default_for(&engine);
        EffectiveTtsRequest {
            profile_id: None,
            profile_name: None,
            voice: voice_for_backend(&engine, tts_config),
            voice_label: None,
            engine,
            pitch: 1.0,
            effects: crate::config::ProfileEffects::default(),
            text_processing: ProfileTextProcessing::default(),
            engine_options,
        }
    })
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
        TtsEngine::Edge => "edge",
    }
}
