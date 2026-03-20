// Telemetry: synthesis timing data for ETA estimation.
// Tracks synthesis duration per backend/voice/character-bucket to predict future job times.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Character count buckets for timing estimates.
/// Smaller buckets for short texts (higher variance), larger for long texts.
const CHAR_BUCKETS: &[u32] = &[0, 500, 2000, 5000, 10000, std::u32::MAX];

/// Get the bucket index for a given character count.
fn get_bucket_index(char_count: usize) -> usize {
    let count = char_count as u32;
    for (i, &threshold) in CHAR_BUCKETS.iter().enumerate() {
        if count < threshold {
            return i.saturating_sub(1);
        }
    }
    CHAR_BUCKETS.len() - 2
}

/// Get the bucket label for display/debugging.
#[allow(dead_code)]
fn get_bucket_label(char_count: usize) -> String {
    let idx = get_bucket_index(char_count);
    let start = CHAR_BUCKETS[idx];
    let end = CHAR_BUCKETS.get(idx + 1).copied().unwrap_or(std::u32::MAX);
    if end == std::u32::MAX {
        format!("{}+", start)
    } else {
        format!("{}-{}", start, end)
    }
}

/// Single timing entry for a synthesis operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TimingSample {
    pub duration_ms: u64,
    pub char_count: u32,
    pub timestamp: DateTime<Utc>,
}

/// Aggregated timing data for a specific backend/voice/bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingStats {
    /// Exponential moving average of duration (ms)
    pub ema_duration_ms: f64,
    /// Number of samples collected
    pub sample_count: u32,
    /// Last time this was updated
    pub last_updated: DateTime<Utc>,
    /// Average characters per millisecond (for interpolation)
    pub chars_per_ms: f64,
}

impl Default for TimingStats {
    fn default() -> Self {
        Self {
            ema_duration_ms: 0.0,
            sample_count: 0,
            last_updated: Utc::now(),
            chars_per_ms: 0.0,
        }
    }
}

impl TimingStats {
    /// EMA smoothing factor (0.3 = moderately responsive)
    const EMA_ALPHA: f64 = 0.3;

    /// Add a new sample and update the moving average.
    pub fn add_sample(&mut self, duration_ms: u64, char_count: u32) {
        let chars = char_count as f64;
        let duration = duration_ms as f64;

        if self.sample_count == 0 {
            self.ema_duration_ms = duration;
            self.chars_per_ms = chars / duration.max(1.0);
        } else {
            self.ema_duration_ms =
                Self::EMA_ALPHA * duration + (1.0 - Self::EMA_ALPHA) * self.ema_duration_ms;
            let new_chars_per_ms = chars / duration.max(1.0);
            self.chars_per_ms =
                Self::EMA_ALPHA * new_chars_per_ms + (1.0 - Self::EMA_ALPHA) * self.chars_per_ms;
        }

        self.sample_count += 1;
        self.last_updated = Utc::now();
    }

    /// Estimate duration for a given character count.
    /// Returns None if insufficient data.
    pub fn estimate_duration(&self, char_count: u32) -> Option<u64> {
        if self.sample_count == 0 {
            return None;
        }

        let chars = char_count as f64;
        let estimated = chars / self.chars_per_ms.max(0.001);
        Some(estimated.max(100.0) as u64)
    }
}

/// Key for timing data lookup: backend + voice + bucket.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TimingKey {
    pub backend: String,
    pub voice: String,
    pub bucket: usize,
}

impl TimingKey {
    fn to_string_key(&self) -> String {
        format!("{}|{}|{}", self.backend, self.voice, self.bucket)
    }

    fn from_string_key(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(3, '|').collect();
        if parts.len() == 3 {
            Some(Self {
                backend: parts[0].to_string(),
                voice: parts[1].to_string(),
                bucket: parts[2].parse().ok()?,
            })
        } else {
            None
        }
    }
}

impl Serialize for TimingKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string_key())
    }
}

impl<'de> Deserialize<'de> for TimingKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_string_key(&s)
            .ok_or_else(|| serde::de::Error::custom("invalid timing key format"))
    }
}

/// Global telemetry storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryLog {
    /// Map of timing stats per backend/voice/bucket
    pub stats: HashMap<TimingKey, TimingStats>,
    /// Metadata about the telemetry log
    pub metadata: TelemetryMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMetadata {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub total_samples_recorded: u64,
}

impl Default for TelemetryLog {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryLog {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            stats: HashMap::new(),
            metadata: TelemetryMetadata {
                version: "1.0".to_string(),
                created_at: now,
                last_modified: now,
                total_samples_recorded: 0,
            },
        }
    }

    /// Record a synthesis timing sample.
    pub fn record(&mut self, backend: &str, voice: &str, char_count: usize, duration_ms: u64) {
        let bucket = get_bucket_index(char_count);
        let key = TimingKey {
            backend: backend.to_string(),
            voice: voice.to_string(),
            bucket,
        };

        let stats = self.stats.entry(key).or_default();
        stats.add_sample(duration_ms, char_count as u32);

        self.metadata.last_modified = Utc::now();
        self.metadata.total_samples_recorded += 1;
    }

    /// Get an estimate for synthesis duration.
    /// Returns (estimated_ms, confidence) where confidence is 0-1 based on sample count.
    pub fn estimate(&self, backend: &str, voice: &str, char_count: usize) -> (Option<u64>, f32) {
        let bucket = get_bucket_index(char_count);
        let key = TimingKey {
            backend: backend.to_string(),
            voice: voice.to_string(),
            bucket,
        };

        match self.stats.get(&key) {
            Some(stats) => {
                let estimated = stats.estimate_duration(char_count as u32);
                let confidence = (stats.sample_count as f32 / 10.0).min(1.0);
                (estimated, confidence)
            }
            None => (None, 0.0),
        }
    }

    /// Estimate total duration for paginated text.
    /// Returns per-fragment estimates if available.
    pub fn estimate_paginated(
        &self,
        backend: &str,
        voice: &str,
        fragment_char_counts: &[usize],
    ) -> (Option<u64>, f32, Vec<Option<u64>>) {
        let mut total_estimate: Option<u64> = None;
        let mut total_confidence = 0.0;
        let mut fragment_estimates = Vec::with_capacity(fragment_char_counts.len());

        for &char_count in fragment_char_counts {
            let (est, conf) = self.estimate(backend, voice, char_count);
            fragment_estimates.push(est);

            if let Some(ms) = est {
                total_estimate = Some(total_estimate.unwrap_or(0) + ms);
                total_confidence += conf as f64;
            }
        }

        let avg_confidence = if fragment_char_counts.is_empty() {
            0.0
        } else {
            (total_confidence / fragment_char_counts.len() as f64) as f32
        };

        (total_estimate, avg_confidence, fragment_estimates)
    }
}

/// Get the path to the telemetry.json file.
fn telemetry_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("CopySpeak").join("telemetry.json")
}

/// Load telemetry from disk, creating default if not found.
pub fn load() -> TelemetryLog {
    let path = telemetry_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("Telemetry parse error, starting fresh: {e}");
            TelemetryLog::new()
        }),
        Err(_) => {
            log::debug!("No telemetry found at {}, starting fresh", path.display());
            TelemetryLog::new()
        }
    }
}

/// Save telemetry to disk.
pub fn save(telemetry: &TelemetryLog) {
    let path = telemetry_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(telemetry);
    match json {
        Ok(j) => {
            if let Err(e) = std::fs::write(&path, j) {
                log::warn!("Failed to save telemetry: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize telemetry: {e}"),
    }
}

/// Record a timing sample and persist.
pub fn record_sample(
    telemetry: &Mutex<TelemetryLog>,
    backend: &str,
    voice: &str,
    char_count: usize,
    duration_ms: u64,
) {
    let mut tel = telemetry.lock().unwrap();
    tel.record(backend, voice, char_count, duration_ms);
    save(&tel);
}

/// Get an estimate without modifying the log.
pub fn get_estimate(
    telemetry: &Mutex<TelemetryLog>,
    backend: &str,
    voice: &str,
    char_count: usize,
) -> (Option<u64>, f32) {
    let tel = telemetry.lock().unwrap();
    tel.estimate(backend, voice, char_count)
}

/// Get estimates for paginated text.
pub fn get_estimate_paginated(
    telemetry: &Mutex<TelemetryLog>,
    backend: &str,
    voice: &str,
    fragment_char_counts: &[usize],
) -> (Option<u64>, f32, Vec<Option<u64>>) {
    let tel = telemetry.lock().unwrap();
    tel.estimate_paginated(backend, voice, fragment_char_counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_index() {
        assert_eq!(get_bucket_index(100), 0);
        assert_eq!(get_bucket_index(500), 1);
        assert_eq!(get_bucket_index(1500), 1);
        assert_eq!(get_bucket_index(2500), 2);
        assert_eq!(get_bucket_index(6000), 3);
        assert_eq!(get_bucket_index(15000), 4);
    }

    #[test]
    fn test_timing_stats_ema() {
        let mut stats = TimingStats::default();
        stats.add_sample(1000, 500);
        assert_eq!(stats.sample_count, 1);
        assert!(stats.ema_duration_ms > 0.0);

        stats.add_sample(2000, 500);
        assert_eq!(stats.sample_count, 2);
        assert!(stats.ema_duration_ms < 2000.0);
        assert!(stats.ema_duration_ms > 1000.0);
    }

    #[test]
    fn test_estimate() {
        let mut log = TelemetryLog::new();

        let (est, conf) = log.estimate("Local", "voice1", 500);
        assert!(est.is_none());
        assert_eq!(conf, 0.0);

        log.record("Local", "voice1", 500, 1000);
        let (est, conf) = log.estimate("Local", "voice1", 500);
        assert!(est.is_some());
        assert!(conf < 1.0);

        for i in 0..20 {
            log.record("Local", "voice1", 500, 1000 + i * 10);
        }
        let (_, conf) = log.estimate("Local", "voice1", 500);
        assert!((conf - 1.0).abs() < 0.01);
    }
}
