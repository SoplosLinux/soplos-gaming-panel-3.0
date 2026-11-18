// What the window asks about the games installed: which ones there are, the
// picture of each, the one the user chose, and starting them.
//
// Nothing here needs the administrator: a game is started the same way the
// menu of the desktop starts it.

use crate::library;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub id: String,
    pub name: String,
    /// "menu", "steam" or "lutris".
    pub source: String,
    /// The program the daemon looks for in /proc, when there is one.
    pub process: Option<String>,
    pub app_id: Option<String>,
    /// Small picture of the desktop, when it has one.
    pub icon: Option<String>,
    /// The cover: the one the user chose, else the one Steam keeps.
    pub cover: Option<String>,
}

fn view_of(game: &library::Game) -> GameView {
    let cover = library::cover_of(&game.id).or_else(|| game.picture.clone()).or_else(|| game.icon.clone());
    GameView {
        id: game.id.clone(),
        name: game.name.clone(),
        source: game.source.to_string(),
        process: game.process.clone(),
        app_id: game.app_id.clone(),
        icon: game.icon.as_deref().and_then(library::data_url),
        cover: cover.as_deref().and_then(library::data_url),
    }
}

#[tauri::command]
pub fn library_list() -> Vec<GameView> {
    library::list().iter().map(view_of).collect()
}

/// Starts a game the same way its own entry of the menu does.
#[tauri::command]
pub fn library_launch(id: String) -> Result<(), String> {
    let game = library::find(&id).ok_or_else(|| format!("unknown game: {id}"))?;
    if let Some(desktop) = game.desktop {
        // gio is what the menu itself uses, so a Flatpak entry keeps working.
        return std::process::Command::new("gio")
            .arg("launch")
            .arg(&desktop)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    if let Some(lutris) = &game.lutris {
        return library::launch_lutris(lutris).map(|_| ()).map_err(|error| error.to_string());
    }
    let app_id = game.app_id.ok_or("the game has nothing to start")?;
    // The client is what knows how to start a game of Steam, even when it is
    // already running.
    std::process::Command::new("steam")
        .arg("-applaunch")
        .arg(&app_id)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn library_set_cover(id: String, path: String) -> Result<(), String> {
    library::set_cover(&id, std::path::Path::new(&path)).map(|_| ())
}

#[tauri::command]
pub fn library_clear_cover(id: String) -> Result<(), String> {
    library::clear_cover(&id)
}
