// Playback configuration: retrigger mode and volume.

use serde::{Deserialize, Serialize};

/// What happens when TTS is triggered while already speaking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RetriggerMode {
    Interrupt,
    Queue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub on_retrigger: RetriggerMode,
    #[serde(default = "default_volume")]
    pub volume: u8,
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

fn default_playback_speed() -> f32 {
    1.0
}

fn default_pitch() -> f32 {
    1.0
}
