// Clipboard facade: text read/write plus the double-copy trigger pipeline.
//
// Platform modules provide only the transport — Win32 window messages on
// Windows, wlr-data-control on Wayland. The double-copy state machine and the
// speak-request emission live here so both platforms behave identically and
// the trigger rules exist in exactly one place.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "windows")]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

/// Payload emitted to the frontend when TTS should trigger. The frontend
/// hands `trigger` back to `speak_queued` unchanged.
#[derive(Clone, serde::Serialize)]
pub struct SpeakRequest {
    pub text: String,
    /// Present only while a browser companion follows trigger speaks.
    pub trigger: Option<crate::browser_bridge::SpeakTrigger>,
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
    #[cfg(target_os = "macos")]
    {
        macos::read_clipboard_text()
    }
    #[cfg(target_os = "linux")]
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
    #[cfg(target_os = "macos")]
    {
        macos::set_clipboard_text(text)
    }
    #[cfg(target_os = "linux")]
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
    #[cfg(target_os = "macos")]
    macos::run_clipboard_listener(app, is_listening);
    #[cfg(target_os = "linux")]
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
                let truncated =
                    truncate_at_word_boundary(&sanitized_text, max_text_length as usize);
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

            // The copied text, before sanitize and truncation, is what a page
            // selection can match. Attach whenever the text could be a page
            // selection at all — whether a companion actually follows is
            // begin_trigger's call, and it waits bounded for the connection
            // the page's copy ping wakes. Gating HERE on a live companion
            // raced that wake: a sleeping worker silently dropped the follow.
            let trigger = (text.encode_utf16().count() <= crate::browser_bridge::MAX_TEXT_UNITS)
                .then(|| crate::browser_bridge::SpeakTrigger {
                    source: crate::browser_bridge::TriggerSource::DoubleCopy,
                    raw_text: text.to_string(),
                });

            // Emit the full text — the frontend will call speak_queued,
            // which handles pagination and sequential fragment playback.
            let _ = self.app.emit(
                "speak-request",
                SpeakRequest {
                    text: final_text,
                    trigger,
                },
            );
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

/// Truncate `text` to at most `max_chars` characters without severing a word.
///
/// Cuts at the last whitespace at or before the limit so a reading never starts
/// with half a word. A single token longer than `max_chars` (a URL, a base64
/// blob) has no boundary to snap to, so it is hard-truncated at the limit.
/// Always returns non-empty for non-empty input.
fn truncate_at_word_boundary(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    let window = &chars[..max_chars];
    // Cutting exactly at the cap is already clean when either side of the cut
    // is not a word character (e.g. the cap lands on a space, or the next
    // character starts a new word). Only back up when the cut would bisect a
    // word, so we never give up a valid complete word.
    let boundary_is_clean = match chars.get(max_chars) {
        Some(&next) => !window[max_chars - 1].is_alphanumeric() || !next.is_alphanumeric(),
        None => true,
    };
    let candidate: String = if boundary_is_clean {
        window.iter().collect()
    } else {
        match window.iter().rposition(|c| c.is_whitespace()) {
            // Keep the whitespace itself out of the result; trailing blanks
            // would otherwise be re-trimmed downstream anyway.
            Some(space) if space > 0 => window[..space].iter().collect(),
            // Whitespace only at index 0, or none at all: fall back to the hard
            // cap so we never return an empty string for real content.
            _ => window.iter().collect(),
        }
    };
    // A leading-space window can still be whitespace-only; keep real content.
    let trimmed = candidate.trim_end();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    // The window itself is all whitespace (e.g. " leading" with max_chars == 1).
    // Extend to the end of the next word so real content is never discarded.
    let mut end = max_chars;
    while end < chars.len() && chars[end].is_whitespace() {
        end += 1;
    }
    while end < chars.len() && !chars[end].is_whitespace() {
        end += 1;
    }
    chars[..end]
        .iter()
        .collect::<String>()
        .trim_start()
        .to_string()
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

    #[test]
    fn test_truncate_keeps_words_whole() {
        assert_eq!(
            truncate_at_word_boundary("alpha beta gamma delta", 12),
            "alpha beta"
        );
        // No boundary to snap to: hard cap so content is still produced.
        assert_eq!(truncate_at_word_boundary("abcdefghijkl", 5), "abcde");
        // At or under the limit is returned untouched.
        assert_eq!(truncate_at_word_boundary("short", 10), "short");
    }

    #[test]
    fn test_truncate_keeps_a_complete_word_ending_at_the_cap() {
        // The cap lands exactly at the end of a word followed by a space, so the
        // whole valid prefix is kept instead of backing up to an earlier space.
        assert_eq!(
            truncate_at_word_boundary("alpha beta gamma", 10),
            "alpha beta"
        );
        // Cap mid-word still backs up to the previous boundary.
        assert_eq!(truncate_at_word_boundary("alpha beta gamma", 9), "alpha");
    }

    #[test]
    fn test_truncate_never_returns_empty_for_real_content() {
        // Single long word, and a word whose only whitespace is at index 0.
        assert!(!truncate_at_word_boundary("supercalifragilistic", 4).is_empty());
        assert!(!truncate_at_word_boundary(" leading", 1).is_empty());
    }
}
