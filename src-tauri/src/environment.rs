// The desktop and the display protocol of the session, for the footer of the
// window. The same detection as the other Soplos 3.0 programs.

use serde::Serialize;

#[derive(Serialize)]
pub struct EnvironmentInfo {
    #[serde(rename = "desktopEnvironment")]
    pub desktop_environment: String,
    #[serde(rename = "displayProtocol")]
    pub display_protocol: String,
}

pub fn desktop_environment() -> String {
    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if current_desktop.contains("gnome") {
        "gnome"
    } else if current_desktop.contains("kde") || current_desktop.contains("plasma") {
        "kde"
    } else if current_desktop.contains("xfce") {
        "xfce"
    } else if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        "gnome"
    } else if std::env::var("KDE_SESSION_VERSION").is_ok() {
        "kde"
    } else if std::env::var("XFCE_PANEL_MIGRATE_DEFAULT").is_ok() {
        "xfce"
    } else {
        "unknown"
    }
    .to_string()
}

pub fn display_protocol() -> String {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();
    if session_type == "wayland" || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "wayland"
    } else if session_type == "x11" || std::env::var("DISPLAY").is_ok() {
        "x11"
    } else {
        "unknown"
    }
    .to_string()
}

#[tauri::command]
pub fn get_environment_info() -> EnvironmentInfo {
    EnvironmentInfo {
        desktop_environment: desktop_environment(),
        display_protocol: display_protocol(),
    }
}
