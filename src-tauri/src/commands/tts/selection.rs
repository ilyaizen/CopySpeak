// Speak-selected-text command.
//
// Windows: simulate Ctrl+C, then read the clipboard.
// Linux (Wayland): read the primary selection directly — data-control lets us
// read what the user has selected without synthesizing input events, which
// Wayland does not allow globally in the first place.

use crate::audio::AudioPlayer;
use crate::config::AppConfig;
use crate::history::HistoryLog;
use crate::telemetry;
use std::sync::Mutex;
use tauri::{AppHandle, State};

use super::synthesis::speak_now;

// ── get_selected_text ───────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_selected_text() -> Result<String, String> {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C, VK_CONTROL,
    };

    // Simulate Ctrl+C
    unsafe {
        let inputs = [
            // Ctrl Down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        dwFlags: Default::default(),
                        ..Default::default()
                    },
                },
            },
            // C Down
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_C,
                        dwFlags: Default::default(),
                        ..Default::default()
                    },
                },
            },
            // C Up
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_C,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
            // Ctrl Up
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
        ];

        let result = SendInput(&inputs, size_of::<INPUT>() as i32);
        if result != inputs.len() as u32 {
            return Err("Failed to send Ctrl+C input".into());
        }
    }

    // The simulated copy is asynchronous: give the clipboard watcher a beat to
    // observe it, then read the text back (the previous inline flow slept
    // 200 ms before speak_now read the clipboard).
    std::thread::sleep(std::time::Duration::from_millis(200));
    crate::clipboard::get_clipboard_text().ok_or_else(|| "No text in clipboard".to_string())
}

#[cfg(target_os = "macos")]
fn get_selected_text() -> Result<String, String> {
    use objc2_core_graphics::{
        CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGKeyCode,
    };

    // Simulate Cmd+C. Requires Accessibility permission (TCC) — the same
    // grant the whole selection feature needs; without it the events post
    // into the void and the clipboard read below fails with "No text".
    const V_KEY_C: CGKeyCode = 8; // kVK_ANSI_C
    const V_KEY_CMD: CGKeyCode = 55; // kVK_Command

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .ok_or("Failed to create CGEventSource")?;

    // Cmd+Down, C Down/Up with the flag held, Cmd Up.
    let flags = CGEventFlags::MaskCommand;
    let tap = CGEventTapLocation::HIDEventTap;

    let cmd_down = CGEvent::new_keyboard_event(Some(&source), V_KEY_CMD, true)
        .ok_or("Failed to create Cmd-down event")?;
    CGEvent::set_flags(Some(&cmd_down), flags);
    CGEvent::post(tap, Some(&cmd_down));

    let c_down = CGEvent::new_keyboard_event(Some(&source), V_KEY_C, true)
        .ok_or("Failed to create C-down event")?;
    CGEvent::set_flags(Some(&c_down), flags);
    CGEvent::post(tap, Some(&c_down));

    let c_up = CGEvent::new_keyboard_event(Some(&source), V_KEY_C, false)
        .ok_or("Failed to create C-up event")?;
    CGEvent::set_flags(Some(&c_up), flags);
    CGEvent::post(tap, Some(&c_up));

    let cmd_up = CGEvent::new_keyboard_event(Some(&source), V_KEY_CMD, false)
        .ok_or("Failed to create Cmd-up event")?;
    CGEvent::set_flags(Some(&cmd_up), flags);
    CGEvent::post(tap, Some(&cmd_up));

    // The simulated copy is asynchronous: give the clipboard watcher a beat to
    // observe it, then read the text back.
    std::thread::sleep(std::time::Duration::from_millis(200));
    crate::clipboard::get_clipboard_text().ok_or_else(|| "No text in clipboard".to_string())
}

#[cfg(target_os = "linux")]
fn get_selected_text() -> Result<String, String> {
    use std::io::Read as _;
    use wl_clipboard_rs::paste::{
        get_contents, ClipboardType, Error as PasteError, MimeType, Seat,
    };

    let result = get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text);
    match result {
        Ok((mut reader, _mime)) => {
            let mut text = String::new();
            reader
                .read_to_string(&mut text)
                .map_err(|e| format!("Failed to read primary selection: {e}"))?;
            Ok(text)
        }
        Err(PasteError::ClipboardEmpty | PasteError::NoMimeType) => Err("No text selected".into()),
        Err(e) => Err(format!("Failed to read primary selection: {e}")),
    }
}

// ── speak_selected_text ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn speak_selected_text(
    app: AppHandle,
    config: State<'_, Mutex<AppConfig>>,
    player: State<'_, Mutex<AudioPlayer>>,
    history: State<'_, Mutex<HistoryLog>>,
    telemetry_state: State<'_, Mutex<telemetry::TelemetryLog>>,
) -> Result<(), String> {
    log::info!("[Command] speak_selected_text triggered");

    // Obtain the selected text. Blocking OS calls (SendInput + clipboard read,
    // or a Wayland data-control round-trip) stay off the async runtime.
    let selected = tokio::task::spawn_blocking(get_selected_text)
        .await
        .map_err(|e| format!("selection task failed: {e}"))??;

    if selected.trim().is_empty() {
        return Err("No text selected".into());
    }

    // Speak the obtained text through the same path as the Play page.
    speak_now(
        app,
        config,
        player,
        history,
        telemetry_state,
        Some(selected),
    )
    .await
}
