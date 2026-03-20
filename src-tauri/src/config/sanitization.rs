// Sanitization and pagination configuration for text preprocessing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SanitizationConfig {
    pub enabled: bool,
    pub markdown: MarkdownSanitizationConfig,
    pub tts_normalization: TtsNormalizationConfig,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            markdown: MarkdownSanitizationConfig::default(),
            tts_normalization: TtsNormalizationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkdownSanitizationConfig {
    pub enabled: bool,
    pub strip_headers: bool,
    pub strip_code_blocks: bool,
    pub strip_inline_code: bool,
    pub strip_links: bool,
    pub strip_bold_italic: bool,
    pub strip_lists: bool,
    pub strip_blockquotes: bool,
}

impl Default for MarkdownSanitizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strip_headers: true,
            strip_code_blocks: true,
            strip_inline_code: true,
            strip_links: true,
            strip_bold_italic: true,
            strip_lists: true,
            strip_blockquotes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsNormalizationConfig {
    pub enabled: bool,
}

impl Default for TtsNormalizationConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Configuration for text pagination/chunking for long texts.
/// When enabled, long texts are split into fragments for progressive TTS playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaginationConfig {
    /// Whether pagination is enabled for long texts
    pub enabled: bool,
    /// Maximum size of each text fragment in characters
    pub fragment_size: u32,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fragment_size: 800,
        }
    }
}
