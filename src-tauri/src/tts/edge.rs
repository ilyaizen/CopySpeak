// Edge-TTS backend (subprocess via Python edge-tts CLI).
//
// Edge-TTS (rany2/edge-tts) connects to Microsoft's free Read Aloud
// WebSocket endpoint. No API key required. The CLI is pip-installed
// (`pip install edge-tts`) and invoked as a subprocess: text is written
// to a temp file (`-f`) and audio is written to a temp MP3 (`--write-media`).

use super::{TtsBackend, TtsError};
use crate::config::EdgeTtsConfig;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct EdgeTtsBackend {
    config: EdgeTtsConfig,
}

impl EdgeTtsBackend {
    pub fn new(config: EdgeTtsConfig) -> Self {
        Self { config }
    }

    /// Convert a playback speed multiplier (1.0 = normal) to an edge-tts
    /// rate string like "+35%" or "-10%".
    fn speed_to_rate(speed: f32) -> String {
        let pct = (speed - 1.0) * 100.0;
        format!("{pct:+.0}%")
    }

    fn input_path() -> String {
        let tmp = std::env::temp_dir();
        tmp.join("copyspeak_edge_input.txt")
            .to_string_lossy()
            .into_owned()
    }

    fn output_path() -> String {
        let tmp = std::env::temp_dir();
        tmp.join("copyspeak_edge_out.mp3")
            .to_string_lossy()
            .into_owned()
    }
}

impl TtsBackend for EdgeTtsBackend {
    fn name(&self) -> &str {
        "Edge-TTS"
    }

    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        let input_path = Self::input_path();
        let output_path = Self::output_path();

        // Write text to temp file (edge-tts reads via -f, not stdin).
        std::fs::write(&input_path, text).map_err(TtsError::Io)?;
        let _ = std::fs::remove_file(&output_path);

        let rate = Self::speed_to_rate(speed);

        log::info!(
            "[Edge-TTS] Synthesizing {} chars, voice: {}, rate: {}",
            text.len(),
            voice,
            rate
        );

        let exec_start = std::time::Instant::now();

        #[allow(unused_mut)]
        let mut cmd = Command::new("edge-tts");
        cmd.arg("--file")
            .arg(&input_path)
            .arg("--voice")
            .arg(voice)
            .arg("--rate")
            .arg(&rate)
            .arg("--write-media")
            .arg(&output_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.env("PATH", crate::tts::cli::get_expanded_path());
        }

        let child = cmd.spawn().map_err(|e| {
            TtsError::Unavailable(format!(
                "edge-tts not found. Install with: pip install edge-tts\n\nError: {e}"
            ))
        })?;

        // Store PID so abort_synthesis can kill this process.
        crate::ACTIVE_CLI_PID.store(child.id(), std::sync::atomic::Ordering::Relaxed);

        let result = child
            .wait_with_output()
            .map_err(|e| TtsError::Unavailable(format!("edge-tts failed: {e}")))?;

        crate::ACTIVE_CLI_PID.store(0, std::sync::atomic::Ordering::Relaxed);

        let elapsed = exec_start.elapsed();
        log::info!(
            "[Edge-TTS] Process exited {} in {:?}",
            result.status,
            elapsed
        );

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let stdout = String::from_utf8_lossy(&result.stdout);
            log::error!("[Edge-TTS] stderr: {}", stderr.trim());
            log::error!("[Edge-TTS] stdout: {}", stdout.trim());
            return Err(TtsError::CommandFailed(format!(
                "edge-tts exit {}: {}",
                result.status,
                stderr.trim()
            )));
        }

        if !std::path::Path::new(&output_path).exists() {
            return Err(TtsError::OutputNotFound(format!(
                "edge-tts succeeded but output file was not created: {output_path}"
            )));
        }

        let bytes = std::fs::read(&output_path).map_err(|e| {
            log::error!("[Edge-TTS] Failed to read output '{}': {}", output_path, e);
            TtsError::Io(e)
        })?;

        log::info!("[Edge-TTS] Synthesis complete: {} MP3 bytes", bytes.len());
        Ok(bytes)
    }

    fn health_check(&self) -> Result<(), TtsError> {
        let voice = if self.config.voice.trim().is_empty() {
            "en-US-AvaMultilingualNeural"
        } else {
            self.config.voice.trim()
        };

        log::debug!(
            "[Edge-TTS] Health check — attempting short synthesis with voice: {voice}"
        );

        // A real one-word synthesis is the most reliable check: it verifies
        // the binary exists, Python deps are intact, and the network endpoint
        // is reachable. Cheaper than a voice-list API call and catches more
        // failure modes.
        self.synthesize("test", voice, 1.0).map(|_| ())
    }

    fn file_extension(&self) -> &str {
        "mp3"
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        // "en-US-EmmaMultilingualNeural" → "emma"
        voice_id
            .split('-')
            .nth(2)
            .unwrap_or(voice_id)
            .trim_end_matches("Neural")
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_to_rate() {
        assert_eq!(EdgeTtsBackend::speed_to_rate(1.0), "+0%");
        assert_eq!(EdgeTtsBackend::speed_to_rate(1.35), "+35%");
        assert_eq!(EdgeTtsBackend::speed_to_rate(0.5), "-50%");
        assert_eq!(EdgeTtsBackend::speed_to_rate(2.0), "+100%");
    }

    #[test]
    fn test_voice_display_name() {
        let backend = EdgeTtsBackend::new(EdgeTtsConfig::default());
        // "en-US-EmmaMultilingualNeural" → split[2]="EmmaMultilingualNeural"
        // → trim "Neural" suffix → "EmmaMultilingual" → lowercase
        assert_eq!(
            backend.voice_display_name("en-US-EmmaMultilingualNeural"),
            "emmamultilingual"
        );
        assert_eq!(
            backend.voice_display_name("en-GB-SoniaNeural"),
            "sonia"
        );
        assert_eq!(
            backend.voice_display_name("en-US-AriaNeural"),
            "aria"
        );
    }
}
