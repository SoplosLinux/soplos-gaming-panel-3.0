// What the window asks about Feral GameMode: the settings of gamemode.ini and
// the value each one has, including the ones the file does not carry yet.

use crate::gamemode;
use serde::Serialize;

/// A setting the panel offers, with the value Feral uses when the file does not
/// say anything.
#[derive(Serialize)]
pub struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub value: Option<String>,
    pub default: &'static str,
    /// Values to choose from, when the key takes only a few.
    pub choices: &'static [&'static str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameModeIni {
    pub path: String,
    pub exists: bool,
    /// There is also a file of the system, which this panel does not touch.
    pub system: bool,
    pub settings: Vec<Setting>,
    /// Every key the file carries, including the ones the panel does not show.
    pub keys: Vec<(String, String)>,
}

fn catalogue() -> Vec<(&'static str, &'static str, &'static str, &'static [&'static str])> {
    vec![
        ("general", "renice", "0", &[]),
        ("general", "inhibit_screensaver", "1", &["0", "1"]),
        ("general", "disable_splitlock", "1", &["0", "1"]),
        ("general", "softrealtime", "off", &["off", "on"]),
        ("general", "reaper_freq", "5", &[]),
        ("general", "desiredgov", "performance", &["performance", "powersave", "schedutil", "ondemand"]),
        ("general", "defaultgov", "powersave", &["performance", "powersave", "schedutil", "ondemand"]),
        ("gpu", "apply_gpu_optimisations", "0", &["0", "1"]),
        ("gpu", "amd_performance_level", "auto", &["auto", "low", "high", "manual"]),
        ("gpu", "nv_powermizer_mode", "1", &["0", "1", "2"]),
    ]
}

fn state_of(path: &std::path::Path) -> Result<GameModeIni, String> {
    let document = gamemode::read(path).map_err(|error| error.to_string())?;
    let settings = catalogue()
        .into_iter()
        .map(|(section, key, default, choices)| Setting {
            section,
            key,
            value: document.get(section, key).map(str::to_string),
            default,
            choices,
        })
        .collect();
    let mut keys = document.section("general");
    keys.extend(document.section("gpu"));
    keys.extend(document.section("custom"));
    Ok(GameModeIni {
        path: path.display().to_string(),
        exists: path.exists(),
        system: gamemode::system_path().exists(),
        settings,
        keys,
    })
}

#[tauri::command]
pub fn gamemode_ini_status() -> Result<GameModeIni, String> {
    let path = gamemode::path().ok_or("no home folder for this user")?;
    state_of(&path)
}

#[tauri::command]
pub fn gamemode_ini_set(section: String, key: String, value: String) -> Result<GameModeIni, String> {
    let path = gamemode::path().ok_or("no home folder for this user")?;
    let mut document = gamemode::read(&path).map_err(|error| error.to_string())?;
    if !document.set(&section, &key, &value) {
        return Err(format!("{key} does not take «{value}»"));
    }
    gamemode::write(&path, &document).map_err(|error| error.to_string())?;
    state_of(&path)
}

/// Takes a key out of the file, so GameMode goes back to its own value.
#[tauri::command]
pub fn gamemode_ini_remove(section: String, key: String) -> Result<GameModeIni, String> {
    let path = gamemode::path().ok_or("no home folder for this user")?;
    let mut document = gamemode::read(&path).map_err(|error| error.to_string())?;
    document.remove(&section, &key);
    gamemode::write(&path, &document).map_err(|error| error.to_string())?;
    state_of(&path)
}

#[tauri::command]
pub fn gamemode_ini_text() -> Result<String, String> {
    let path = gamemode::path().ok_or("no home folder for this user")?;
    Ok(gamemode::read(&path).map_err(|error| error.to_string())?.to_text())
}
