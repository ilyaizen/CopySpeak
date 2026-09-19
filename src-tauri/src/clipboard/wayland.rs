// Wayland clipboard transport (wlr-data-control protocol).
//
// Compositors that speak wlr-data-control — Hyprland, Sway, and the rest of
// the wlroots family — let a client read and write the clipboard and the
// primary selection without focus. CopySpeak targets exactly that family.
//
// There is no clipboard-changed event in the protocol, so the double-copy
// trigger polls: one `get_contents` round-trip per second. Polling reads the
// same state the Win32 listener reacts to; the shared state machine dedupes
// and decides, so a copy is never missed and never double-counted.

use std::io::Read as _;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use wl_clipboard_rs::copy::{MimeType, Options as CopyOptions, Source};
use wl_clipboard_rs::paste::{
    get_contents, ClipboardType, Error as PasteError, MimeType as PasteMimeType, Seat,
};

use super::TriggerContext;

/// Poll interval for clipboard changes. Sub-second keeps the double-copy
/// window (config default 800 ms) usable; 1 Hz is imperceptible for a
/// copy-triggered workflow and cheap (one Wayland round-trip).
const POLL_INTERVAL_MS: u64 = 1000;

/// Read current clipboard text. `None` when the clipboard holds no text
/// offer, is empty, or the compositor/data-control protocol is unavailable.
pub fn read_clipboard_text() -> Option<String> {
    let result = get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        PasteMimeType::Text,
    );
    match result {
        Ok((mut reader, _mime)) => {
            let mut text = String::new();
            reader.read_to_string(&mut text).ok()?;
            // An empty clipboard offer must not read as "text is empty", which
            // would look like a copy of the empty string.
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Err(PasteError::ClipboardEmpty | PasteError::NoMimeType) => None,
        Err(e) => {
            log::debug!("[Clipboard] Wayland read failed: {e}");
            None
        }
    }
}

/// Set clipboard text. With default `Options` the crate daemonizes a helper
/// that serves paste requests indefinitely, so the copy keeps working after
/// CopySpeak exits — the same behavior as the Windows clipboard.
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let opts = CopyOptions::new();
    let source = Source::Bytes(text.as_bytes().into());
    opts.copy(source, MimeType::Text)
        .map_err(|e| format!("Failed to set clipboard: {e}"))
}

/// Poll the clipboard and feed changes into the shared trigger pipeline.
/// Runs on a dedicated thread; blocks for the process lifetime, mirroring the
/// Win32 message loop this replaces.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    log::info!("Clipboard listener thread starting (Wayland wlr-data-control poll)");

    let mut ctx = TriggerContext {
        app,
        is_listening,
        state: super::ClipboardState::new(),
    };
    let mut last_seen: Option<String> = None;

    loop {
        // Sleep first: the first iteration does not need an immediate read.
        std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));

        let Some(text) = read_clipboard_text() else {
            // No text (image/file clipboard, or empty). Forget the last text so
            // re-copying the same text after a non-text interlude still counts
            // as a fresh first copy — the state machine itself handles the rest.
            last_seen = None;
            continue;
        };

        // Only forward actual changes; identical consecutive reads are one
        // copy observed repeatedly. The trigger's own debounce and
        // double-copy logic stay authoritative.
        if last_seen.as_deref() == Some(text.as_str()) {
            continue;
        }
        last_seen = Some(text.clone());
        ctx.handle_change(&text);
    }
}
