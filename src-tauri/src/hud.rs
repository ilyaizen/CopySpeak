// HUD overlay window management.
// Controls the transparent always-on-top window that shows waveform + controls during playback.
// The HUD window is created by Tauri at startup (hidden) and shown/hidden as needed.
//
// Event strategy: We use **global** `app.emit()` rather than `emit_to("hud", ...)`
// because in Tauri v2 window-targeted events require window-scoped listeners
// (`getCurrentWindow().listen()`), which adds fragility. Global events are
// received by the standard `listen()` from `@tauri-apps/api/event`, and only
// the HUD component registers handlers for `hud:*` events.

use crate::audio::AmplitudeEnvelope;
use crate::config::{AppConfig, HudConfig, HudPosition, HudPresetPosition, TtsEngine};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewWindow};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct HudStartPayload {
    pub envelope: AmplitudeEnvelope,
    pub text: Option<String>,
    pub provider: Option<String>,
    pub voice: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct HudSynthesizingPayload {
    pub text: Option<String>,
    pub provider: Option<String>,
    pub voice: Option<String>,
    pub duration_ms: Option<u64>,
}

fn get_provider_voice(cfg: &AppConfig) -> (Option<String>, Option<String>) {
    let provider = match cfg.tts.active_backend {
        TtsEngine::Local => {
            // Determine specific local engine name
            match cfg.tts.preset.as_str() {
                "piper" => "Piper",
                "kokoro-tts" => "Kokoro",
                "pocket-tts" => "Pocket",
                _ => "Local",
            }
        }
        TtsEngine::OpenAI => "OpenAI",
        TtsEngine::ElevenLabs => "ElevenLabs",
    }
    .to_string();
    let voice = match cfg.tts.active_backend {
        TtsEngine::Local => {
            let voice_id = cfg.tts.voice.clone();
            match cfg.tts.preset.as_str() {
                "kokoro-tts" => {
                    // Kokoro voices use underscore format (e.g., "af_heart" -> "Heart")
                    voice_id
                        .split('_')
                        .nth(1)
                        .map(|s| {
                            let mut chars = s.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                }
                            }
                        })
                        .unwrap_or_else(|| voice_id.clone())
                }
                _ => voice_id,
            }
        }
        TtsEngine::OpenAI => {
            let voice_id = cfg.tts.openai.voice.clone();
            // Capitalize first letter (e.g., "alloy" -> "Alloy")
            let mut chars = voice_id.chars();
            match chars.next() {
                None => voice_id,
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
        TtsEngine::ElevenLabs => {
            // Use cached voice_name if available, otherwise fall back to static resolver
            let voice_name = cfg.tts.elevenlabs.voice_name.clone().unwrap_or_else(|| {
                crate::tts::elevenlabs::ElevenLabsTtsBackend::resolve_voice_name_static(
                    &cfg.tts.elevenlabs.voice_id,
                )
            });
            // Capitalize first letter (e.g., "rachel" -> "Rachel")
            let mut chars = voice_name.chars();
            match chars.next() {
                None => voice_name,
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    };
    (Some(provider), Some(voice))
}

#[derive(Clone, Serialize)]
pub struct MonitorInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub position_x: i32,
    pub position_y: i32,
    pub is_primary: bool,
}

#[derive(Clone, Serialize)]
pub struct HudPlaybackStartPayload {
    pub text: Option<String>,
    pub provider: Option<String>,
    pub voice: Option<String>,
    pub audio_duration_ms: Option<u64>,
}

pub fn get_available_monitors(window: &WebviewWindow) -> Vec<MonitorInfo> {
    let primary_monitor = window.primary_monitor().ok().flatten();

    let primary_name = primary_monitor.as_ref().and_then(|m| m.name().cloned());

    match window.available_monitors() {
        Ok(monitors) => monitors
            .into_iter()
            .map(|m| monitor_to_info(&m, &primary_name))
            .collect(),
        Err(e) => {
            log::error!("Failed to get available monitors: {}", e);
            Vec::new()
        }
    }
}

fn monitor_to_info(monitor: &Monitor, primary_name: &Option<String>) -> MonitorInfo {
    let size = monitor.size();
    let position = monitor.position();
    let name = monitor.name().cloned();
    let is_primary = match (&name, primary_name) {
        (Some(n), Some(pn)) => n == pn,
        _ => position.x == 0 && position.y == 0,
    };
    MonitorInfo {
        id: None,
        name,
        width: size.width,
        height: size.height,
        position_x: position.x,
        position_y: position.y,
        is_primary,
    }
}

/// Position the HUD window according to config.
pub fn position_hud_window(hud_window: &WebviewWindow, config: &HudConfig) {
    let available_monitors = get_available_monitors(hud_window);

    let target_monitor = available_monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| available_monitors.first());

    let (monitor_size, monitor_offset) = match target_monitor {
        Some(m) => {
            log::debug!(
                "Target monitor: {}x{} at ({}, {})",
                m.width,
                m.height,
                m.position_x,
                m.position_y
            );
            ((m.width, m.height), (m.position_x, m.position_y))
        }
        None => {
            log::warn!("No monitor found, using default 1920x1080");
            ((1920, 1080), (0, 0))
        }
    };

    if let Some(position) = compute_hud_position(
        &config.position,
        config.width,
        config.height,
        monitor_size,
        monitor_offset,
    ) {
        log::debug!("Setting HUD position to ({}, {})", position.x, position.y);
        if let Err(e) = hud_window.set_position(position) {
            log::error!("Failed to set HUD position: {}", e);
        }
    } else {
        log::warn!("Failed to compute HUD position, using default");
    }
}

pub fn show_hud(app: &AppHandle, envelope: AmplitudeEnvelope, text: Option<String>) {
    let (config, provider, voice) = {
        let state = app.state::<std::sync::Mutex<AppConfig>>();
        let cfg = state.lock().unwrap();
        let (p, v) = get_provider_voice(&cfg);
        (cfg.hud.clone(), p, v)
    };

    if !config.enabled {
        log::debug!("HUD disabled in config, not showing");
        return;
    }

    log::info!("Showing HUD for playback");

    // Show window before emitting event
    if let Some(window) = app.get_webview_window("hud") {
        if let Err(e) = window.show() {
            log::error!("Failed to show HUD window: {}", e);
        }
    } else {
        log::warn!("HUD window not found");
    }

    // Spawn a thread so we don't block the caller (possibly on Tokio runtime).
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));

        log::info!("Emitting hud:start event to hud window");
        log::debug!(
            "[HUD] Event payload: envelope values={}, text={:?}",
            envelope.values.len(),
            text
        );

        if let Err(e) = app_clone.emit(
            "hud:start",
            HudStartPayload {
                envelope,
                text,
                provider,
                voice,
            },
        ) {
            log::error!("Failed to emit hud:start event: {}", e);
        } else {
            log::info!("hud:start emitted");
        }
    });
}

pub fn show_hud_synthesizing(app: &AppHandle, text: Option<String>) {
    let (config, provider, voice) = {
        let state = app.state::<std::sync::Mutex<AppConfig>>();
        let cfg = state.lock().unwrap();
        let (p, v) = get_provider_voice(&cfg);
        (cfg.hud.clone(), p, v)
    };

    if !config.enabled {
        log::debug!("HUD disabled in config, not showing synthesizing state");
        return;
    }

    log::info!("Showing HUD for synthesizing");

    // Show window before emitting event
    if let Some(window) = app.get_webview_window("hud") {
        if let Err(e) = window.show() {
            log::error!("Failed to show HUD window: {}", e);
        }
    } else {
        log::warn!("HUD window not found");
    }

    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));

        log::info!("Emitting hud:synthesizing event (global)");
        if let Err(e) = app_clone.emit(
            "hud:synthesizing",
            HudSynthesizingPayload {
                text,
                provider,
                voice,
                duration_ms: None,
            },
        ) {
            log::error!("Failed to emit hud:synthesizing event: {}", e);
        } else {
            log::info!("hud:synthesizing emitted");
        }
    });
}

/// Show HUD for playback of existing audio file.
/// Emits hud:playback_start event - frontend will stream amplitude data.
pub fn show_hud_playback(app: &AppHandle, text: Option<String>, audio_duration_ms: Option<u64>) {
    let (config, provider, voice) = {
        let state = app.state::<std::sync::Mutex<AppConfig>>();
        let cfg = state.lock().unwrap();
        let (p, v) = get_provider_voice(&cfg);
        (cfg.hud.clone(), p, v)
    };

    if !config.enabled {
        log::debug!("HUD disabled in config, not showing for playback");
        return;
    }

    log::info!("Showing HUD for playback");

    // Show window before emitting event
    if let Some(window) = app.get_webview_window("hud") {
        if let Err(e) = window.show() {
            log::error!("Failed to show HUD window: {}", e);
        }
    } else {
        log::warn!("HUD window not found");
    }

    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));

        log::info!("Emitting hud:playback_start event (global)");
        if let Err(e) = app_clone.emit(
            "hud:playback_start",
            HudPlaybackStartPayload {
                text,
                provider,
                voice,
                audio_duration_ms,
            },
        ) {
            log::error!("Failed to emit hud:playback_start event: {}", e);
        } else {
            log::info!("hud:playback_start emitted");
        }
    });
}

#[derive(Clone, Serialize)]
pub struct ClipboardCopiedPayload {
    pub trigger_window_ms: u64,
}

#[derive(Clone, Serialize)]
pub struct SynthesisProgressPayload {
    pub estimated_total_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub fragment_index: usize,
    pub fragment_total: usize,
    pub is_paginated: bool,
    pub confidence: f32,
    pub text_preview: String,
    pub total_chars: usize,
    pub processed_chars: usize,
}

pub fn emit_synthesis_progress(
    app: &AppHandle,
    estimated_total_ms: Option<u64>,
    elapsed_ms: u64,
    fragment_index: usize,
    fragment_total: usize,
    is_paginated: bool,
    confidence: f32,
    text_preview: String,
    total_chars: usize,
    processed_chars: usize,
) {
    let config = {
        let state = app.state::<std::sync::Mutex<AppConfig>>();
        let cfg = state.lock().unwrap();
        cfg.hud.clone()
    };

    if !config.enabled {
        return;
    }

    let payload = SynthesisProgressPayload {
        estimated_total_ms,
        elapsed_ms,
        fragment_index,
        fragment_total,
        is_paginated,
        confidence,
        text_preview,
        total_chars,
        processed_chars,
    };

    if let Err(e) = app.emit("hud:synthesis-progress", payload) {
        log::error!("Failed to emit hud:synthesis-progress event: {}", e);
    }
}

pub fn show_hud_clipboard_copied(app: &AppHandle, trigger_window_ms: u64) {
    let config = {
        let state = app.state::<std::sync::Mutex<AppConfig>>();
        let cfg = state.lock().unwrap();
        cfg.hud.clone()
    };

    if !config.enabled {
        return;
    }

    // Show window before emitting event
    if let Some(window) = app.get_webview_window("hud") {
        if let Err(e) = window.show() {
            log::error!("Failed to show HUD window: {}", e);
        }
    } else {
        log::warn!("HUD window not found");
    }

    if let Err(e) = app.emit(
        "hud:clipboard-copied",
        ClipboardCopiedPayload { trigger_window_ms },
    ) {
        log::error!("Failed to emit hud:clipboard-copied event: {}", e);
    }
}

pub fn hide_hud(app: &AppHandle) {
    log::debug!("Hiding HUD");
    let _ = app.emit("hud:stop", ());

    // Hide the window
    if let Some(window) = app.get_webview_window("hud") {
        if let Err(e) = window.hide() {
            log::error!("Failed to hide HUD window: {}", e);
        }
    }
}

fn compute_hud_position(
    position: &HudPosition,
    hud_width: u32,
    hud_height: u32,
    monitor_size: (u32, u32),
    monitor_offset: (i32, i32),
) -> Option<PhysicalPosition<i32>> {
    let (screen_w, screen_h) = monitor_size;
    let (offset_x, offset_y) = monitor_offset;
    let margin: i32 = 16;

    let (x, y) = match position {
        HudPosition::Preset(preset) => {
            let pos = match preset {
                HudPresetPosition::TopLeft => (offset_x + margin, offset_y + margin),
                HudPresetPosition::TopCenter => (
                    offset_x + (screen_w as i32 - hud_width as i32) / 2,
                    offset_y + margin,
                ),
                HudPresetPosition::TopRight => (
                    offset_x + screen_w as i32 - hud_width as i32 - margin,
                    offset_y + margin,
                ),
                HudPresetPosition::BottomLeft => (
                    offset_x + margin,
                    offset_y + screen_h as i32 - hud_height as i32 - margin - 48,
                ),
                HudPresetPosition::BottomCenter => (
                    offset_x + (screen_w as i32 - hud_width as i32) / 2,
                    offset_y + screen_h as i32 - hud_height as i32 - margin - 48,
                ),
                HudPresetPosition::BottomRight => (
                    offset_x + screen_w as i32 - hud_width as i32 - margin,
                    offset_y + screen_h as i32 - hud_height as i32 - margin - 48,
                ),
            };
            log::debug!(
                "Using preset HUD position {:?}: ({}, {})",
                preset,
                pos.0,
                pos.1
            );
            pos
        }
    };

    Some(PhysicalPosition::new(x, y))
}
