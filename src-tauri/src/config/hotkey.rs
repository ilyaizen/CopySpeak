// Hotkey configuration: single global shortcut to trigger speech from clipboard.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeyConfig {
    pub enabled: bool,
    pub shortcut: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: "Win+Shift+A".to_string(),
        }
    }
}

impl HotkeyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.shortcut.trim().is_empty() {
            return Err("Hotkey shortcut cannot be empty when enabled".to_string());
        }

        let parts: Vec<&str> = self.shortcut.split('+').map(|s| s.trim()).collect();

        if parts.is_empty() {
            return Err("Invalid hotkey format".to_string());
        }

        let has_modifier = parts.iter().any(|p| {
            matches!(
                p.to_lowercase().as_str(),
                "ctrl" | "control" | "alt" | "shift" | "super" | "meta" | "win" | "cmd"
            )
        });

        if !has_modifier {
            return Err(
                "Hotkey must include at least one modifier (Ctrl, Alt, Shift, or Win)".to_string(),
            );
        }

        Ok(())
    }
}
