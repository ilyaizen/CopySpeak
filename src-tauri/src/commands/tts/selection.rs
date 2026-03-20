// Speak-selected-text command (Ctrl+C simulation + speak).

use crate::audio::AudioPlayer;
use crate::config::AppConfig;
use crate::history::HistoryLog;
use crate::telemetry;
use std::sync::Mutex;
use tauri::{AppHandle, State};

use super::synthesis::speak_now;

// ── simulate_copy_sequence ──────────────────────────────────────────────────

fn simulate_copy_sequence() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C, VK_CONTROL,
    };
    use std::mem::size_of;

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
    Ok(())
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

    // Simulate Ctrl+C
    if let Err(e) = simulate_copy_sequence() {
        log::error!("Failed to simulate copy: {}", e);
        return Err(e);
    }

    // Wait for clipboard to update (naive approach for now)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Call speak_now with None to use clipboard content
    speak_now(app, config, player, history, telemetry_state, None).await
}
