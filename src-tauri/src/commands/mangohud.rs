// What the window asks about the MangoHud configuration: where it is, what it
// holds, and the three presets. Reading and writing happens here, in the
// process of the user, because the file is theirs.

use crate::mangohud::{self, Document, Setting};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangoHudState {
    /// The configuration file found for this user.
    pub path: String,
    pub exists: bool,
    /// The keys it already has, in the order of the file.
    pub entries: Vec<(String, String)>,
    /// Comments and blank lines, which are left untouched.
    pub other_lines: usize,
    /// Name of the last preset applied, when one was.
    pub preset: Option<String>,
    /// The keys the window can offer as boxes and fields.
    pub settings: &'static [Setting],
    /// The folder MangoHud writes its logs to, when the file names one.
    pub fps_log: Option<String>,
}

fn state_of(path: &std::path::Path) -> MangoHudState {
    let document = mangohud::read(path).unwrap_or_default();
    let fps_log = mangohud::log_folder(&document).map(|folder| folder.display().to_string());
    MangoHudState {
        path: path.display().to_string(),
        exists: path.exists(),
        entries: document.entries(),
        other_lines: document.lines.iter().filter(|line| matches!(line, mangohud::Line::Raw(_))).count(),
        preset: None,
        settings: mangohud::SETTINGS,
        fps_log,
    }
}

#[tauri::command]
pub fn mangohud_status() -> Result<MangoHudState, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    Ok(state_of(&path))
}

/// Changes the keys given, one by one, and nothing else.
#[tauri::command]
pub fn mangohud_set(changes: Vec<(String, String)>) -> Result<MangoHudState, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    for (key, value) in &changes {
        if !document.set(key, value) {
            return Err(format!("{key} does not take «{value}»"));
        }
    }
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}

#[tauri::command]
pub fn mangohud_remove(key: String) -> Result<MangoHudState, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    document.remove(&key);
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}

/// Applies one of the three presets on top of what the file has.
#[tauri::command]
pub fn mangohud_preset(name: String) -> Result<MangoHudState, String> {
    let pairs = mangohud::preset(&name).ok_or_else(|| format!("unknown preset: {name}"))?;
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    for (key, value) in pairs {
        document.set(key, value);
    }
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    let mut state = state_of(&path);
    state.preset = Some(name);
    Ok(state)
}

/// The file as text, for the section that shows it as it is.
#[tauri::command]
pub fn mangohud_text() -> Result<Value, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let document: Document = mangohud::read(&path).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({ "text": document.to_text() }))
}

/// Points MangoHud at a folder it can write in from every install, and makes
/// it log from the start of each game, which is what lets the panel read the
/// frames per second while a game runs.
#[tauri::command]
pub fn mangohud_fps_log() -> Result<MangoHudState, String> {
    let folder = mangohud::default_log_folder().ok_or("no home folder for this user")?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    // output_file is the old key, which MangoHud no longer reads.
    document.remove("output_file");
    if !document.set("output_folder", &folder.display().to_string()) {
        return Err("output_folder refused the path".to_string());
    }
    // Start logging one second after the game opens, with no time limit. The
    // interval must not be 0: with 0 MangoHud keeps no logging thread and
    // nothing reaches the file while the game runs.
    document.set("autostart_log", "1");
    document.set("log_duration", "0");
    document.set("log_interval", "500");
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}

/// Stops MangoHud from writing the log, taking only those keys away.
#[tauri::command]
pub fn mangohud_fps_log_off() -> Result<MangoHudState, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    for key in ["output_file", "output_folder", "autostart_log", "log_duration", "log_interval"] {
        document.remove(key);
    }
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}
