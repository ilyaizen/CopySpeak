// Audio player: AudioPlayerInner (thread-bound) and AudioPlayer (thread-safe handle).
// Handles playback with interrupt/queue modes via a dedicated audio thread.

use crate::config::RetriggerMode;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

/// Commands sent to the audio thread
pub(super) enum AudioCommand {
    Stop,
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

                match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(cmd) => match cmd {
                        AudioCommand::Stop => {
                            player.stop();
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

    pub fn stop(&mut self) {
        let _ = self.tx.send(AudioCommand::Stop);
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

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn get_state(&self) -> PlaybackState {
        PlaybackState {
            is_playing: self.is_playing.load(Ordering::Relaxed),
            is_paused: self.is_paused.load(Ordering::Relaxed),
        }
    }

    /// Check and clear the playback finished flag.
    /// Returns true if playback finished since the last call, false otherwise.
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
