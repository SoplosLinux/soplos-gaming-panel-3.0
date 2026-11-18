// The overlay as a widget of the desktop, not as a window.
//
// A window is the business of the window manager: on Wayland a program cannot
// place it, and a tiling script takes it into a slot or centers it. The panels
// of a desktop are not windows: they are layer-shell surfaces, which the
// compositor anchors to an edge or a corner and never tiles. KWin (Plasma) and
// xfwl4 (Xfce 4.22) speak that protocol; Mutter (GNOME) does not, and there the
// extension of the package places the overlay (see resources/gnome-shell).
// On X11 the window is marked as a panel instead (see `mark_as_panel`).

use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the overlay was made a layer-shell surface. Once it is one, its
/// place is set with anchors and margins, not with a position.
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// The space between the small card and the edges of the screen.
const MARGIN: i32 = 24;

/// Turns the window of the overlay into a layer-shell surface, when the
/// session is Wayland and the compositor speaks the protocol. It has to happen
/// before GTK realizes the window, so it runs on the main thread, before the
/// overlay is first shown. Returns whether it did.
pub fn make_widget(window: &tauri::WebviewWindow) -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() || !gtk_layer_shell::is_supported() {
        return false;
    }
    let Ok(gtk_window) = window.gtk_window() else { return false };
    if gtk_window.is_realized() {
        // The protocol is chosen when the surface is created: a window GTK has
        // already realized has to go through it again.
        gtk_window.hide();
        gtk_window.unrealize();
    }
    gtk_window.init_layer_shell();
    gtk_window.set_namespace("soplos-gaming-overlay");
    // Over the windows and the panels, as the overlay of a game should be.
    gtk_window.set_layer(Layer::Overlay);
    // It shows the machine and takes clicks; the keyboard stays with the game.
    gtk_window.set_keyboard_mode(KeyboardMode::None);
    // It never pushes the windows of the desktop aside.
    gtk_window.set_exclusive_zone(0);
    ACTIVE.store(true, Ordering::Relaxed);
    true
}

/// Puts the surface where the settings ask: the small card in a corner, or the
/// panel of the height of the screen against one side.
pub fn place(window: &tauri::WebviewWindow, mode: &str, corner: &str, scale: u16) {
    let Ok(gtk_window) = window.gtk_window() else { return };
    let factor = f64::from(scale.clamp(50, 200)) / 100.0;
    let right = corner.ends_with("right");
    let bottom = corner.starts_with("bottom");
    if mode == "advanced" {
        gtk_window.set_anchor(Edge::Top, true);
        gtk_window.set_anchor(Edge::Bottom, true);
        gtk_window.set_anchor(Edge::Left, !right);
        gtk_window.set_anchor(Edge::Right, right);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            gtk_window.set_layer_shell_margin(edge, 0);
        }
        // Anchored above and below, the compositor gives it the height.
        gtk_window.set_size_request((420.0 * factor) as i32, -1);
    } else {
        gtk_window.set_anchor(Edge::Top, !bottom);
        gtk_window.set_anchor(Edge::Bottom, bottom);
        gtk_window.set_anchor(Edge::Left, !right);
        gtk_window.set_anchor(Edge::Right, right);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            gtk_window.set_layer_shell_margin(edge, MARGIN);
        }
        gtk_window.set_size_request((360.0 * factor) as i32, (300.0 * factor) as i32);
    }
}
