// Clipboard facade: text read/write plus the double-copy trigger pipeline.
//
// Platform modules provide only the transport — Win32 window messages on
// Windows, wlr-data-control on Wayland. The double-copy state machine and the
// speak-request emission live here so both platforms behave identically and
// the trigger rules exist in exactly one place.

#[cfg(not(target_os = "windows"))]
mod wayland;
#[cfg(target_os = "windows")]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

/// Payload emitted to the frontend when TTS should trigger.
#[derive(Clone, serde::Serialize)]
pub struct SpeakRequest {
    pub text: String,
}

/// Payload emitted when clipboard content changes.
#[derive(Clone, serde::Serialize)]
pub struct ClipboardChange {
    pub text: String,
}

/// Payload emitted when text is truncated due to max_text_length.
#[derive(Clone, serde::Serialize)]
pub struct TextTruncated {
    pub original_length: usize,
    pub truncated_length: usize,
    pub max_length: u64,
}

/// Read current clipboard text (public for use by commands).
pub fn get_clipboard_text() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        win32::read_clipboard_text()
    }
    #[cfg(not(target_os = "windows"))]
    {
        wayland::read_clipboard_text()
    }
}

/// Set clipboard text to the specified string.
/// Returns Ok(()) on success, or an error message on failure.
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win32::set_clipboard_text(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        wayland::set_clipboard_text(text)
    }
}

/// Start the platform clipboard listener on the calling thread (a dedicated
/// thread in production). Blocks for the process lifetime, like the Win32
/// message loop it mirrors.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    #[cfg(target_os = "windows")]
    win32::run_clipboard_listener(app, is_listening);
    #[cfg(not(target_os = "windows"))]
    wayland::run_clipboard_listener(app, is_listening);
}

/// Shared per-listener state: the app handle, the listening flag the settings
/// page flips, and the double-copy state machine. Platform listeners own one
/// instance and feed every clipboard change into [`TriggerContext::handle_change`].
pub(crate) struct TriggerContext {
    pub app: AppHandle,
    pub is_listening: Arc<AtomicBool>,
    pub state: ClipboardState,
}

impl TriggerContext {
    /// One clipboard change: notify the frontend, decide double-copy, and on a
    /// trigger sanitize, truncate, and emit `speak-request` exactly like a
    /// desktop double-copy always has.
    pub fn handle_change(&mut self, text: &str) {
        let text_len = text.len();
        let text_preview: String = text.chars().take(50).collect();
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");

        if crate::logging::is_debug_mode() {
            log::debug!("[Clipboard] Change detected at {}", timestamp);
            log::debug!("[Clipboard] Text length: {} chars", text_len);
            log::debug!(
                "[Clipboard] Text preview: {:?}{}",
                text_preview,
                if text_len > 50 { "..." } else { "" }
            );
        } else {
            log::debug!("[Clipboard] Change detected: {} chars", text_len);
        }

        let _ = self.app.emit(
            "clipboard-change",
            ClipboardChange {
                text: text.to_string(),
            },
        );

        if !self.is_listening.load(Ordering::Relaxed) {
            if crate::logging::is_debug_mode() {
                log::debug!("[Clipboard] Skipping TTS - listening disabled");
            } else {
                log::debug!("[Clipboard] Skipping - listening disabled");
            }
            return;
        }

        let (trigger_window_ms, max_text_length) = {
            let config_state = self
                .app
                .state::<std::sync::Mutex<crate::config::AppConfig>>();
            let config = config_state.lock().unwrap();
            (
                config.trigger.double_copy_window_ms,
                config.trigger.max_text_length,
            )
        };

        if self.state.on_change(text, trigger_window_ms) {
            log::info!("[Clipboard] Double-copy detected");

            let sanitized_text = {
                let config_state = self
                    .app
                    .state::<std::sync::Mutex<crate::config::AppConfig>>();
                let config = config_state.lock().unwrap();
                let sanitization_config = config.sanitization.clone();
                if sanitization_config.enabled {
                    crate::sanitize::sanitize_text(text, &sanitization_config)
                } else {
                    text.to_string()
                }
            };

            let final_char_count: usize = sanitized_text.chars().count();
            if crate::logging::is_debug_mode() {
                log::debug!(
                    "[Clipboard] Sanitized: {} → {} chars",
                    text_len,
                    final_char_count
                );
            }

            // Apply max_text_length truncation
            let final_text = if sanitized_text.chars().count() > max_text_length as usize {
                let truncated: String = sanitized_text
                    .chars()
                    .take(max_text_length as usize)
                    .collect();
                log::info!(
                    "[Clipboard] Text truncated from {} to {} chars (max: {})",
                    sanitized_text.chars().count(),
                    truncated.chars().count(),
                    max_text_length
                );
                let _ = self.app.emit(
                    "text-truncated",
                    TextTruncated {
                        original_length: sanitized_text.chars().count(),
                        truncated_length: truncated.chars().count(),
                        max_length: max_text_length,
                    },
                );
                truncated
            } else {
                sanitized_text
            };

            log::debug!(
                "[Clipboard] Emitting speak-request ({} chars)",
                final_text.chars().count()
            );

            // Emit the full text — the frontend will call speak_queued,
            // which handles pagination and sequential fragment playback.
            let _ = self
                .app
                .emit("speak-request", SpeakRequest { text: final_text });
        } else {
            if crate::logging::is_debug_mode() {
                log::debug!(
                    "[Clipboard] Single copy - waiting for second copy within {}ms",
                    trigger_window_ms
                );
            } else {
                log::debug!("[Clipboard] Single copy detected, waiting for second copy");
            }
            crate::hud::show_hud_clipboard_copied(&self.app, trigger_window_ms);
        }
    }
}

/// The double-copy state machine.
pub(crate) struct ClipboardState {
    last_text: Option<String>,
    last_copy_time: Option<Instant>,
}

impl ClipboardState {
    fn new() -> Self {
        Self {
            last_text: None,
            last_copy_time: None,
        }
    }

    /// Returns true if this clipboard change should trigger TTS.
    fn on_change(&mut self, new_text: &str, trigger_window_ms: u64) -> bool {
        let now = Instant::now();

        // 1. Debounce rapid OS events
        // Windows often fires multiple WM_CLIPBOARDUPDATE events within a few milliseconds
        // for a single Ctrl+C user action (e.g., when adding multiple formats).
        // Wayland polling collapses the same action into one observation.
        if let Some(time) = self.last_copy_time {
            let elapsed = now.duration_since(time).as_millis();
            if elapsed < 50 {
                // Ignore events that are too close; it's the same copy action.
                return false;
            }
        }

        // 2. Check for double copy
        let should_speak = match (&self.last_text, self.last_copy_time) {
            (Some(prev), Some(time))
                if prev == new_text
                    && now.duration_since(time).as_millis() < trigger_window_ms as u128 =>
            {
                true
            }
            _ => false,
        };

        // 3. Update state
        if should_speak {
            // "Consume" the double copy so that if the user mashes Ctrl+C a 3rd time,
            // it doesn't immediately re-trigger. Instead, it arms again.
            self.last_text = None;
            // Preserve time so debounce applies to the next mash.
            self.last_copy_time = Some(now);
        } else {
            // First copy of a sequence. Arm it.
            self.last_text = Some(new_text.to_string());
            self.last_copy_time = Some(now);
        }

        should_speak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double_copy_triggers() {
        let mut state = ClipboardState::new();
        // First copy — arms the state
        assert!(!state.on_change("hello", 800));
        std::thread::sleep(std::time::Duration::from_millis(60));
        // Same text within window — should trigger
        assert!(state.on_change("hello", 800));
    }

    #[test]
    fn test_different_text_resets() {
        let mut state = ClipboardState::new();
        assert!(!state.on_change("hello", 800));
        assert!(!state.on_change("world", 800));
    }

    #[test]
    fn test_second_double_copy_after_trigger_does_not_retrigger() {
        let mut state = ClipboardState::new();
        // Shift time artificially using sleep since we use actual time

        // 1st copy
        assert!(!state.on_change("hello", 800));
        std::thread::sleep(std::time::Duration::from_millis(60));

        // 2nd copy -> Triggers!
        assert!(state.on_change("hello", 800));
        std::thread::sleep(std::time::Duration::from_millis(60));

        // 3rd copy -> State was consumed, so this is treated as the start of a NEW double copy.
        assert!(!state.on_change("hello", 800));

        std::thread::sleep(std::time::Duration::from_millis(60));

        // 4th copy -> Triggers again!
        assert!(state.on_change("hello", 800));
    }
}
