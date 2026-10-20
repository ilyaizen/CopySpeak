// Playback configuration: retrigger mode and volume.

use serde::{Deserialize, Serialize};

/// What happens when TTS is triggered while already speaking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RetriggerMode {
    Interrupt,
    Queue,
}

/// How other apps are quieted while CopySpeak speaks.
///
/// The old `mode: lower|mute` dropdown is gone (mute was volume 0 — the
/// level slider covers it) and the fade is fixed; configs saved with those
/// fields still deserialize, serde drops the unknown keys.
pub const DUCK_FADE_MS: u32 = 150;

/// Per-app ducking of other apps' audio during playback (Windows only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuckConfig {
    pub enabled: bool,
    /// Volume other apps are lowered to, as a percent of their own level.
    /// 0 replaces the removed "mute" mode.
    pub level_percent: u8,
}

impl Default for DuckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level_percent: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub on_retrigger: RetriggerMode,
    #[serde(default = "default_volume")]
    pub volume: u8,
    #[serde(default = "default_streaming_enabled")]
    pub streaming_enabled: bool,
    #[serde(default)]
    pub duck: DuckConfig,
    // Legacy fields — kept for deserialization during v2→v3 migration,
    // then skipped on serialize.
    #[serde(default = "default_playback_speed", skip_serializing)]
    pub playback_speed: f32,
    #[serde(default = "default_pitch", skip_serializing)]
    pub pitch: f32,
}

fn default_volume() -> u8 {
    100
}

fn default_streaming_enabled() -> bool {
    true
}

fn default_playback_speed() -> f32 {
    1.0
}

fn default_pitch() -> f32 {
    1.0
}
