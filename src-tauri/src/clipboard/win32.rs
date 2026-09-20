// Clipboard watcher using Win32 AddClipboardFormatListener.
// Runs on a dedicated thread (not async — Win32 message pump requires it).
// Detects clipboard changes; the shared trigger pipeline in the parent module
// decides double-copy and emits the Tauri events.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

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

use super::TriggerContext;

/// CF_UNICODETEXT clipboard format constant
const CF_UNICODETEXT: u32 = 13;

/// Read current clipboard text using Win32 API.
/// Returns None if clipboard doesn't contain text or can't be opened.
pub(crate) fn read_clipboard_text() -> Option<String> {
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

/// Set clipboard text to the specified string.
/// Returns Ok(()) on success, or an error message on failure.
pub(crate) fn set_clipboard_text(text: &str) -> Result<(), String> {
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
    trigger: TriggerContext,
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
                        ctx.trigger.handle_change(&text);
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
/// via WM_CLIPBOARDUPDATE, then feeds them to the shared trigger pipeline.
pub fn run_clipboard_listener(app: AppHandle, is_listening: Arc<AtomicBool>) {
    log::info!("Clipboard listener thread starting with Win32 AddClipboardFormatListener");

    // Initialize the thread-local context
    LISTENER_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(ListenerContext {
            trigger: TriggerContext {
                app: app.clone(),
                is_listening,
                state: super::ClipboardState::new(),
            },
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
