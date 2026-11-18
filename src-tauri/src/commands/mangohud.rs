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
    /// The log MangoHud writes to, when the file names one.
    pub fps_log: Option<String>,
}

fn state_of(path: &std::path::Path) -> MangoHudState {
    let document = mangohud::read(path).unwrap_or_default();
    let fps_log = mangohud::log_path(&document).map(|log| log.display().to_string());
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

/// Points MangoHud at a log inside the folder of the panel, which is what lets
/// the panel read the frames per second while a game runs.
#[tauri::command]
pub fn mangohud_fps_log() -> Result<MangoHudState, String> {
    let log = mangohud::default_log_path().ok_or("no home folder for this user")?;
    if let Some(folder) = log.parent() {
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    if !document.set("output_file", &log.display().to_string()) {
        return Err("output_file refused the path".to_string());
    }
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}

/// Stops MangoHud from writing the log, taking only that key away.
#[tauri::command]
pub fn mangohud_fps_log_off() -> Result<MangoHudState, String> {
    let path = mangohud::path().ok_or("no home folder for this user")?;
    let mut document = mangohud::read(&path).map_err(|error| error.to_string())?;
    document.remove("output_file");
    mangohud::write(&path, &document).map_err(|error| error.to_string())?;
    Ok(state_of(&path))
}
