// HUD overlay configuration.
// Controls the transparent always-on-top window that shows waveform + controls during playback.

use super::ValidationError;
use serde::{Deserialize, Serialize};

/// Where the HUD overlay appears on screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum HudPosition {
    Preset(HudPresetPosition),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HudPresetPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HudConfig {
    pub enabled: bool,
    pub position: HudPosition,
    pub width: u32,
    pub height: u32,
    pub opacity: f32,
}

impl HudConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if self.opacity < 0.0 || self.opacity > 1.0 {
            errors.push(ValidationError::OpacityOutOfRange {
                value: self.opacity,
                min: 0.0,
                max: 1.0,
            });
        }

        if self.width < 50 {
            errors.push(ValidationError::HudWidthTooSmall {
                value: self.width,
                min: 50,
            });
        }

        if self.height < 20 {
            errors.push(ValidationError::HudHeightTooSmall {
                value: self.height,
                min: 20,
            });
        }

        errors
    }
}
