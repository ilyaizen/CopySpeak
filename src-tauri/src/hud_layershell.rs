//! Wayland layer-surface setup for the HUD window.
//!
//! A plain Wayland toplevel cannot position itself — the compositor owns
//! xdg_toplevel placement, which is why every `set_position` on the HUD was
//! clamped/ignored (even on-screen coordinates landed at (0, 0)). The
//! supported mechanism for overlay windows is the wlr-layer-shell protocol:
//! instead of coordinates, the surface declares anchors (screen edges) +
//! margins and the compositor places it. Hyprland, sway, and other
//! wlroots-family compositors implement it; GNOME does not.
//!
//! The GTK window must be initialized BEFORE it is first mapped. After that,
//! anchors/margins can be re-applied at any time (the window re-maps) —
//! `apply_anchors` is idempotent and is what the position code calls instead
//! of `set_position` on Wayland.

use crate::config::{HudPosition, HudPresetPosition};
use gtk_layer_shell::LayerShell;

/// Layer-shell margins are logical (pre-scale) pixels; our config margin and
/// compute_hud_position both work in physical pixels, so divide by the
/// monitor's scale factor on HiDPI outputs.
fn margin_logical(margin_px: i32, scale: f64) -> i32 {
    if scale > 1.0 {
        (margin_px as f64 / scale).round() as i32
    } else {
        margin_px
    }
}

/// True if GTK is actually running on a native Wayland display — the only
/// case where layer-shell can work. Checking WAYLAND_DISPLAY is NOT enough:
/// main.rs forces GDK_BACKEND=x11 on Hyprland (XWayland fallback), where the
/// env var is still set but init_layer_shell fails with "Shell does not
/// support XDG shell stable". Must be called from the main thread (GDK).
pub fn is_wayland() -> bool {
    gtk::gdk::prelude::DisplayExtManual::backend(
        &gtk::gdk::Display::default()
            .unwrap_or_else(|| panic!("no GDK display — GTK not initialized")),
    )
    .is_wayland()
}

/// Apply (or re-apply) layer-shell anchors for the given preset.
///
/// Idempotent: initializes the surface on first call (must happen before the
/// window is first mapped — the startup path guarantees that), then sets
/// anchors + margins. Safe to call again on every position change.
pub fn apply_anchors(
    gtk_win: &gtk::ApplicationWindow,
    position: &HudPosition,
    margin_px: i32,
    scale: f64,
) -> Result<(), String> {
    let HudPosition::Preset(preset) = position;

    use gtk_layer_shell::{Edge, KeyboardMode, Layer};

    if !gtk_win.is_layer_window() {
        if !is_wayland() {
            return Err("not a Wayland session".to_string());
        }
        // Attach the zwlr_layer_surface_v1 role. From here on the compositor,
        // not the app, places the window.
        gtk_win.init_layer_shell();
        gtk_win.set_layer(Layer::Overlay);
        gtk_win.set_namespace("copyspeak-hud");
        // The HUD is display-only (click-through via set_ignore_cursor_events
        // on the input side); never take keyboard focus.
        gtk_win.set_keyboard_mode(KeyboardMode::None);
    }

    let (anchor_top, anchor_bottom, anchor_left, anchor_right) = match preset {
        HudPresetPosition::TopLeft => (true, false, true, false),
        HudPresetPosition::TopCenter => (true, false, true, true),
        HudPresetPosition::TopRight => (true, false, false, true),
        HudPresetPosition::BottomLeft => (false, true, true, false),
        HudPresetPosition::BottomCenter => (false, true, true, true),
        HudPresetPosition::BottomRight => (false, true, false, true),
    };

    gtk_win.set_anchor(Edge::Top, anchor_top);
    gtk_win.set_anchor(Edge::Bottom, anchor_bottom);
    gtk_win.set_anchor(Edge::Left, anchor_left);
    gtk_win.set_anchor(Edge::Right, anchor_right);

    // Corner presets: anchor two edges, margin insets from each.
    // Center presets: anchored on both long-axis edges (which centers the
    // surface on that axis), so their margins must be 0; margin applies to
    // the anchored short edge only.
    let (h_margin, v_margin) = match preset {
        HudPresetPosition::TopCenter | HudPresetPosition::BottomCenter => (0, margin_px),
        _ => (margin_px, margin_px),
    };

    gtk_win.set_layer_shell_margin(Edge::Left, margin_logical(h_margin, scale));
    gtk_win.set_layer_shell_margin(Edge::Right, margin_logical(h_margin, scale));
    gtk_win.set_layer_shell_margin(Edge::Top, margin_logical(v_margin, scale));
    gtk_win.set_layer_shell_margin(Edge::Bottom, margin_logical(v_margin, scale));

    Ok(())
}
