// CopySpeak - Clipboard-to-speech orchestrator
// Main entry: wires up tray, clipboard watcher, and IPC commands.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod autostart;
mod clipboard;
mod commands;
mod config;
mod fragment_queue;
mod history;
mod history_manager;
mod hud;
mod logging;
mod pagination;
mod sanitize;
mod telemetry;
mod tts;

pub struct JobStatus {
    pub is_synthesizing: AtomicBool,
}

/// PID of the active CLI TTS process (0 if none).
pub static ACTIVE_CLI_PID: AtomicU32 = AtomicU32::new(0);

/// Flag set when synthesis abort is requested.
pub static ABORT_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn update_tray_icon(app_handle: &tauri::AppHandle) {
    if let Some(tray) = app_handle.tray_by_id("main") {
        let is_playing = {
            let player = app_handle.state::<std::sync::Mutex<audio::AudioPlayer>>();
            player.lock().map(|p| p.is_playing()).unwrap_or(false)
        };

        let is_synthesizing = {
            let status = app_handle.state::<JobStatus>();
            status.is_synthesizing.load(Ordering::Relaxed)
        };

        let is_busy = is_playing || is_synthesizing;

        // Using include_bytes for simplicity, you might want to load this from a path
        // if this was dynamic, but since it's static we can embed it.
        // Assuming the icons are pngs, Tauri `Image` handles it from bytes easily
        let result = if is_busy {
            tauri::image::Image::from_bytes(include_bytes!("../icons/app-icon-inverted.png"))
        } else {
            tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
        };

        if let Ok(img) = result {
            let _ = tray.set_icon(Some(img));
        }
    }
}

/// Kill a process tree by PID (used for aborting CLI TTS synthesis).
#[cfg(windows)]
pub fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    log::info!("[Abort] Killing process tree for PID {}", pid);
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

#[cfg(not(windows))]
pub fn kill_process_tree(pid: u32) {
    log::info!("[Abort] Killing process {}", pid);
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
}

/// Parse a hotkey string like "Ctrl+Shift+A" into a Shortcut object.
fn parse_hotkey(hotkey: &str) -> Result<Shortcut, String> {
    let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();

    if parts.is_empty() {
        return Err("Empty hotkey string".into());
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "super" | "meta" | "win" | "cmd" => modifiers |= Modifiers::SUPER,
            key => {
                if key.len() == 1 {
                    let ch = key.chars().next().unwrap().to_ascii_uppercase();
                    key_code = Some(match ch {
                        'A' => Code::KeyA,
                        'B' => Code::KeyB,
                        'C' => Code::KeyC,
                        'D' => Code::KeyD,
                        'E' => Code::KeyE,
                        'F' => Code::KeyF,
                        'G' => Code::KeyG,
                        'H' => Code::KeyH,
                        'I' => Code::KeyI,
                        'J' => Code::KeyJ,
                        'K' => Code::KeyK,
                        'L' => Code::KeyL,
                        'M' => Code::KeyM,
                        'N' => Code::KeyN,
                        'O' => Code::KeyO,
                        'P' => Code::KeyP,
                        'Q' => Code::KeyQ,
                        'R' => Code::KeyR,
                        'S' => Code::KeyS,
                        'T' => Code::KeyT,
                        'U' => Code::KeyU,
                        'V' => Code::KeyV,
                        'W' => Code::KeyW,
                        'X' => Code::KeyX,
                        'Y' => Code::KeyY,
                        'Z' => Code::KeyZ,
                        '0' => Code::Digit0,
                        '1' => Code::Digit1,
                        '2' => Code::Digit2,
                        '3' => Code::Digit3,
                        '4' => Code::Digit4,
                        '5' => Code::Digit5,
                        '6' => Code::Digit6,
                        '7' => Code::Digit7,
                        '8' => Code::Digit8,
                        '9' => Code::Digit9,
                        _ => return Err(format!("Unsupported key: {}", key)),
                    });
                } else {
                    key_code = Some(match key {
                        "space" => Code::Space,
                        "enter" | "return" => Code::Enter,
                        "tab" => Code::Tab,
                        "escape" | "esc" => Code::Escape,
                        "backspace" => Code::Backspace,
                        "delete" | "del" => Code::Delete,
                        "up" | "arrowup" => Code::ArrowUp,
                        "down" | "arrowdown" => Code::ArrowDown,
                        "left" | "arrowleft" => Code::ArrowLeft,
                        "right" | "arrowright" => Code::ArrowRight,
                        "f1" => Code::F1,
                        "f2" => Code::F2,
                        "f3" => Code::F3,
                        "f4" => Code::F4,
                        "f5" => Code::F5,
                        "f6" => Code::F6,
                        "f7" => Code::F7,
                        "f8" => Code::F8,
                        "f9" => Code::F9,
                        "f10" => Code::F10,
                        "f11" => Code::F11,
                        "f12" => Code::F12,
                        _ => return Err(format!("Unsupported key: {}", key)),
                    });
                }
            }
        }
    }

    key_code
        .map(|code| Shortcut::new(if modifiers.is_empty() { None } else { Some(modifiers) }, code))
        .ok_or_else(|| "No key code found in hotkey string".into())
}

/// Register the global hotkey for speak-from-clipboard.
pub fn register_hotkey(app: &tauri::AppHandle, hotkey_config: &config::HotkeyConfig) -> Result<(), String> {
    log::info!("[Hotkey] Attempting to register - enabled: {}, shortcut: {}", hotkey_config.enabled, hotkey_config.shortcut);
    
    app.global_shortcut().unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;

    if !hotkey_config.enabled {
        log::info!("[Hotkey] Disabled, skipping registration");
        return Ok(());
    }

    let shortcut = parse_hotkey(&hotkey_config.shortcut)?;
    app.global_shortcut().register(shortcut)
        .map_err(|e| format!("Failed to register shortcut '{}': {}", hotkey_config.shortcut, e))?;

    log::info!("[Hotkey] Successfully registered: {}", hotkey_config.shortcut);
    Ok(())
}

/// Abort any in-progress synthesis and stop playback.
/// Called from the abort_synthesis command and tray icon click handler.
pub fn do_abort_synthesis(app: &tauri::AppHandle) {
    log::info!("[Abort] Aborting synthesis");

    // Kill CLI process if running
    let pid = ACTIVE_CLI_PID.load(Ordering::Relaxed);
    if pid != 0 {
        kill_process_tree(pid);
        ACTIVE_CLI_PID.store(0, Ordering::Relaxed);
    }

    // Signal abort to synthesis tasks
    ABORT_REQUESTED.store(true, Ordering::Relaxed);

    // Reset synthesis state
    {
        let status = app.state::<JobStatus>();
        status.is_synthesizing.store(false, Ordering::Relaxed);
    }
    update_tray_icon(app);
    let _ = app.emit("synthesis-state-change", false);

    // Emit abort event for frontend feedback
    let _ = app.emit("synthesis-aborted", ());

    // Stop playback
    let _ = app.emit("playback-stop", ());
    {
        let player = app.state::<std::sync::Mutex<audio::AudioPlayer>>();
        if let Ok(mut p) = player.lock() {
            p.stop();
            p.set_playing_entry_id(None);
        };
    }
}

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn main() {
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tauri::Builder::default()
        .setup(|app| {
            // --- Load config ---
            let cfg = config::load_or_default();
            app.manage(std::sync::Mutex::new(cfg));

            // --- Init audio player with saved config ---
            let app_handle = app.handle().clone();
            let mut player = audio::AudioPlayer::new(app_handle);
            // Apply saved mode from config
            {
                let cfg = app.state::<std::sync::Mutex<config::AppConfig>>();
                let cfg = cfg.lock().unwrap();
                player.set_mode(cfg.playback.on_retrigger.clone());
            }
            app.manage(std::sync::Mutex::new(player));

            // --- Init speech history log ---
            let history = history::load();
            app.manage(std::sync::Mutex::new(history));

            // --- Init history manager (centralized history operations) ---
            let history_manager = history_manager::HistoryManager::new();
            app.manage(std::sync::Mutex::new(history_manager));

            // --- Init cached audio state (for replay) ---
            app.manage(std::sync::Mutex::new(commands::CachedAudio::default()));

            // --- Init fragment queue (for sequential TTS playback) ---
            app.manage(std::sync::Mutex::new(fragment_queue::FragmentQueue::new()));

            // --- Init telemetry (synthesis timing data for ETA) ---
            let telemetry = telemetry::load();
            app.manage(std::sync::Mutex::new(telemetry));

            // --- Init listening state (shared with clipboard thread) ---
            let is_listening = Arc::new(AtomicBool::new(true));
            app.manage(is_listening.clone());

            // --- Init Job status ---
            app.manage(JobStatus {
                is_synthesizing: AtomicBool::new(false),
            });

            // --- Init global synthesis lock ---
            app.manage(tokio::sync::Mutex::new(()));

            // --- Build system tray ---
            let version = app.package_info().version.to_string();
            let version_item = MenuItem::with_id(app, "version", format!("CopySpeak v{version}"), false, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let toggle_item = MenuItem::with_id(app, "toggle", "● Listening", true, None::<&str>)?;
            let speak_item = MenuItem::with_id(app, "speak", "Speak Clipboard Now", true, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, Some("Ctrl+,"))?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, Some("Ctrl+Q"))?;

            let menu = Menu::with_items(app, &[
                &version_item,
                &sep1,
                &toggle_item,
                &speak_item,
                &sep2,
                &settings_item,
                &sep3,
                &quit_item,
            ])?;

            let is_listening_for_tray = is_listening.clone();
            let toggle_item_for_event = toggle_item.clone();
            let _tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("CopySpeak")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "toggle" => {
                            // Toggle listening state
                            let new_state = !is_listening_for_tray.load(Ordering::Relaxed);
                            is_listening_for_tray.store(new_state, Ordering::Relaxed);
                            log::info!("Tray: toggle listening -> {}", new_state);

                            // Update menu item label
                            let label = if new_state {
                                "● Listening"
                            } else {
                                "○ Paused"
                            };
                            let _ = toggle_item_for_event.set_text(label);
                        }
                        "speak" => {
                            log::info!("Tray: speak clipboard");
                            // Clone app handle for async context
                            let app_handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                // Get required states inside the async block
                                let config: State<std::sync::Mutex<config::AppConfig>> = app_handle.state();
                                let player: State<std::sync::Mutex<audio::AudioPlayer>> = app_handle.state();
                                let history: State<std::sync::Mutex<history::HistoryLog>> = app_handle.state();
                                let telemetry_state: State<std::sync::Mutex<telemetry::TelemetryLog>> = app_handle.state();
                                if let Err(e) = commands::speak_now(app_handle.clone(), config, player, history, telemetry_state, None).await {
                                    log::error!("Failed to speak from tray: {}", e);
                                }
                            });
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();

                        let is_synthesizing = {
                            let status = app.state::<JobStatus>();
                            status.is_synthesizing.load(Ordering::Relaxed)
                        };

                        let is_playing = {
                            let player = app.state::<std::sync::Mutex<audio::AudioPlayer>>();
                            player.lock().map(|p| p.is_playing()).unwrap_or(false)
                        };

                        if is_synthesizing {
                            // Abort synthesis when tray icon clicked during synthesis
                            do_abort_synthesis(app);
                        } else if is_playing {
                            // Toggle pause when tray icon clicked during playback
                            let _ = app.emit("playback-toggle-pause", ());
                            {
                                let player_state = app.state::<std::sync::Mutex<audio::AudioPlayer>>();
                                if let Ok(mut p) = player_state.lock() {
                                    p.toggle_pause();
                                };
                            }
                        } else {
                            // Show window when idle
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // --- Intercept main window close to minimize to tray ---
            if let Some(main_window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let close_behavior = {
                            let cfg: State<std::sync::Mutex<config::AppConfig>> = app_handle.state();
                            let cfg = cfg.lock().unwrap();
                            cfg.general.close_behavior.clone()
                        };

                        match close_behavior {
                            config::CloseBehavior::MinimizeToTray => {
                                api.prevent_close();
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    let _ = window.hide();
                                }
                                log::info!("Main window minimized to tray");
                            }
                            config::CloseBehavior::Exit => {
                                // Let the window close, app will exit
                            }
                        }
                    }
                });
            }

            // --- Position HUD window and make it click-through at startup ---
            if let Some(hud_window) = app.get_webview_window("hud") {
                let cfg_state = app.state::<std::sync::Mutex<config::AppConfig>>();
                let cfg = cfg_state.lock().unwrap();
                hud::position_hud_window(&hud_window, &cfg.hud);
                drop(cfg);
                let _ = hud_window.set_ignore_cursor_events(true);
            }

            // --- Start clipboard watcher (background thread) ---
            let app_handle = app.handle().clone();
            let is_listening_clone = is_listening.clone();
            std::thread::spawn(move || {
                clipboard::run_clipboard_listener(app_handle, is_listening_clone);
            });

            // --- Register global hotkey ---
            let hotkey_config = {
                let cfg = app.state::<std::sync::Mutex<config::AppConfig>>();
                let cfg = cfg.lock().unwrap();
                cfg.hotkey.clone()
            };
            
            if let Err(e) = register_hotkey(app.handle(), &hotkey_config) {
                log::warn!("Failed to register initial hotkey: {}", e);
            }

            // --- Start playback monitor thread (auto-hide HUD when audio finishes) ---
            let app_handle_for_monitor = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let player: State<std::sync::Mutex<audio::AudioPlayer>> = app_handle_for_monitor.state();
                    let finished = {
                        let p = player.lock().unwrap();
                        p.take_playback_finished()
                    };
                    if finished {
                        hud::hide_hud(&app_handle_for_monitor);
                    }
                }
            });

            // --- Start history cleanup service ---
            let app_handle_for_cleanup = app.handle().clone();
            history::start_cleanup_service(app_handle_for_cleanup);

            log::info!("CopySpeak started");
            Ok(())
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    log::info!("Global hotkey triggered: speak from clipboard");
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let config: State<std::sync::Mutex<config::AppConfig>> = app_handle.state();
                        let player: State<std::sync::Mutex<audio::AudioPlayer>> = app_handle.state();
                        let history: State<std::sync::Mutex<history::HistoryLog>> = app_handle.state();
                        let telemetry_state: State<std::sync::Mutex<telemetry::TelemetryLog>> = app_handle.state();
                        if let Err(e) = commands::speak_now(app_handle.clone(), config, player, history, telemetry_state, None).await {
                            log::error!("Failed to speak from hotkey: {}", e);
                        }
                    });
                }
            })
            .build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        log::info!("Global hotkey triggered: speak from clipboard");
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let config: State<std::sync::Mutex<config::AppConfig>> = app_handle.state();
                            let player: State<std::sync::Mutex<audio::AudioPlayer>> = app_handle.state();
                            let history: State<std::sync::Mutex<history::HistoryLog>> = app_handle.state();
                            let telemetry_state: State<std::sync::Mutex<telemetry::TelemetryLog>> = app_handle.state();
                            if let Err(e) = commands::speak_now(app_handle.clone(), config, player, history, telemetry_state, None).await {
                                log::error!("Failed to speak from hotkey: {}", e);
                            }
                        });
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::reset_config,
            commands::config_exists,
            commands::get_data_dir,
            commands::get_home_dir,
            commands::speak_now,
            commands::replay_cached,
            commands::speak_selected_text,
            commands::speak_queued,
            commands::abort_synthesis,
            commands::stop_speaking,
            commands::toggle_pause,
            commands::skip_forward,
            commands::skip_backward,
            commands::set_playback_speed,
            commands::get_playback_state,
            commands::set_listening,
            commands::get_listening,
            commands::get_clipboard_content,
            commands::set_volume,
            commands::set_debug_mode,
            commands::get_logs,
            commands::get_logs_path,
            commands::get_history,
            commands::clear_history,
            commands::test_tts_engine,
            commands::check_command_exists,
            commands::check_elevenlabs_credentials,
            commands::check_openai_credentials,
            commands::list_elevenlabs_voices,
            commands::get_elevenlabs_voice_by_id,
            commands::get_elevenlabs_output_formats,
            commands::get_queue_state,
            commands::get_queue_fragments,
            commands::skip_to_fragment,
            commands::stop_queue,
            commands::clear_queue,
            commands::play_history_entry,
            commands::show_hud_for_playback,
            commands::test_show_hud,
            commands::speak_history_entry,
            commands::copy_history_entry_text,
            commands::delete_history_entry,
            commands::list_history,
            commands::search_history,
            commands::export_history,
            commands::get_history_with_metadata,
            commands::get_history_statistics,
            commands::get_file_tracking,
             commands::get_history_unique_engines,
             commands::get_history_unique_voices,
             commands::get_history_unique_tags,
            commands::get_history_date_range,
            commands::run_history_cleanup,
            commands::get_entry_by_file_path,
            commands::verify_file_exists,
            commands::verify_all_files,
            commands::get_orphaned_files,
            commands::get_missing_files,
            commands::unlink_file,
            commands::get_file_metadata,
            commands::is_file_tracked,
            commands::validate_config,
            commands::get_history_batch,
            commands::delete_history_batch,
            commands::play_history_batch,
            commands::trigger_update_check,
            commands::get_installer_script_path,
            commands::run_kittentts_installer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CopySpeak");
}
