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
        let icons = bytes
            .into_iter()
            .map(Image::from_bytes)
            .collect::<tauri::Result<Vec<_>>>()?;
        #[cfg(target_os = "windows")]
        let icons = {
            let icon_size = windows_tray_icon_size();
            icons
                .into_iter()
                .map(|icon| resize_tray_icon(icon, icon_size))
                .collect()
        };
        Ok(Self {
            icons,
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

#[cfg(target_os = "windows")]
fn windows_tray_icon_size() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};

    // SAFETY: GetSystemMetrics reads process-global metrics and accepts these constants.
    let width = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let height = unsafe { GetSystemMetrics(SM_CYSMICON) };
    (
        if width > 0 { width as u32 } else { 16 },
        if height > 0 { height as u32 } else { 16 },
    )
}

#[cfg(target_os = "windows")]
fn resize_tray_icon(icon: Image<'static>, (width, height): (u32, u32)) -> Image<'static> {
    use image::{imageops, ImageBuffer, Rgba};

    // Filter premultiplied colors so transparent pixels cannot create dark halos.
    let pixels = icon
        .rgba()
        .chunks_exact(4)
        .flat_map(|pixel| {
            let alpha = f32::from(pixel[3]) / 255.0;
            [
                f32::from(pixel[0]) / 255.0 * alpha,
                f32::from(pixel[1]) / 255.0 * alpha,
                f32::from(pixel[2]) / 255.0 * alpha,
                alpha,
            ]
        })
        .collect();
    let source: ImageBuffer<Rgba<f32>, Vec<f32>> =
        ImageBuffer::from_raw(icon.width(), icon.height(), pixels)
            .expect("decoded tray image must contain valid RGBA pixels");
    let resized = imageops::resize(&source, width, height, imageops::FilterType::Lanczos3);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for pixel in resized.pixels() {
        let alpha = pixel[3].clamp(0.0, 1.0);
        for channel in &pixel.0[..3] {
            let unpremultiplied = if alpha > 0.0 { channel / alpha } else { 0.0 };
            rgba.push((unpremultiplied.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        rgba.push((alpha * 255.0).round() as u8);
    }
    Image::new_owned(rgba, width, height)
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

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_tray_states_are_sized_for_the_system_small_icon() {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};

        let tray = TrayState::new().expect("tray images must decode");
        let size = unsafe { (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)) };
        assert!(size.0 > 0 && size.1 > 0);
        for state in [
            IconState::Idle,
            IconState::Synthesizing,
            IconState::Speaking,
            IconState::Paused,
        ] {
            let icon = tray.icon(state);
            assert_eq!(
                (icon.width(), icon.height()),
                (size.0 as u32, size.1 as u32)
            );
            assert!(icon
                .rgba()
                .chunks_exact(4)
                .any(|pixel| (1..255).contains(&pixel[3])));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn tray_resize_preserves_color_across_transparent_edges() {
        let mut rgba = Vec::new();
        for y in 0..32 {
            for x in 0..32 {
                if (8..24).contains(&x) && (8..24).contains(&y) {
                    rgba.extend_from_slice(&[30, 180, 100, 255]);
                } else {
                    rgba.extend_from_slice(&[255, 0, 255, 0]);
                }
            }
        }
        let resized = resize_tray_icon(Image::new_owned(rgba, 32, 32), (16, 16));
        let mut antialiased = 0;
        for pixel in resized.rgba().chunks_exact(4) {
            if (8..247).contains(&pixel[3]) {
                antialiased += 1;
                assert!(pixel[0].abs_diff(30) <= 2, "{pixel:?}");
                assert!(pixel[1].abs_diff(180) <= 2, "{pixel:?}");
                assert!(pixel[2].abs_diff(100) <= 2, "{pixel:?}");
            }
        }
        assert!(antialiased > 0);
    }
}
