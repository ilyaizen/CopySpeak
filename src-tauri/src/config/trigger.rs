// Trigger configuration: double-copy detection window and text length limits.

use serde::{Deserialize, Serialize};

use super::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub double_copy_window_ms: u64,
    #[serde(default = "default_max_text_length")]
    pub max_text_length: u64,
}

fn default_max_text_length() -> u64 {
    100000
}

impl TriggerConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if self.double_copy_window_ms < 100 {
            errors.push(ValidationError::DoubleCopyWindowTooSmall {
                value: self.double_copy_window_ms,
                min: 100,
            });
        }

        if self.max_text_length < 100 {
            errors.push(ValidationError::MaxTextLengthTooSmall {
                value: self.max_text_length,
                min: 100,
            });
        }

        errors
    }
}
