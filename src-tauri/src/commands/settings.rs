// The settings of the window: reading them, changing a few keys and letting
// the rest of the program know what changed.

use serde_json::Value;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn get_settings() -> crate::config::Settings {
    let mut settings = crate::config::load();
    // The file of the session can be removed by hand; then the switch says so.
    if settings.start_with_session && !crate::autostart::is_enabled() {
        settings.start_with_session = false;
        let _ = crate::config::save(&settings);
    }
    settings
}

#[tauri::command]
pub fn config_path() -> String {
    crate::config::config_path().to_string_lossy().to_string()
}

/// Saves the keys of the patch and applies what cannot wait for a restart:
/// the file of the session and the shortcuts.
#[tauri::command]
pub fn set_settings(app: AppHandle, patch: Value) -> Result<crate::config::Settings, String> {
    let before = crate::config::load();
    let after = crate::config::apply_patch(patch)?;
    take_effect(&app, &before, &after)?;
    Ok(after)
}

/// Writes the settings to a file the user chose, to take them to another
/// machine.
#[tauri::command]
pub fn settings_export(path: String) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&crate::config::load()).map_err(|error| error.to_string())?;
    std::fs::write(&path, format!("{text}\n")).map_err(|error| error.to_string())
}

/// Takes the settings of a file the user chose, kept inside what the program
/// can draw with, and applies them as if they had been changed one by one.
#[tauri::command]
pub fn settings_import(app: AppHandle, path: String) -> Result<crate::config::Settings, String> {
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    // A file of settings is a few hundred bytes.
    if metadata.len() > 64 * 1024 {
        return Err("The file is too big to be settings of the panel".to_string());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let after = crate::config::from_text(&text)?;
    replace(&app, after)
}

/// Goes back to the settings the program comes with.
#[tauri::command]
pub fn settings_reset(app: AppHandle) -> Result<crate::config::Settings, String> {
    replace(&app, crate::config::Settings::default())
}

fn replace(app: &AppHandle, after: crate::config::Settings) -> Result<crate::config::Settings, String> {
    let before = crate::config::load();
    crate::config::save(&after)?;
    take_effect(app, &before, &after)?;
    let _ = app.emit("settings-replaced", serde_json::to_value(&after).unwrap_or_default());
    Ok(after)
}

/// What a change of the settings does at once instead of at the next start:
/// the file of the session, the shortcuts, the tray and the overlay.
fn take_effect(app: &AppHandle, before: &crate::config::Settings, after: &crate::config::Settings) -> Result<(), String> {
    if after.start_with_session != before.start_with_session {
        crate::autostart::set(after.start_with_session)?;
    }
    if after.panel_hotkey != before.panel_hotkey || after.hud_hotkey != before.hud_hotkey {
        let _ = crate::hotkey::register_all(app);
    }
    // The menu of the tray is built once, at start: it is written again so it
    // does not keep the language the program had then.
    if after.language != before.language {
        crate::tray::refresh_texts(app);
    }
    // Turning the switch of the settings on or off is the same as using the
    // tray or the shortcut: the window has to appear or go away, not only the
    // value in the file change.
    if after.hud.enabled != before.hud.enabled {
        if after.hud.enabled {
            crate::show_hud(app);
        } else {
            crate::hide_hud(app);
        }
    }
    // The overlay is a window of its own and reads its settings when it draws,
    // but where it sits and how big it is belong to the window itself.
    let moved = after.hud.corner != before.hud.corner
        || after.hud.scale != before.hud.scale
        || after.hud.mode != before.hud.mode;
    if moved {
        crate::place_hud(app);
    }
    let _ = app.emit("hud-settings", serde_json::to_value(&after.hud).unwrap_or_default());
    Ok(())
}

/// Registers the shortcuts of config.json again. The answer says what the
/// desktop did with them; an empty answer is a plain success.
#[tauri::command]
pub fn apply_hotkeys(app: AppHandle) -> String {
    match crate::hotkey::register_all(&app) {
        Ok(notes) => notes,
        Err(e) => e,
    }
}

#[tauri::command]
pub fn set_hud_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::config::load();
    settings.hud.enabled = enabled;
    crate::config::save(&settings)?;
    crate::tray::refresh_hud_item(&app, enabled);
    let _ = app.emit("hud-enabled", enabled);
    Ok(())
}
