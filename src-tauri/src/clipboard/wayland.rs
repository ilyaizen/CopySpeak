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

/// Event-driven copy notification + fallback polling.
///
/// The wlr/ext data-control protocol has no "clipboard re-set with the same
/// content" event visible through wl-clipboard-rs, and a pure content poll
/// cannot distinguish a second copy of identical text from the first copy
/// still sitting there — so the double-copy trigger (copy X, copy X again)
/// never fired on Wayland. `wl-paste --watch` (wl-clipboard) reacts to every
/// data-control offer event, including identical re-sets, which is exactly
/// the Win32 WM_CLIPBOARDUPDATE semantics the state machine expects. We
/// spawn it and forward each event as a copy; the 1 Hz content poll below is
/// only a fallback for systems without wl-clipboard installed.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    log::info!("Clipboard listener thread starting (Wayland wlr-data-control)");

    if spawn_watch_listener(app.clone(), is_listening.clone()) {
        log::info!("[Clipboard] event listener active (wl-paste --watch)");
        return;
    }
    log::warn!("[Clipboard] wl-paste unavailable — falling back to 1 Hz content poll (same-text double-copy will not trigger)");
    run_poll_fallback(app, is_listening);
}

/// Spawn `wl-paste --watch echo` and forward every event through
/// handle_change. Each clipboard set (including identical re-sets) makes
/// wl-paste print one line to the shared stdout pipe — that line IS the copy
/// event. Returns false when wl-paste is not available.
fn spawn_watch_listener(app: AppHandle, is_listening: Arc<AtomicBool>) -> bool {
    let mut child = match std::process::Command::new("wl-paste")
        .arg("--watch")
        .arg("echo")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[Clipboard] failed to spawn wl-paste --watch: {e}");
            return false;
        }
    };
    log::info!("[Clipboard] wl-paste --watch spawned (pid {})", child.id());

    let mut stdout = child.stdout.take().expect("piped stdout");
    // Reap the child when it dies (compositor restart); it logs the status.
    std::thread::spawn(move || {
        let status = child.wait();
        log::warn!("[Clipboard] wl-paste --watch exited ({status:?})");
    });

    let mut ctx = TriggerContext {
        app,
        is_listening,
        state: super::ClipboardState::new(),
    };

    let mut buf = [0u8; 64];
    loop {
        match stdout.read(&mut buf) {
            Ok(0) => {
                // EOF: wl-paste died. Clipboard events stop until app restart.
                log::error!("[Clipboard] watch pipe closed; events stop until app restart");
                return true;
            }
            Ok(_) => {
                // One copy event (possibly coalesced) — read current content
                // and let the shared state machine decide.
                if let Some(text) = read_clipboard_text() {
                    ctx.handle_change(&text);
                }
            }
            Err(e) => {
                log::error!("[Clipboard] watch pipe read failed: {e}");
                return true;
            }
        }
    }
}

/// Fallback: 1 Hz content poll. Cannot see a re-copy of identical text.
fn run_poll_fallback(app: AppHandle, is_listening: Arc<AtomicBool>) {
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
