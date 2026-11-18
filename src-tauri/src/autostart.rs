// Starting with the session: a .desktop file in ~/.config/autostart, which
// every desktop reads on its own. No extra package is needed for it.

use std::fs;
use std::path::PathBuf;

fn autostart_file() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .map(|base| base.join("autostart").join("soplos-gaming-panel.desktop"))
}

/// The program that is running: in a package it is the one in /usr/bin, which
/// is also the one the session starts later.
fn command() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "soplos-gaming-panel".to_string())
}

pub fn is_enabled() -> bool {
    autostart_file().is_some_and(|path| path.is_file())
}

pub fn set(enabled: bool) -> Result<(), String> {
    let path = autostart_file().ok_or("The home folder is not known")?;
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=Soplos Gaming Panel\n\
         Comment=Live gaming companion for Soplos Linux\n\
         Exec={}\n\
         Icon=soplos-gaming-panel\n\
         Terminal=false\n\
         Categories=Utility;\n\
         X-GNOME-Autostart-enabled=true\n\
         NoDisplay=false\n",
        command()
    );
    fs::write(&path, entry).map_err(|e| e.to_string())
}
