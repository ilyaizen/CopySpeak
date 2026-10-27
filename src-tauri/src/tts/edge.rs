// Edge-TTS backend (subprocess via Python edge-tts CLI).
//
// Edge-TTS (rany2/edge-tts) connects to Microsoft's free Read Aloud
// WebSocket endpoint. No API key required. uv runs the pinned client with an
// embedded wrapper; input, MP3 audio and word-boundary sidecars use temp
// files. The pin (7.3.1) matters: 7.3.x moved Read Aloud to Microsoft's
// current host and handles the 403 clock-skew retry themselves — the
// previously pinned 7.2.8 spoke a protocol the service refuses.
//
// The service throttles aggressively, so requests share a process-wide
// cooldown that survives backend recreation and retrigger: `Retry-After`
// when the provider names one (clamped — a hostile or buggy header must not
// disable the engine for the life of the process), else an escalating
// 5/10/20 s ladder capped at 60 s. Request starts are spaced by at least one
// second. Cooldown state lasts for this app process, not across restarts.

use super::{TtsBackend, TtsError};
use crate::config::EdgeTtsConfig;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};

static REQUEST_LOCK: Mutex<()> = Mutex::new(());
static COOLDOWN: Mutex<EdgeCooldown> = Mutex::new(EdgeCooldown {
    until: None,
    strikes: 0,
});
const MAX_WAIT: Duration = Duration::from_secs(30);
const REQUEST_SPACING: Duration = Duration::from_secs(1);
/// Hard ceiling for a provider-requested pause. `Retry-After` is response
/// metadata, so an extreme value must not disable the engine for the life of
/// the process (an unclamped `u64::MAX` once fell back to a ten-year cooldown).
const MAX_COOLDOWN: Duration = Duration::from_secs(300);

struct EdgeCooldown {
    until: Option<Instant>,
    strikes: u32,
}

impl EdgeCooldown {
    fn remaining(&self, now: Instant) -> Duration {
        self.until
            .map(|until| until.saturating_duration_since(now))
            .unwrap_or_default()
    }

    fn rate_limited(&mut self, now: Instant, retry_after: Option<u64>) -> Duration {
        self.strikes = self.strikes.saturating_add(1);
        let fallback = 5u64 * (1u64 << self.strikes.saturating_sub(1).min(4));
        let seconds = retry_after
            .unwrap_or(fallback.min(60))
            .clamp(1, MAX_COOLDOWN.as_secs());
        let until = now + Duration::from_secs(seconds);
        self.until = Some(
            self.until
                .map(|previous| previous.max(until))
                .unwrap_or(until),
        );
        self.remaining(now)
    }

    fn succeeded(&mut self) {
        self.strikes = 0;
    }

    fn reserve_start(&mut self, now: Instant) {
        let until = now + REQUEST_SPACING;
        self.until = Some(
            self.until
                .map(|previous| previous.max(until))
                .unwrap_or(until),
        );
    }
}

#[derive(serde::Deserialize)]
struct EdgeFailure {
    kind: String,
    status: Option<u16>,
    retry_after_seconds: Option<u64>,
}

/// The wrapper's structured `[edge-error]` line — the only stderr content
/// this backend interprets. Everything else in stderr is untrusted
/// diagnostics that must not reach the UI.
fn parse_failure(stderr: &str) -> Option<EdgeFailure> {
    stderr
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.strip_prefix("[edge-error] ")?).ok())
}

fn safe_failure(failure: Option<&EdgeFailure>) -> String {
    match failure {
        Some(error) if error.kind == "http" => match error.status {
            Some(429) => "Edge HTTP 429: Microsoft is limiting requests. A shared cooldown applies to new readings too; wait before trying again.".into(),
            Some(403) => "Edge HTTP 403: Microsoft refused the speech connection. Try again later or select another engine.".into(),
            Some(status @ 400..=599) => format!("Edge HTTP {status}: the speech service rejected the request."),
            _ => "Edge speech request failed. Check the engine setup or select another engine.".into(),
        },
        Some(error) if error.kind == "timeout" => "Edge request timed out.".into(),
        Some(error) if error.kind == "connection" => {
            "Edge connection closed before synthesis completed.".into()
        }
        Some(error) if error.kind == "no_audio" => "Edge: no audio was received.".into(),
        _ => "Edge speech request failed. Check the engine setup or select another engine.".into(),
    }
}

/// Poll a cooldown until it expires, or fail into cancellation / the wait
/// budget. A longer server delay is retained by the cooldown itself (up to
/// [`MAX_COOLDOWN`]) without reconnecting early.
fn wait_for_slot(
    mut remaining: impl FnMut() -> Duration,
    cancelled: impl Fn() -> bool,
    budget: Duration,
) -> Result<(), TtsError> {
    let started = Instant::now();
    loop {
        if cancelled() {
            return Err(TtsError::Unavailable("Reading cancelled".into()));
        }
        let delay = remaining();
        if delay.is_zero() {
            return Ok(());
        }
        let available = budget.saturating_sub(started.elapsed());
        if delay > available || available.is_zero() {
            return Err(TtsError::Unavailable(
                "Edge is cooling down. Stop repeated attempts and try again later, or select another engine."
                    .into(),
            ));
        }
        std::thread::sleep(delay.min(Duration::from_millis(50)));
    }
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct EdgeTtsBackend {
    config: EdgeTtsConfig,
    /// Set when the last request ended in a provider-side refusal (429/403).
    /// Read by [`TtsBackend::failure_is_final`] so the command layer does not
    /// amplify throttling: every retry is another request against a provider
    /// that just said "too many", and each one escalates the shared cooldown.
    refused: AtomicBool,
}

impl EdgeTtsBackend {
    pub fn new(config: EdgeTtsConfig) -> Self {
        Self {
            config,
            refused: AtomicBool::new(false),
        }
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

    /// One request against the shared cooldown.
    ///
    /// Cancellation polls the abort generation from THIS request's start
    /// (`AbortScope::begin`), so a stale Stop from a previous reading never
    /// cancels the Engines-page health check, while a Stop during a real
    /// reading interrupts the waits immediately.
    fn request(&self, text: &str, voice: &str) -> Result<super::captions::SpeechAudio, TtsError> {
        self.refused.store(false, Ordering::Relaxed);
        let scope = crate::AbortScope::begin();
        let cancelled = || scope.aborted();
        let started = Instant::now();
        // One Edge request at a time, process-wide: overlapping readings would
        // each look like a burst to the provider and earn a shared 429.
        let _request = loop {
            if cancelled() {
                return Err(TtsError::Unavailable("Reading cancelled".into()));
            }
            match REQUEST_LOCK.try_lock() {
                Ok(guard) => break guard,
                Err(TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    if started.elapsed() >= MAX_WAIT {
                        return Err(TtsError::Unavailable(
                            "Edge is busy. Try again later.".into(),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        };
        let remaining = COOLDOWN
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remaining(Instant::now());
        if !remaining.is_zero() {
            log::info!(
                "[Edge-TTS] Waiting for shared request cooldown ({} ms)",
                remaining.as_millis()
            );
        }
        wait_for_slot(
            || {
                COOLDOWN
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remaining(Instant::now())
            },
            cancelled,
            MAX_WAIT.saturating_sub(started.elapsed()),
        )?;
        let input_path = Self::input_path();
        let output_path = Self::output_path();

        // Write text to temp file (edge-tts reads via -f, not stdin).
        std::fs::write(&input_path, text).map_err(TtsError::Io)?;
        let _ = std::fs::remove_file(&output_path);
        let _ = std::fs::remove_file(format!("{output_path}.captions.json"));

        log::info!(
            "[Edge-TTS] Synthesizing {} chars, voice: {}",
            text.len(),
            voice
        );

        let exec_start = std::time::Instant::now();

        #[allow(unused_mut)]
        let mut cmd = Command::new("uv");
        cmd.args([
            "run",
            "--no-project",
            "--with",
            "edge-tts==7.3.1",
            "python",
            "-c",
        ])
        .arg(include_str!("../../../scripts/edge/copyspeak-edge.py"))
        .arg(&input_path)
        .arg(&output_path)
        .arg(voice)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.env("PATH", crate::tts::cli::get_expanded_path());
        }

        if cancelled() {
            return Err(TtsError::Unavailable("Reading cancelled".into()));
        }
        COOLDOWN
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .reserve_start(Instant::now());
        let child = cmd.spawn().map_err(|_| {
            TtsError::Unavailable(
                "Could not start uv. Install uv to run the pinned Edge speech client.".into(),
            )
        })?;

        // Store PID so abort_synthesis can kill this process.
        crate::ACTIVE_CLI_PID.store(child.id(), std::sync::atomic::Ordering::Relaxed);

        let result = child.wait_with_output();
        crate::ACTIVE_CLI_PID.store(0, std::sync::atomic::Ordering::Relaxed);
        let result = result
            .map_err(|_| TtsError::Unavailable("Could not collect Edge speech output.".into()))?;

        let elapsed = exec_start.elapsed();
        log::info!(
            "[Edge-TTS] Process exited {} in {:?}",
            result.status,
            elapsed
        );

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let failure = parse_failure(&stderr);
            if let Some(error) = failure
                .as_ref()
                .filter(|error| error.kind == "http" && error.status == Some(429))
            {
                let delay = COOLDOWN
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .rate_limited(Instant::now(), error.retry_after_seconds);
                log::warn!(
                    "[Edge-TTS] HTTP 429; shared cooldown {} seconds",
                    delay.as_secs()
                );
            }
            let message = safe_failure(failure.as_ref());
            log::error!("[Edge-TTS] {message}");
            if failure.as_ref().is_some_and(|error| {
                error.kind == "http" && matches!(error.status, Some(403 | 429))
            }) {
                self.refused.store(true, Ordering::Relaxed);
            }
            return Err(TtsError::CommandFailed(message));
        }
        COOLDOWN
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .succeeded();

        if !std::path::Path::new(&output_path).exists() {
            return Err(TtsError::OutputNotFound(
                "Edge did not create an audio file.".into(),
            ));
        }

        let bytes = std::fs::read(&output_path)
            .map_err(|_| TtsError::Unavailable("Could not read Edge audio output.".into()))?;

        log::info!("[Edge-TTS] Synthesis complete: {} MP3 bytes", bytes.len());
        let captions = super::captions::read_sidecar(&output_path);
        Ok(super::captions::SpeechAudio { bytes, captions })
    }
}

impl TtsBackend for EdgeTtsBackend {
    fn name(&self) -> &str {
        "Edge-TTS"
    }

    fn synthesize(&self, text: &str, voice: &str) -> Result<Vec<u8>, TtsError> {
        self.request(text, voice).map(|speech| speech.bytes)
    }

    fn synthesize_with_captions(
        &self,
        text: &str,
        voice: &str,
    ) -> Result<super::captions::SpeechAudio, TtsError> {
        self.request(text, voice)
    }

    fn health_check(&self) -> Result<(), TtsError> {
        let voice = if self.config.voice.trim().is_empty() {
            "en-US-AvaMultilingualNeural"
        } else {
            self.config.voice.trim()
        };

        log::debug!("[Edge-TTS] Health check — attempting short synthesis with voice: {voice}");

        // A real one-word synthesis is the most reliable check: it verifies
        // the binary exists, Python deps are intact, and the network endpoint
        // is reachable. Cheaper than a voice-list API call and catches more
        // failure modes. The shared cooldown still applies — a probe during a
        // cooldown is a real request, and skipping the wait would earn the
        // provider another 429.
        self.request("test", voice).map(|_| ())
    }

    fn failure_is_final(&self) -> bool {
        self.refused.load(Ordering::Relaxed)
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
    fn cooldown_survives_retriggers_and_cannot_be_shortened() {
        let now = Instant::now();
        let mut state = EdgeCooldown {
            until: None,
            strikes: 0,
        };
        assert_eq!(state.rate_limited(now, None), Duration::from_secs(5));
        assert_eq!(
            state.remaining(now + Duration::from_secs(1)),
            Duration::from_secs(4)
        );
        state.reserve_start(now + Duration::from_secs(1));
        assert_eq!(
            state.remaining(now + Duration::from_secs(1)),
            Duration::from_secs(4)
        );
        assert_eq!(state.rate_limited(now, None), Duration::from_secs(10));
        assert_eq!(state.rate_limited(now, Some(120)), Duration::from_secs(120));
        state.rate_limited(now, Some(2));
        assert_eq!(state.remaining(now), Duration::from_secs(120));
        assert!(state.remaining(now + Duration::from_secs(120)).is_zero());
    }

    #[test]
    fn waiting_cancels_without_resetting_the_provider_cooldown() {
        let now = Instant::now();
        let mut state = EdgeCooldown {
            until: None,
            strikes: 0,
        };
        state.rate_limited(now, Some(20));
        let result = wait_for_slot(|| state.remaining(now), || true, MAX_WAIT);
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert_eq!(state.remaining(now), Duration::from_secs(20));
    }

    #[test]
    fn long_retry_after_fails_without_reconnecting_early() {
        let result = wait_for_slot(|| Duration::from_secs(120), || false, MAX_WAIT);
        let message = result.unwrap_err().to_string();
        assert!(message.contains("cooling down"));
        assert!(!message.contains("HTTP 429"));
    }

    #[test]
    fn cancellation_during_cooldown_prevents_request_start() {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let result = wait_for_slot(
            || {
                cancelled.store(true, Ordering::SeqCst);
                Duration::from_secs(1)
            },
            || cancelled.load(Ordering::SeqCst),
            MAX_WAIT,
        );
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[test]
    fn oversized_retry_after_is_capped_instead_of_disabling_the_engine() {
        let now = Instant::now();
        let mut state = EdgeCooldown {
            until: None,
            strikes: 0,
        };
        // A hostile or buggy header must not outlive the process.
        assert_eq!(state.rate_limited(now, Some(u64::MAX)), MAX_COOLDOWN);
        assert!(state.remaining(now) <= MAX_COOLDOWN);
        assert!(wait_for_slot(|| Duration::ZERO, || false, Duration::ZERO).is_ok());
    }

    #[test]
    fn success_resets_the_escalation_ladder() {
        let now = Instant::now();
        let mut state = EdgeCooldown {
            until: None,
            strikes: 0,
        };
        state.rate_limited(now, None);
        state.rate_limited(now, None);
        assert_eq!(state.rate_limited(now, None), Duration::from_secs(20));
        let accepted_at = now + Duration::from_secs(20);
        state.reserve_start(accepted_at);
        state.succeeded();
        assert_eq!(state.remaining(accepted_at), REQUEST_SPACING);
        assert!(state
            .remaining(accepted_at + Duration::from_secs(2))
            .is_zero());
        assert_eq!(
            state.rate_limited(accepted_at, None),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn fallback_cooldown_never_exceeds_sixty_seconds() {
        let mut now = Instant::now();
        let mut state = EdgeCooldown {
            until: None,
            strikes: 0,
        };
        for expected in [5, 10, 20, 40, 60, 60] {
            assert_eq!(state.rate_limited(now, None), Duration::from_secs(expected));
            now += Duration::from_secs(expected);
        }
    }

    #[test]
    fn provider_refusal_is_not_retried_by_the_command_layer() {
        let backend = EdgeTtsBackend::new(EdgeTtsConfig::default());
        assert!(
            !backend.failure_is_final(),
            "a fresh backend has no verdict"
        );
        backend.refused.store(true, Ordering::Relaxed);
        assert!(backend.failure_is_final());
        // The next request clears it: one refusal must not mute later readings.
        backend.refused.store(false, Ordering::Relaxed);
        assert!(!backend.failure_is_final());
    }

    #[test]
    fn structured_failure_does_not_forward_stderr_or_context() {
        let stderr = "untrusted diagnostic\n[edge-error] {\"kind\":\"http\",\"status\":429,\"retry_after_seconds\":17}\n";
        let error = parse_failure(stderr).unwrap();
        assert_eq!(error.retry_after_seconds, Some(17));
        assert!(safe_failure(Some(&error)).contains("HTTP 429"));
        assert!(!safe_failure(Some(&error)).contains("untrusted"));
        assert!(parse_failure("unstructured failure").is_none());
        assert!(!safe_failure(None).contains("unstructured"));
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
        assert_eq!(backend.voice_display_name("en-GB-SoniaNeural"), "sonia");
        assert_eq!(backend.voice_display_name("en-US-AriaNeural"), "aria");
    }
}
