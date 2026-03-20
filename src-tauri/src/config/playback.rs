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
    #[serde(default = "default_playback_speed")]
    pub playback_speed: f32,
    #[serde(default = "default_pitch")]
    pub pitch: f32,
}

fn default_volume() -> u8 {
    100
}

fn default_playback_speed() -> f32 {
    1.4
}

fn default_pitch() -> f32 {
    0.9
}
