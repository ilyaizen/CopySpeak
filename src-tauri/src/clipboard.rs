// Clipboard watcher using Win32 AddClipboardFormatListener.
// Runs on a dedicated thread (not async — Win32 message pump requires it).
// Detects double-copy (same text copied twice within N ms) and emits a Tauri event.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};


use windows::core::w;
use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, OpenClipboard,
    RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HWND_MESSAGE, MSG,
    WM_CLIPBOARDUPDATE, WM_DESTROY, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

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
    read_clipboard_text()
}

/// Set clipboard text to the specified string.
/// Returns Ok(()) on success, or an error message on failure.
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GMEM_MOVEABLE};

    unsafe {
        if OpenClipboard(None).is_err() {
            return Err("Failed to open clipboard".to_string());
        }

        let _guard = ClipboardGuard;

        if EmptyClipboard().is_err() {
            return Err("Failed to empty clipboard".to_string());
        }

        let text_utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = text_utf16.len() * std::mem::size_of::<u16>();

        let handle = GlobalAlloc(GMEM_MOVEABLE, byte_len)
            .map_err(|e| format!("Failed to allocate memory: {}", e))?;

        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return Err("Failed to lock memory".to_string());
        }

        std::ptr::copy_nonoverlapping(text_utf16.as_ptr(), ptr as *mut u16, text_utf16.len());

        let _ = GlobalUnlock(handle);

        SetClipboardData(CF_UNICODETEXT, HANDLE(handle.0))
            .map_err(|_| "Failed to set clipboard data".to_string())?;

        Ok(())
    }
}

/// The double-copy state machine.
struct ClipboardState {
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

/// CF_UNICODETEXT clipboard format constant
const CF_UNICODETEXT: u32 = 13;

/// Read current clipboard text using Win32 API.
/// Returns None if clipboard doesn't contain text or can't be opened.
fn read_clipboard_text() -> Option<String> {
    unsafe {
        // Open the clipboard (None means associate with current task)
        if OpenClipboard(None).is_err() {
            log::debug!("Failed to open clipboard");
            return None;
        }

        // Ensure we close the clipboard when done
        let _guard = ClipboardGuard;

        // Get clipboard data as CF_UNICODETEXT (wide string)
        let handle = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) => h,
            Err(_) => {
                log::debug!("No text data in clipboard");
                return None;
            }
        };

        if handle.0.is_null() {
            return None;
        }

        // Convert HANDLE to HGLOBAL for GlobalLock/GlobalUnlock
        let hglobal = HGLOBAL(handle.0);

        // Lock the global memory to get a pointer
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            log::debug!("Failed to lock clipboard memory");
            return None;
        }

        // Read the wide string (null-terminated UTF-16)
        let wstr = ptr as *const u16;
        let mut len = 0;
        while *wstr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(wstr, len);
        let text = String::from_utf16_lossy(slice);

        // Unlock the memory
        let _ = GlobalUnlock(hglobal);

        Some(text)
    }
}

/// RAII guard to ensure clipboard is closed
struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

// Thread-local storage for the clipboard listener context.
// Required because Win32 window procedures don't have a context pointer.
thread_local! {
    static LISTENER_CONTEXT: std::cell::RefCell<Option<ListenerContext>> = const { std::cell::RefCell::new(None) };
}

/// Context for the clipboard listener, stored in thread-local storage.
struct ListenerContext {
    app: AppHandle,
    is_listening: Arc<AtomicBool>,
    state: ClipboardState,
}

/// Window procedure for the clipboard listener window.
unsafe extern "system" fn clipboard_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            LISTENER_CONTEXT.with(|ctx| {
                if let Some(ctx) = ctx.borrow_mut().as_mut() {
                    if let Some(text) = read_clipboard_text() {
                        let text_len = text.len();
                        let text_preview: String = text.chars().take(50).collect();
                        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");

                        if crate::logging::is_debug_mode() {
                            log::debug!("[Clipboard] Change detected at {}", timestamp);
                            log::debug!("[Clipboard] Text length: {} chars", text_len);
                            log::debug!("[Clipboard] Text preview: {:?}{}", text_preview, if text_len > 50 { "..." } else { "" });
                        } else {
                            log::debug!("[Clipboard] Change detected: {} chars", text_len);
                        }

                        let _ = ctx.app.emit("clipboard-change", ClipboardChange { text: text.clone() });

                        if !ctx.is_listening.load(Ordering::Relaxed) {
                            if crate::logging::is_debug_mode() {
                                log::debug!("[Clipboard] Skipping TTS - listening disabled");
                            } else {
                                log::debug!("[Clipboard] Skipping - listening disabled");
                            }
                            return;
                        }

                        let (trigger_window_ms, max_text_length) = {
                            let config_state =
                                ctx.app.state::<std::sync::Mutex<crate::config::AppConfig>>();
                            let config = config_state.lock().unwrap();
                            (config.trigger.double_copy_window_ms, config.trigger.max_text_length)
                        };

                        if ctx.state.on_change(&text, trigger_window_ms) {
                            log::info!("[Clipboard] Double-copy detected");

                            let sanitized_text = {
                                let config_state =
                                    ctx.app.state::<std::sync::Mutex<crate::config::AppConfig>>();
                                let config = config_state.lock().unwrap();
                                let sanitization_config = config.sanitization.clone();
                                if sanitization_config.enabled {
                                    crate::sanitize::sanitize_text(&text, &sanitization_config)
                                } else {
                                    text.clone()
                                }
                            };

                            let final_char_count: usize = sanitized_text.chars().count();
                            if crate::logging::is_debug_mode() {
                                log::debug!("[Clipboard] Sanitized: {} → {} chars", text_len, final_char_count);
                            }

                            // Apply max_text_length truncation
                            let final_text = if sanitized_text.chars().count() > max_text_length as usize {
                                let truncated: String = sanitized_text.chars().take(max_text_length as usize).collect();
                                log::info!(
                                    "[Clipboard] Text truncated from {} to {} chars (max: {})",
                                    sanitized_text.chars().count(),
                                    truncated.chars().count(),
                                    max_text_length
                                );
                                let _ = ctx.app.emit("text-truncated", TextTruncated {
                                    original_length: sanitized_text.chars().count(),
                                    truncated_length: truncated.chars().count(),
                                    max_length: max_text_length,
                                });
                                truncated
                            } else {
                                sanitized_text
                            };

                            log::debug!("[Clipboard] Emitting speak-request ({} chars)", final_text.chars().count());

                            // Emit the full text — the frontend will call speak_queued,
                            // which handles pagination and sequential fragment playback.
                            let _ = ctx.app.emit("speak-request", SpeakRequest { text: final_text });
                        } else {
                            if crate::logging::is_debug_mode() {
                                log::debug!("[Clipboard] Single copy - waiting for second copy within {}ms", trigger_window_ms);
                            } else {
                                log::debug!("[Clipboard] Single copy detected, waiting for second copy");
                            }
                            crate::hud::show_hud_clipboard_copied(&ctx.app, trigger_window_ms);
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Main clipboard listener loop. Call from a dedicated thread.
/// Uses Win32 AddClipboardFormatListener to get notified of clipboard changes
/// via WM_CLIPBOARDUPDATE, then checks the double-copy state machine.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    log::info!("Clipboard listener thread starting with Win32 AddClipboardFormatListener");

    // Initialize the thread-local context
    LISTENER_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(ListenerContext {
            app: app.clone(),
            is_listening,
            state: ClipboardState::new(),
        });
    });

    unsafe {
        // Get module handle
        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h,
            Err(e) => {
                log::error!("Failed to get module handle: {:?}", e);
                return;
            }
        };

        // Register window class
        let class_name = w!("CopySpeakClipboardListener");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(clipboard_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            log::error!("Failed to register window class");
            return;
        }

        // Create a message-only window (invisible, doesn't appear in taskbar)
        let hwnd = match CreateWindowExW(
            Default::default(),
            class_name,
            w!("CopySpeak Clipboard Listener"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE, // Message-only window
            None,
            hinstance,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                log::error!("Failed to create window: {:?}", e);
                return;
            }
        };

        // Register for clipboard format listener notifications
        if let Err(e) = AddClipboardFormatListener(hwnd) {
            log::error!("Failed to add clipboard format listener: {:?}", e);
            let _ = DestroyWindow(hwnd);
            return;
        }

        log::info!("Win32 clipboard listener registered successfully");

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = DestroyWindow(hwnd);
        log::info!("Clipboard listener thread exiting");
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
