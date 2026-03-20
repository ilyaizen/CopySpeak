// Audio player: AudioPlayerInner (thread-bound) and AudioPlayer (thread-safe handle).
// Handles playback with interrupt/queue modes via a dedicated audio thread.

use crate::config::RetriggerMode;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::{BufReader, Cursor};
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

use super::stream::WavStreamSource;

/// Commands sent to the audio thread
#[allow(dead_code)]
pub(super) enum AudioCommand {
    Play(Vec<u8>),
    PlayStreaming(Child),
    Stop,
    Pause,
    Resume,
    TogglePause,
    SetMode(RetriggerMode),
    SetVolume(u8),
    SeekRelative(i32),
}

/// Internal AudioPlayer that runs on a dedicated thread (not Send+Sync)
struct AudioPlayerInner {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    mode: RetriggerMode,
    volume: u8,
    playback_start: Option<std::time::Instant>,
    paused_duration: std::time::Duration,
    pause_start: Option<std::time::Instant>,
    current_position: std::time::Duration,
}

impl AudioPlayerInner {
    fn new() -> Self {
        Self {
            _stream: None,
            stream_handle: None,
            sink: None,
            mode: RetriggerMode::Interrupt,
            volume: 100,
            playback_start: None,
            paused_duration: std::time::Duration::ZERO,
            pause_start: None,
            current_position: std::time::Duration::ZERO,
        }
    }

    fn set_mode(&mut self, mode: RetriggerMode) {
        log::info!("Audio retrigger mode changed to: {:?}", mode);
        self.mode = mode;
    }

    fn set_volume(&mut self, volume: u8) {
        log::debug!("Volume changed to: {}%", volume);
        self.volume = volume;
        if let Some(ref sink) = self.sink {
            sink.set_volume(volume as f32 / 100.0);
        }
    }

    fn play(&mut self, audio_bytes: Vec<u8>) -> Result<(), String> {
        log::debug!(
            "play() called with {} bytes, mode: {:?}",
            audio_bytes.len(),
            self.mode
        );

        // Validate audio data before attempting playback
        if audio_bytes.is_empty() {
            log::error!("Cannot play empty audio data");
            return Err(
                "Cannot play empty audio data. The audio file may be corrupted.".to_string(),
            );
        }

        // Minimum size check for any audio format (MP3, WAV, etc. need at least some bytes)
        if audio_bytes.len() < 16 {
            log::error!(
                "Audio data too small ({} bytes), likely corrupted",
                audio_bytes.len()
            );
            return Err(format!(
                "Audio data too small ({} bytes). The audio file is corrupted or incomplete.",
                audio_bytes.len()
            ));
        }

        // Log audio format for debugging (check for common headers)
        let header = &audio_bytes[0..4];
        if header == b"RIFF" {
            log::debug!("Detected WAV format (RIFF header)");
        } else if header[0] == 0xFF && (header[1] & 0xE0) == 0xE0 {
            log::debug!("Detected MP3 format (MPEG sync word)");
        } else if header == b"OggS" {
            log::debug!("Detected OGG format");
        } else if header == b"fLaC" {
            log::debug!("Detected FLAC format");
        } else {
            log::debug!("Unknown audio format, letting decoder handle it");
        }

        match self.mode {
            RetriggerMode::Interrupt => {
                log::debug!("Interrupt mode: stopping current playback");
                self.stop();
            }
            RetriggerMode::Queue => {
                if let Some(ref sink) = self.sink {
                    if !sink.empty() {
                        log::debug!("Queue mode: appending to existing playback");
                        let cursor = Cursor::new(audio_bytes);
                        let source = Decoder::new(cursor).map_err(|e| {
                            log::error!("Failed to decode audio for queue: {}", e);
                            format!("Failed to decode audio: {}. The audio file may be corrupted or in an unsupported format.", e)
                        })?;
                        sink.set_volume(self.volume as f32 / 100.0);
                        sink.append(source);
                        return Ok(());
                    }
                }
            }
        }

        log::debug!("Creating new stream and sink for playback");
        let (stream, handle) = OutputStream::try_default().map_err(|e| {
            log::error!("Failed to open default audio output device: {}", e);
            format!("No audio output device found: {}", e)
        })?;

        let sink = Sink::try_new(&handle).map_err(|e| {
            log::error!("Failed to create audio sink: {}", e);
            format!(
                "Failed to initialize audio playback: {}. Check your audio device settings.",
                e
            )
        })?;
        let cursor = Cursor::new(audio_bytes);
        let source = Decoder::new(cursor).map_err(|e| {
            log::error!("Failed to decode audio: {}", e);
            format!("Failed to decode audio: {}. The audio file may be corrupted or in an unsupported format.", e)
        })?;

        sink.set_volume(self.volume as f32 / 100.0);
        sink.append(source);

        self.sink = Some(sink);
        self._stream = Some(stream);
        self.stream_handle = Some(handle);
        self.playback_start = Some(std::time::Instant::now());
        self.paused_duration = std::time::Duration::ZERO;
        self.pause_start = None;
        self.current_position = std::time::Duration::ZERO;
        log::info!("Audio playback started");
        Ok(())
    }

    fn play_streaming(&mut self, mut child: Child) -> Result<(), String> {
        log::debug!("play_streaming() called, mode: {:?}", self.mode);

        match self.mode {
            RetriggerMode::Interrupt => {
                log::debug!("Interrupt mode: stopping current playback");
                self.stop();
            }
            RetriggerMode::Queue => {
                if let Some(ref sink) = self.sink {
                    if !sink.empty() {
                        log::debug!("Queue mode: appending streaming audio to existing playback");
                        let stdout = child.stdout.take().ok_or_else(|| {
                            log::error!("TTS process has no stdout stream available");
                            "No audio stream from TTS process. The TTS engine may have failed."
                                .to_string()
                        })?;
                        let reader = BufReader::new(stdout);
                        let source = WavStreamSource::new(reader).map_err(|e| {
                            log::error!("Failed to decode streaming audio: {}", e);
                            format!("Failed to decode streaming audio: {}. The TTS output may be corrupted.", e)
                        })?;
                        sink.set_volume(self.volume as f32 / 100.0);
                        sink.append(source.buffered());
                        return Ok(());
                    }
                }
            }
        }

        log::debug!("Creating new stream and sink for streaming playback");
        let stdout = child.stdout.take().ok_or_else(|| {
            log::error!("TTS process has no stdout stream available");
            "No audio stream from TTS process. The TTS engine may have failed.".to_string()
        })?;
        let reader = BufReader::new(stdout);

        let source = WavStreamSource::new(reader).map_err(|e| {
            log::error!("Failed to decode streaming audio: {}", e);
            format!(
                "Failed to decode streaming audio: {}. The TTS output may be corrupted.",
                e
            )
        })?;

        let (stream, handle) = OutputStream::try_default().map_err(|e| {
            log::error!("Failed to find default audio output device: {}", e);
            format!("No audio output device found: {}", e)
        })?;

        let sink = Sink::try_new(&handle).map_err(|e| {
            log::error!("Failed to create audio sink: {}", e);
            format!(
                "Failed to initialize audio playback: {}. Check your audio device settings.",
                e
            )
        })?;

        sink.set_volume(self.volume as f32 / 100.0);
        sink.append(source.buffered());

        self.sink = Some(sink);
        self._stream = Some(stream);
        self.stream_handle = Some(handle);
        self.playback_start = Some(std::time::Instant::now());
        self.paused_duration = std::time::Duration::ZERO;
        self.pause_start = None;
        self.current_position = std::time::Duration::ZERO;
        log::info!("Streaming audio playback started");

        std::thread::spawn(move || {
            let _ = child.wait();
        });

        Ok(())
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            log::info!("Audio playback stopped");
            sink.stop();
        } else {
            log::debug!("stop() called but no active playback");
        }
        self._stream = None;
        self.stream_handle = None;
        self.playback_start = None;
        self.paused_duration = std::time::Duration::ZERO;
        self.pause_start = None;
        self.current_position = std::time::Duration::ZERO;
    }

    fn pause(&mut self) {
        if let Some(ref sink) = self.sink {
            log::info!("Audio playback paused");
            sink.pause();
            self.pause_start = Some(std::time::Instant::now());
        } else {
            log::debug!("pause() called but no active playback");
        }
    }

    fn resume(&mut self) {
        if let Some(ref sink) = self.sink {
            log::info!("Audio playback resumed");
            sink.play();
            if let Some(pause_start) = self.pause_start {
                self.paused_duration += pause_start.elapsed();
                self.pause_start = None;
            }
        } else {
            log::debug!("resume() called but no active playback");
        }
    }

    fn is_playing(&self) -> bool {
        self.sink.as_ref().map_or(false, |s| !s.empty())
    }

    fn is_paused(&self) -> bool {
        self.sink.as_ref().map_or(false, |s| s.is_paused())
    }

    fn toggle_pause(&mut self) {
        if let Some(ref sink) = self.sink {
            if sink.is_paused() {
                log::info!("Audio playback resumed (via toggle)");
                sink.play();
                if let Some(pause_start) = self.pause_start {
                    self.paused_duration += pause_start.elapsed();
                    self.pause_start = None;
                }
            } else {
                log::info!("Audio playback paused (via toggle)");
                sink.pause();
                self.pause_start = Some(std::time::Instant::now());
            }
        } else {
            log::debug!("toggle_pause() called but no active playback");
        }
    }

    fn get_current_position(&self) -> std::time::Duration {
        if let Some(start) = self.playback_start {
            let elapsed = start.elapsed();
            let total_paused = self.paused_duration
                + self
                    .pause_start
                    .map(|p| p.elapsed())
                    .unwrap_or(std::time::Duration::ZERO);
            self.current_position + elapsed.saturating_sub(total_paused)
        } else {
            std::time::Duration::ZERO
        }
    }

    fn seek_relative(&mut self, delta: std::time::Duration) {
        if let Some(ref sink) = self.sink {
            let current = self.get_current_position();
            let new_pos = if delta.as_nanos() > 0 {
                current.saturating_add(delta)
            } else {
                current.saturating_sub(std::time::Duration::from_nanos(delta.as_nanos() as u64))
            };
            log::debug!(
                "Seeking from {:?} to {:?} (delta: {:?})",
                current,
                new_pos,
                delta
            );
            if let Err(e) = sink.try_seek(new_pos) {
                log::warn!("Seek failed: {:?}", e);
            } else {
                log::info!("Seeked to position: {:?}", new_pos);
                self.current_position = new_pos;
                self.playback_start = Some(std::time::Instant::now());
                self.paused_duration = std::time::Duration::ZERO;
                self.pause_start = None;
            }
        } else {
            log::debug!("seek_relative() called but no active playback");
        }
    }
}

/// Playback state information for the frontend.
#[derive(Clone, serde::Serialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub is_paused: bool,
}

/// Thread-safe handle to the AudioPlayer.
/// Communicates with the audio thread via channels.
pub struct AudioPlayer {
    tx: Sender<AudioCommand>,
    is_playing: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    #[allow(dead_code)]
    playback_finished: Arc<AtomicBool>,
    currently_playing_entry_id: Arc<std::sync::Mutex<Option<String>>>,
}

// Explicitly implement Send and Sync since all fields are thread-safe
unsafe impl Send for AudioPlayer {}
unsafe impl Sync for AudioPlayer {}

impl AudioPlayer {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        let (tx, rx) = channel::<AudioCommand>();
        let is_playing = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let playback_finished = Arc::new(AtomicBool::new(false));
        let currently_playing_entry_id = Arc::new(std::sync::Mutex::new(None));
        let is_playing_clone = is_playing.clone();
        let is_paused_clone = is_paused.clone();
        let playback_finished_clone = playback_finished.clone();
        let currently_playing_entry_id_clone = currently_playing_entry_id.clone();

        thread::spawn(move || {
            log::info!("Audio thread started");
            let mut player = AudioPlayerInner::new();
            let mut prev_playing = false;

            loop {
                let now_playing = player.is_playing();
                is_playing_clone.store(now_playing, Ordering::Relaxed);
                is_paused_clone.store(player.is_paused(), Ordering::Relaxed);

                // Detect state changes for the tray icon
                if now_playing != prev_playing {
                    crate::update_tray_icon(&app_handle);
                }

                // Detect playback finish (true -> false transition)
                if prev_playing && !now_playing {
                    log::info!("Playback finished");
                    playback_finished_clone.store(true, Ordering::Relaxed);
                    // Clear currently playing entry ID when playback finishes
                    if let Ok(mut entry_id) = currently_playing_entry_id_clone.lock() {
                        *entry_id = None;
                    }
                }
                prev_playing = now_playing;

                match rx.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(cmd) => match cmd {
                        AudioCommand::Play(wav_bytes) => {
                            if let Err(e) = player.play(wav_bytes) {
                                log::error!("Audio play error: {}", e);
                            }
                        }
                        AudioCommand::PlayStreaming(child) => {
                            if let Err(e) = player.play_streaming(child) {
                                log::error!("Audio streaming play error: {}", e);
                            }
                        }
                        AudioCommand::Stop => {
                            player.stop();
                        }
                        AudioCommand::Pause => {
                            player.pause();
                        }
                        AudioCommand::Resume => {
                            player.resume();
                        }
                        AudioCommand::TogglePause => {
                            player.toggle_pause();
                        }
                        AudioCommand::SetMode(mode) => {
                            player.set_mode(mode);
                        }
                        AudioCommand::SetVolume(volume) => {
                            player.set_volume(volume);
                        }
                        AudioCommand::SeekRelative(seconds) => {
                            if seconds >= 0 {
                                player
                                    .seek_relative(std::time::Duration::from_secs(seconds as u64));
                            } else {
                                player.seek_relative(std::time::Duration::from_secs(
                                    (-seconds) as u64,
                                ));
                            }
                        }
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Self {
            tx,
            is_playing,
            is_paused,
            playback_finished,
            currently_playing_entry_id,
        }
    }

    pub fn set_mode(&mut self, mode: RetriggerMode) {
        let _ = self.tx.send(AudioCommand::SetMode(mode));
    }

    /// Set the playback volume (0-100).
    pub fn set_volume(&mut self, volume: u8) {
        let _ = self.tx.send(AudioCommand::SetVolume(volume));
    }

    /// Play WAV audio bytes. Behavior on re-trigger depends on current mode.
    #[allow(dead_code)]
    pub fn play(&mut self, wav_bytes: Vec<u8>) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Play(wav_bytes))
            .map_err(|e| format!("Failed to send play command: {e}"))?;
        Ok(())
    }

    pub fn stop(&mut self) {
        let _ = self.tx.send(AudioCommand::Stop);
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        let _ = self.tx.send(AudioCommand::Pause);
    }

    #[allow(dead_code)]
    pub fn resume(&mut self) {
        let _ = self.tx.send(AudioCommand::Resume);
    }

    pub fn toggle_pause(&mut self) {
        let _ = self.tx.send(AudioCommand::TogglePause);
    }

    pub fn skip_forward(&mut self, seconds: u64) {
        let _ = self.tx.send(AudioCommand::SeekRelative(seconds as i32));
    }

    pub fn skip_backward(&mut self, seconds: u64) {
        let _ = self.tx.send(AudioCommand::SeekRelative(-(seconds as i32)));
    }

    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    pub fn get_state(&self) -> PlaybackState {
        PlaybackState {
            is_playing: self.is_playing.load(Ordering::Relaxed),
            is_paused: self.is_paused.load(Ordering::Relaxed),
        }
    }

    /// Check and clear the playback finished flag.
    /// Returns true if playback finished since the last call, false otherwise.
    #[allow(dead_code)]
    pub fn take_playback_finished(&self) -> bool {
        self.playback_finished.swap(false, Ordering::Relaxed)
    }

    /// Set the currently playing history entry ID.
    pub fn set_playing_entry_id(&self, entry_id: Option<String>) {
        if let Ok(mut id) = self.currently_playing_entry_id.lock() {
            *id = entry_id;
        }
    }

    /// Get the currently playing history entry ID.
    pub fn get_playing_entry_id(&self) -> Option<String> {
        self.currently_playing_entry_id.lock().ok()?.clone()
    }
}
