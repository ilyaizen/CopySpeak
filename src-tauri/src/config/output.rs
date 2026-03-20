// Output configuration: audio format, file output, history storage, and filename patterns.

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::{ValidationError, GENERATION_COUNTER};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Wav,
    Mp3,
    Ogg,
    Flac,
}

impl AudioFormat {
    pub fn default_extension(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Flac => "flac",
        }
    }

    #[allow(dead_code)]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(AudioFormat::Wav),
            "mp3" => Some(AudioFormat::Mp3),
            "ogg" => Some(AudioFormat::Ogg),
            "flac" => Some(AudioFormat::Flac),
            _ => None,
        }
    }
}

impl Default for AudioFormat {
    fn default() -> Self {
        AudioFormat::Wav
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormatConfig {
    pub format: AudioFormat,
    pub mp3_bitrate: u32,
    pub ogg_bitrate: u32,
    pub flac_compression: u8,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::Wav,
            mp3_bitrate: 192,
            ogg_bitrate: 192,
            flac_compression: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub enabled: bool,
    pub directory: String,
    pub filename_pattern: String,
    pub format_config: FormatConfig,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: String::new(),
            filename_pattern: "copyspeak-{compact_datetime}-{seq}.wav".into(),
            format_config: FormatConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Temp,
    Persistent,
}

impl Default for StorageMode {
    fn default() -> Self {
        Self::Persistent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AutoDeleteMode {
    KeepLatest(u32),
    Never,
    AfterDays(u32),
}

impl Default for AutoDeleteMode {
    fn default() -> Self {
        Self::KeepLatest(15)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
    #[serde(default)]
    pub storage_mode: StorageMode,
    #[serde(default)]
    pub persistent_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_delete: AutoDeleteMode,
    pub cleanup_orphaned_files: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            storage_mode: StorageMode::Persistent,
            persistent_dir: None,
            auto_delete: AutoDeleteMode::KeepLatest(15),
            cleanup_orphaned_files: true,
        }
    }
}

impl HistoryConfig {
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        match &self.auto_delete {
            AutoDeleteMode::KeepLatest(val) => {
                if *val < 1 {
                    errors.push(ValidationError::MaxEntriesTooSmall {
                        value: *val,
                        min: 1,
                    });
                }
                if *val > 50000 {
                    errors.push(ValidationError::MaxEntriesTooLarge {
                        value: *val,
                        max: 50000,
                    });
                }
            }
            AutoDeleteMode::AfterDays(val) => {
                if *val < 1 {
                    errors.push(ValidationError::MaxAgeDaysTooSmall {
                        value: *val,
                        min: 1,
                    });
                }
                if *val > 365 {
                    errors.push(ValidationError::MaxAgeDaysTooLarge {
                        value: *val,
                        max: 365,
                    });
                }
            }
            AutoDeleteMode::Never => {}
        }

        errors
    }
}

/// Expand placeholders in a filename pattern.
///
/// Supported placeholders:
/// - `{timestamp}` - Unix timestamp in seconds (e.g., "1707235200")
/// - `{datetime}` - ISO-like format without colons for filesystem compatibility (e.g., "2024-02-06_143022")
/// - `{date}` - Date in YYYY-MM-DD format (e.g., "2024-02-06")
/// - `{time}` - Time in HHMMSS format, no colons for filesystem compatibility (e.g., "143022")
/// - `{voice}` - Current voice ID (e.g., "af_heart")
/// - `{text}` - First N characters of the spoken text, sanitized for filenames
///
/// # Arguments
/// * `pattern` - The filename pattern with placeholders (e.g., "copyspeak_{timestamp}.wav")
/// * `voice` - The voice ID to substitute for `{voice}`
/// * `text` - The spoken text to substitute for `{text}` (truncated and sanitized)
///
/// # Returns
/// The expanded filename with all placeholders replaced.
pub fn expand_filename_pattern(pattern: &str, voice: &str, text: &str) -> String {
    let now = Local::now();

    // Sanitize text for use in filename: keep only alphanumeric and spaces,
    // replace spaces with underscores, truncate to 30 chars
    let sanitized_text: String = text
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .take(30)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase();

    // Sanitize voice for filesystem compatibility
    let sanitized_voice: String = voice
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    let seq = GENERATION_COUNTER.fetch_add(1, Ordering::SeqCst);

    pattern
        .replace("{timestamp}", &now.timestamp().to_string())
        .replace("{compact_datetime}", &now.format("%Y%m%d%H%M").to_string())
        .replace("{datetime}", &now.format("%Y-%m-%d_%H%M%S").to_string())
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H%M%S").to_string())
        .replace("{voice}", &sanitized_voice)
        .replace("{text}", &sanitized_text)
        .replace("{seq}", &seq.to_string())
}
