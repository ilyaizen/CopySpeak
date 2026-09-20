// Onboarding-scoped config commands (issue #43).
//
// On a fresh install (no config.json), the onboarding page's `get_config`
// invoke() never resolves on Windows v0.2.5 (issue #43: ~4.4KB payload is
// produced fine on the Rust side but the promise hangs). The proven
// workaround from the issue thread: small dedicated commands go through.
//
// These commands deliberately avoid the full AppConfig payload. They work
// against tiny scalars so onboarding cannot depend on the (suspect) full
// config IPC round-trip. `get_config` stays for Settings/power users, along
// with the .env hydration overlay in commands/config.rs.

use crate::config::{self, AppConfig, TtsEngine};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

fn engine_display_name(engine: &TtsEngine) -> String {
    match engine {
        TtsEngine::Local => "Local".into(),
        TtsEngine::Kitten => "Kitten TTS".into(),
        TtsEngine::Qwen => "Qwen3-TTS".into(),
        TtsEngine::Piper => "Piper".into(),
        TtsEngine::Kokoro => "Kokoro".into(),
        TtsEngine::Pocket => "Pocket".into(),
        TtsEngine::OpenAI => "OpenAI".into(),
        TtsEngine::ElevenLabs => "ElevenLabs".into(),
        TtsEngine::Cartesia => "Cartesia".into(),
        TtsEngine::Http => "HTTP".into(),
        TtsEngine::Google => "Google".into(),
        TtsEngine::Microsoft => "Microsoft".into(),
        TtsEngine::Edge => "Edge-TTS".into(),
    }
}

fn engine_is_keyless(engine: &TtsEngine) -> bool {
    matches!(
        engine,
        TtsEngine::Edge
            | TtsEngine::Kitten
            | TtsEngine::Qwen
            | TtsEngine::Piper
            | TtsEngine::Kokoro
            | TtsEngine::Pocket
    )
}

/// What onboarding needs to know, and nothing more.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OnboardingStatus {
    /// True when a config file already exists on disk (returning user).
    pub has_config: bool,
    /// The active TTS engine id ("edge", "cartesia", ...).
    pub engine: String,
    /// Display name for the active engine.
    pub engine_name: String,
    /// Voice shown to the user for the active engine.
    pub voice: String,
    /// True when the active engine needs no API key (Edge, local engines).
    pub keyless: bool,
}

#[tauri::command]
pub fn get_onboarding_status(config: State<'_, Mutex<AppConfig>>) -> OnboardingStatus {
    let cfg = config.lock().unwrap().clone();
    let profile = cfg
        .tts
        .profiles
        .iter()
        .find(|p| p.id == cfg.tts.active_profile_id);
    let engine = profile
        .map(|p| p.engine.clone())
        .unwrap_or_else(|| cfg.tts.active_backend.clone());
    let engine_str = serde_json::to_value(&engine)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{engine:?}").to_lowercase());
    let voice = profile.map(|p| p.voice.clone()).unwrap_or_default();
    OnboardingStatus {
        has_config: config::config_path().exists(),
        engine: engine_str,
        engine_name: engine_display_name(&engine),
        voice,
        keyless: engine_is_keyless(&engine),
    }
}

/// Complete onboarding: pin the active TTS engine and mark onboarding done.
///
/// Uses a single tiny scalar (`engine`) instead of shipping a full
/// AppConfig back and forth, so a fresh install never touches the suspect
/// `get_config` IPC path. The in-memory config already holds the defaults;
/// we only switch the active engine + profile, then persist.
#[tauri::command]
pub fn complete_onboarding(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    engine: String,
) -> Result<OnboardingStatus, String> {
    use crate::config::TtsEngine;

    let parsed: TtsEngine = serde_json::from_value(serde_json::Value::String(engine.clone()))
        .map_err(|_| format!("Unknown engine: {engine}"))?;

    let status = {
        let mut cfg = config.lock().unwrap();
        // Find this engine's first profile; refuse engines we have no
        // profile for (their setup lives in Settings, not onboarding).
        let profile = cfg
            .tts
            .profiles
            .iter()
            .find(|p| p.engine == parsed)
            .map(|p| (p.id.clone(), p.engine.clone()))
            .ok_or_else(|| format!("No profile configured for engine {engine}"))?;
        cfg.tts.active_profile_id = profile.0;
        cfg.tts.active_backend = profile.1;

        let voice = cfg
            .tts
            .profiles
            .iter()
            .find(|p| p.id == cfg.tts.active_profile_id)
            .map(|p| p.voice.clone())
            .unwrap_or_default();
        let engine_name = engine_display_name(&cfg.tts.active_backend);
        let keyless = engine_is_keyless(&cfg.tts.active_backend);
        OnboardingStatus {
            has_config: true,
            engine: engine.clone(),
            engine_name,
            voice,
            keyless,
        }
    };

    // Persist. set_config's side effects (hotkey, audio, autostart) are not
    // needed here: onboarding only changes which TTS engine is active.
    {
        let cfg = config.lock().unwrap();
        config::save(&cfg)?;
    }

    let _ = app.emit("config-changed", ());
    Ok(status)
}
