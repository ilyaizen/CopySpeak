// macOS clipboard transport: NSPasteboard read/write plus a changeCount poll.
//
// changeCount increments on EVERY pasteboard write, including a re-set of
// identical content — exactly the Win32 WM_CLIPBOARDUPDATE semantics the
// double-copy state machine expects. (Wayland needed `wl-paste --watch` for
// this; the mac gets it from the pasteboard sequence number directly.)

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

use super::TriggerContext;

/// Poll cadence. The pasteboard has no cross-process notification callback we
/// can take without running an NSApplication loop; 250 ms is one integer
/// compare per tick and is imperceptible against the double-copy window.
const POLL_INTERVAL_MS: u64 = 250;

/// Read current clipboard text. `None` when the clipboard holds no string,
/// is empty, or the pasteboard is unavailable.
pub fn read_clipboard_text() -> Option<String> {
    let pb = NSPasteboard::generalPasteboard();
    let s = unsafe { pb.stringForType(NSPasteboardTypeString) }?;
    let text = s.to_string();
    // An empty clipboard must not read as "copy of the empty string".
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Set clipboard text. `clearContents` + `setString` is the standard write
/// pair; the value keeps working after CopySpeak exits (system-owned
/// pasteboard, same as Windows).
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let pb = NSPasteboard::generalPasteboard();
    let s = NSString::from_str(text);
    pb.clearContents();
    let ok = unsafe { pb.setString_forType(&s, NSPasteboardTypeString) };
    if ok {
        Ok(())
    } else {
        Err("Failed to set clipboard".to_string())
    }
}

/// Main clipboard listener loop. Call from a dedicated thread. Polls
/// changeCount and feeds every bump into the shared trigger pipeline — a bump
/// fires even for identical re-sets, so copy X / copy X again triggers.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    log::info!("Clipboard listener thread starting (macOS NSPasteboard changeCount poll)");

    let mut ctx = TriggerContext {
        app,
        is_listening,
        state: super::ClipboardState::new(),
    };

    let pb = NSPasteboard::generalPasteboard();
    let mut last_seen_count = unsafe { pb.changeCount() };

    loop {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

        let count = unsafe { pb.changeCount() };
        if count == last_seen_count {
            continue;
        }
        last_seen_count = count;

        if let Some(text) = read_clipboard_text() {
            ctx.handle_change(&text);
        }
    }
}
