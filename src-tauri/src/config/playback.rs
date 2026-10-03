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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DuckMode {
    #[default]
    Lower,
    Mute,
}

/// Per-app ducking of other apps' audio during playback (Windows only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuckConfig {
    pub enabled: bool,
    pub mode: DuckMode,
    /// Volume other apps are lowered to, as a percent of their own level.
    pub level_percent: u8,
    /// Fade in/out duration; 0 = instant.
    pub fade_ms: u32,
}

impl Default for DuckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: DuckMode::Lower,
            level_percent: 20,
            fade_ms: 150,
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
