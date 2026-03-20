// Config: typed settings struct, persisted as JSON to %APPDATA%/CopySpeak/config.json.
// Auto-saved on every change from the frontend via set_config command.

/// Current config schema version. Bumped when making breaking changes to config structure.
const CONFIG_VERSION: &str = "0.0.2";

mod general;
mod hud;
mod output;
mod playback;
mod sanitization;
mod trigger;
mod tts;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::{Mutex, OnceLock};

// Re-export all public types so external code can use `crate::config::*` unchanged.
pub use general::*;
pub use hud::*;
pub use output::*;
pub use playback::*;
pub use sanitization::*;
pub use trigger::*;
pub use tts::*;

/// Global sequence counter for generation filenames. Used by output.rs for
/// user-configured output patterns via the {seq} placeholder.
pub static GENERATION_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Per-minute counter for history audio filenames. Tracks the current minute
/// key and count; resets automatically when the minute changes.
static MINUTE_COUNTER: OnceLock<Mutex<(String, u32)>> = OnceLock::new();

/// Returns the count for this minute (1 = first, 2 = second, etc.).
/// Thread-safe; resets to 1 when the minute key changes.
pub fn get_and_increment_minute_counter(minute_key: &str) -> u32 {
    let mutex = MINUTE_COUNTER.get_or_init(|| Mutex::new((String::new(), 0)));
    let mut state = mutex.lock().unwrap();
    if state.0 != minute_key {
        *state = (minute_key.to_string(), 1);
    } else {
        state.1 += 1;
    }
    state.1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationError {
    CommandEmpty,
    ArgsTemplateMissingPlaceholder { placeholder: String },
    DoubleCopyWindowTooSmall { value: u64, min: u64 },
    MaxTextLengthTooSmall { value: u64, min: u64 },
    MaxEntriesTooSmall { value: u32, min: u32 },
    MaxEntriesTooLarge { value: u32, max: u32 },
    MaxAgeDaysTooSmall { value: u32, min: u32 },
    MaxAgeDaysTooLarge { value: u32, max: u32 },
    AutoCleanupIntervalTooSmall { value: u32, min: u32 },
    AutoCleanupIntervalTooLarge { value: u32, max: u32 },
    OpacityOutOfRange { value: f32, min: f32, max: f32 },
    HudWidthTooSmall { value: u32, min: u32 },
    HudHeightTooSmall { value: u32, min: u32 },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::CommandEmpty => write!(f, "TTS command cannot be empty"),
            ValidationError::ArgsTemplateMissingPlaceholder { placeholder } => {
                write!(
                    f,
                    "Args template missing required placeholder: {}",
                    placeholder
                )
            }
            ValidationError::DoubleCopyWindowTooSmall { value, min } => {
                write!(
                    f,
                    "Double-copy window {}ms is too small (minimum: {}ms)",
                    value, min
                )
            }
            ValidationError::MaxTextLengthTooSmall { value, min } => {
                write!(
                    f,
                    "Max text length {} is too small (minimum: {})",
                    value, min
                )
            }
            ValidationError::MaxEntriesTooSmall { value, min } => {
                write!(
                    f,
                    "Max history entries {} is too small (minimum: {})",
                    value, min
                )
            }
            ValidationError::MaxEntriesTooLarge { value, max } => {
                write!(
                    f,
                    "Max history entries {} is too large (maximum: {})",
                    value, max
                )
            }
            ValidationError::MaxAgeDaysTooSmall { value, min } => {
                write!(
                    f,
                    "Max history age {} days is too small (minimum: {})",
                    value, min
                )
            }
            ValidationError::MaxAgeDaysTooLarge { value, max } => {
                write!(
                    f,
                    "Max history age {} days is too large (maximum: {})",
                    value, max
                )
            }
            ValidationError::AutoCleanupIntervalTooSmall { value, min } => {
                write!(
                    f,
                    "Auto cleanup interval {} hours is too small (minimum: {})",
                    value, min
                )
            }
            ValidationError::AutoCleanupIntervalTooLarge { value, max } => {
                write!(
                    f,
                    "Auto cleanup interval {} hours is too large (maximum: {})",
                    value, max
                )
            }
            ValidationError::OpacityOutOfRange { value, min, max } => {
                write!(f, "Opacity {} is out of range [{}, {}]", value, min, max)
            }
            ValidationError::HudWidthTooSmall { value, min } => {
                write!(f, "HUD width {} is too small (minimum: {})", value, min)
            }
            ValidationError::HudHeightTooSmall { value, min } => {
                write!(f, "HUD height {} is too small (minimum: {})", value, min)
            }
        }
    }
}

pub type ValidationResult = Result<(), Vec<ValidationError>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_config_version")]
    pub version: String,
    pub general: GeneralConfig,
    pub trigger: TriggerConfig,
    pub tts: TtsConfig,
    pub playback: PlaybackConfig,
    pub hud: HudConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub sanitization: SanitizationConfig,
    #[serde(default)]
    pub pagination: PaginationConfig,
    #[serde(default)]
    pub history: HistoryConfig,
}

fn default_config_version() -> String {
    CONFIG_VERSION.into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION.into(),
            general: GeneralConfig {
                start_with_windows: false,
                start_minimized: true,
                show_notifications: true,
                debug_mode: false,
                close_behavior: CloseBehavior::default(),
                appearance: AppearanceMode::default(),
                update_checks_enabled: true,
                locale: "en".to_string(),
            },
            trigger: TriggerConfig {
                double_copy_window_ms: 1500,
                max_text_length: 100000,
            },
            tts: TtsConfig::default(),
            playback: PlaybackConfig {
                on_retrigger: RetriggerMode::Queue,
                volume: 100,
                playback_speed: 1.35,
                pitch: 1.15,
            },
            hud: HudConfig {
                enabled: true,
                position: HudPosition::Preset(HudPresetPosition::BottomCenter),
                width: 300,
                height: 100,
                opacity: 0.85,
            },
            output: OutputConfig::default(),
            sanitization: SanitizationConfig::default(),
            pagination: PaginationConfig::default(),
            history: HistoryConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();

        errors.extend(self.trigger.validate());
        errors.extend(self.tts.validate());
        errors.extend(self.hud.validate());
        errors.extend(self.history.validate());

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Returns the config file path: %APPDATA%/CopySpeak/config.json
pub fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("CopySpeak").join("config.json")
}

/// Load config from disk, or return defaults if missing/corrupt.
pub fn load_or_default() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            // Use a permissive Value parse to detect the old "http" active_backend before
            // deserializing into AppConfig (which no longer has the Http variant).
            let raw: serde_json::Value = serde_json::from_str(&contents).unwrap_or_default();
            let had_http_backend = raw
                .get("tts")
                .and_then(|t| t.get("active_backend"))
                .and_then(|b| b.as_str())
                == Some("http");

            let mut cfg: AppConfig = serde_json::from_str(&contents).unwrap_or_else(|e| {
                log::warn!("Config parse error, using defaults: {e}");
                AppConfig::default()
            });

            if had_http_backend {
                log::warn!(
                    "Config had deprecated HTTP TTS engine; migrating active_backend to Local"
                );
                cfg.tts.active_backend = TtsEngine::Local;
            }

            cfg
        }
        Err(_) => {
            log::info!("No config found at {}, using defaults", path.display());
            AppConfig::default()
        }
    }
}

/// Save config to disk. Creates parent directory if needed.
pub fn save(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}
