//! Tray artwork and window lifecycle. Native icons are decoded once, not per reading.

use crate::config::{AppConfig, CloseBehavior};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IconState {
    Idle,
    Synthesizing,
    Speaking,
    Paused,
}

fn icon_index(state: IconState) -> usize {
    match state {
        IconState::Idle => 0,
        IconState::Synthesizing => 1,
        IconState::Speaking => 2,
        IconState::Paused => 3,
    }
}

fn icon_state(synthesizing: bool, playing: bool, paused: bool, listening: bool) -> IconState {
    if paused {
        IconState::Paused
    } else if playing {
        IconState::Speaking
    } else if synthesizing {
        IconState::Synthesizing
    } else if !listening {
        IconState::Paused
    } else {
        IconState::Idle
    }
}

pub fn should_start_hidden(has_config: bool, start_minimized: bool, tray_available: bool) -> bool {
    has_config && start_minimized && tray_available
}

fn should_close_to_tray(close_to_tray: bool, tray_available: bool) -> bool {
    close_to_tray && tray_available
}

struct TrayInner {
    playing: bool,
    paused: bool,
    applied: Option<IconState>,
}

/// Decoded assets are immutable; only the tiny status snapshot is locked.
pub struct TrayState {
    icons: Vec<Image<'static>>,
    inner: Mutex<TrayInner>,
    available: AtomicBool,
}

impl TrayState {
    pub fn new() -> tauri::Result<Self> {
        let bytes: [&[u8]; 4] = [
            include_bytes!("../icons/icon.png"),
            include_bytes!("../icons/app-icon-inverted.png"),
            include_bytes!("../icons/tray/speaking.png"),
            include_bytes!("../icons/tray/paused.png"),
        ];
        Ok(Self {
            icons: bytes
                .into_iter()
                .map(Image::from_bytes)
                .collect::<tauri::Result<_>>()?,
            inner: Mutex::new(TrayInner {
                playing: false,
                paused: false,
                applied: None,
            }),
            available: AtomicBool::new(false),
        })
    }

    fn icon(&self, state: IconState) -> Image<'static> {
        self.icons[icon_index(state)].clone()
    }
}

pub fn initial_icon(app: &AppHandle) -> Image<'static> {
    app.state::<TrayState>().icon(IconState::Idle)
}

/// Read the latest snapshot ON the main thread so queued updates cannot apply stale state.
/// Never lock AudioPlayer here: its own callbacks also refresh the tray.
pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let Some(tray) = handle.tray_by_id("main") else {
            return;
        };
        let state = handle.state::<TrayState>();
        let mut inner = state.inner.lock().unwrap();
        let listening = handle.state::<Arc<AtomicBool>>().load(Ordering::Relaxed);
        let synthesizing = handle
            .state::<crate::JobStatus>()
            .is_synthesizing
            .load(Ordering::Relaxed);
        let icon = icon_state(synthesizing, inner.playing, inner.paused, listening);
        if inner.applied != Some(icon) {
            let result = tray.set_icon_with_as_template(Some(state.icon(icon)), false);
            if let Err(error) = result {
                log::warn!("[Tray] Icon update failed: {error}");
            } else {
                inner.applied = Some(icon);
            }
        }
        let busy = synthesizing || inner.playing || inner.paused;
        drop(inner);
        if let Some(items) = handle.try_state::<crate::tray_labels::TrayItems>() {
            let _ = items.stop.set_enabled(busy);
        }
    }) {
        log::warn!("[Tray] Could not schedule update: {error}");
    }
}

/// The webview owns audible audio, including PCM streams and pause state.
#[tauri::command]
pub fn set_tray_playback_state(
    app: AppHandle,
    state: State<'_, TrayState>,
    playing: bool,
    paused: bool,
) {
    {
        let mut inner = state.inner.lock().unwrap();
        inner.playing = playing;
        inner.paused = paused;
    }
    refresh(&app);
}

pub fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn show_settings(app: &AppHandle) {
    show_main_window(app);
    let _ = app.emit_to("main", "open-settings", ());
}

pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

pub fn close_main_window(app: &AppHandle) {
    let close_to_tray = {
        let cfg = app.state::<Mutex<AppConfig>>();
        let cfg = cfg.lock().unwrap();
        should_close_to_tray(
            cfg.general.close_behavior == CloseBehavior::MinimizeToTray,
            cfg.general.show_tray_icon
                && app.state::<TrayState>().available.load(Ordering::Relaxed),
        )
    };
    if close_to_tray {
        hide_main_window(app);
    } else {
        app.exit(0);
    } // The permanent HUD must not keep a windowless app alive.
}

/// Apply visibility on save, without hiding a window the user is working in.
pub fn sync_visibility(app: &AppHandle, visible: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_visible(visible)
            .map_err(|error| error.to_string())?;
        app.state::<TrayState>()
            .available
            .store(visible, Ordering::Relaxed);
    } else if visible {
        return Err("Tray is unavailable; restart CopySpeak to try again".into());
    }
    if !visible {
        show_main_window(app);
    }
    Ok(())
}

pub fn set_available(app: &AppHandle, available: bool) {
    app.state::<TrayState>()
        .available
        .store(available, Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
pub fn recover(app: &AppHandle) {
    let visible = app
        .state::<Mutex<AppConfig>>()
        .lock()
        .unwrap()
        .general
        .show_tray_icon;
    if visible {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_visible(false).and_then(|_| tray.set_visible(true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_start_requires_a_saved_config_and_a_working_tray() {
        assert!(should_start_hidden(true, true, true));
        assert!(!should_start_hidden(false, true, true));
        assert!(!should_start_hidden(true, false, true));
        assert!(!should_start_hidden(true, true, false));
    }

    #[test]
    fn closing_without_a_tray_exits_instead_of_stranding_the_app() {
        assert!(should_close_to_tray(true, true));
        assert!(!should_close_to_tray(true, false));
        assert!(!should_close_to_tray(false, true));
    }

    #[test]
    fn audible_playback_takes_priority_over_background_synthesis() {
        assert_eq!(icon_state(true, true, false, true), IconState::Speaking);
        assert_eq!(icon_state(true, true, true, true), IconState::Paused);
        assert_eq!(
            icon_state(true, false, false, true),
            IconState::Synthesizing
        );
        assert_eq!(icon_state(false, false, false, false), IconState::Paused);
        assert_eq!(icon_state(false, false, false, true), IconState::Idle);
    }

    #[test]
    fn branded_states_have_distinct_decodable_icons() {
        let tray = TrayState::new().expect("tray images must decode");
        let states = [
            IconState::Idle,
            IconState::Synthesizing,
            IconState::Speaking,
            IconState::Paused,
        ];
        for (index, state) in states.into_iter().enumerate() {
            assert_eq!(icon_index(state), index);
            assert!(!tray.icon(state).rgba().is_empty());
        }
    }
}
