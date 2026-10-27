// General app configuration: close behavior, appearance mode, startup settings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CloseBehavior {
    MinimizeToTray,
    Exit,
}

impl Default for CloseBehavior {
    fn default() -> Self {
        CloseBehavior::MinimizeToTray
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

impl Default for AppearanceMode {
    fn default() -> Self {
        AppearanceMode::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    // `launch_on_login` was `start_with_windows` before 0.2.10; alias keeps old configs valid.
    #[serde(alias = "start_with_windows")]
    pub launch_on_login: bool,
    pub start_minimized: bool,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
    #[serde(default)]
    pub debug_mode: bool,
    #[serde(default)]
    pub close_behavior: CloseBehavior,
    #[serde(default)]
    pub appearance: AppearanceMode,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default)]
    pub auto_save_profiles: bool,
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Master switch for Experimental features (Dramatize post-processing,
    /// the Post-processing panel, Effects). Off hides them in the UI AND
    /// forces them off at synthesis time; saved settings are kept for when
    /// it is turned back on.
    #[serde(default)]
    pub experimental_enabled: bool,
}

fn default_update_checks_enabled() -> bool {
    true
}

fn default_show_tray_icon() -> bool {
    true
}

fn default_locale() -> String {
    "en".to_string()
}
