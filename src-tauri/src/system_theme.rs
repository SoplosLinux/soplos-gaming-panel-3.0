// The light or dark theme of the desktop, so the window can follow it. Each
// desktop keeps the answer somewhere else: GNOME in gsettings, KDE in
// kdeglobals, Xfce in the xsettings channel of xfconf.

use std::process::Command;

fn out(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    if !result.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&result.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase()
}

fn from_name(name: &str) -> Option<&'static str> {
    let name = name.to_lowercase();
    if name.contains("dark") || name.contains("black") {
        Some("dark")
    } else if name.is_empty() {
        None
    } else {
        Some("light")
    }
}

fn gnome() -> Option<&'static str> {
    match out("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"]).as_deref() {
        Some("'prefer-dark'") => return Some("dark"),
        Some("'prefer-light'") => return Some("light"),
        // "default" leaves the GTK theme as the only answer.
        _ => {}
    }
    out("gsettings", &["get", "org.gnome.desktop.interface", "gtk-theme"]).and_then(|name| from_name(name.trim_matches('\'')))
}

fn kde() -> Option<&'static str> {
    let args = ["--file", "kdeglobals", "--group", "General", "--key", "ColorScheme"];
    out("kreadconfig6", &args).or_else(|| out("kreadconfig5", &args)).and_then(|name| from_name(&name))
}

fn xfce() -> Option<&'static str> {
    if out("xfconf-query", &["-c", "xsettings", "-p", "/Net/PreferDarkTheme"]).as_deref() == Some("true") {
        return Some("dark");
    }
    out("xfconf-query", &["-c", "xsettings", "-p", "/Net/ThemeName"]).and_then(|name| from_name(&name))
}

fn gtk_theme() -> Option<&'static str> {
    out("gsettings", &["get", "org.gnome.desktop.interface", "gtk-theme"]).and_then(|name| from_name(name.trim_matches('\'')))
}

/// "dark", "light" or "unknown" when the desktop does not answer.
#[tauri::command]
pub fn get_system_theme() -> String {
    let de = desktop();
    let found = if de.contains("gnome") || de.contains("unity") || de.contains("pantheon") {
        gnome().or_else(gtk_theme)
    } else if de.contains("kde") || de.contains("plasma") {
        kde().or_else(gtk_theme)
    } else if de.contains("xfce") {
        xfce().or_else(gtk_theme)
    } else {
        gnome().or_else(kde).or_else(xfce).or_else(gtk_theme)
    };
    found.unwrap_or("unknown").to_string()
}
