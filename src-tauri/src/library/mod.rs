// The games installed for this user, and what starts them.
//
// Three honest sources: the menu of the desktop, where a file with
// `Categories=Game` names the program (emulators and Flatpak games included);
// Steam's own manifests, because Steam puts a single entry in the menu and
// keeps its games in `steamapps`; and Lutris, which lists the games it has
// installed when asked. Nothing is guessed from a running process.

use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The biggest picture read into the window. A cover bigger than this is left
/// out instead of turning the list into megabytes of text.
const PICTURE_LIMIT: u64 = 400 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    /// The same game keeps the same id: the entry of the menu or the app of
    /// Steam it comes from.
    pub id: String,
    pub name: String,
    /// "menu", "steam" or "lutris".
    pub source: &'static str,
    /// The program the daemon looks for in `/proc`, when the entry names one.
    pub process: Option<String>,
    /// The `.desktop` that starts it, when it comes from the menu.
    pub desktop: Option<PathBuf>,
    /// The app of Steam, when it comes from there.
    pub app_id: Option<String>,
    /// The picture the desktop offers, wherever the theme keeps it.
    pub icon: Option<PathBuf>,
    /// The cover Steam or Lutris keeps of the game, when it has one.
    pub picture: Option<PathBuf>,
    /// The game of Lutris, when it comes from there.
    pub lutris: Option<Lutris>,
}

/// A game of Lutris: its slug, which is what starts it, and whether the Lutris
/// that has it is the Flatpak one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lutris {
    pub slug: String,
    pub flatpak: bool,
}

/// Every game installed, ordered by name, each one once.
pub fn list() -> Vec<Game> {
    let mut games = from_menu();
    let names: Vec<String> = games.iter().map(|game| game.name.to_lowercase()).collect();
    games.extend(from_steam(names));
    let names: Vec<String> = games.iter().map(|game| game.name.to_lowercase()).collect();
    games.extend(from_lutris(names));
    games.sort_by(|one, other| one.name.to_lowercase().cmp(&other.name.to_lowercase()).then(one.id.cmp(&other.id)));
    games
}

/// One game by the id the window was given.
pub fn find(id: &str) -> Option<Game> {
    list().into_iter().find(|game| game.id == id)
}

// ── The menu of the desktop ─────────────────────────────────────────────────

fn from_menu() -> Vec<Game> {
    let mut games = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for dir in crate::governor::games::user_desktop_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
                continue;
            }
            let Some(game) = menu_game(&path) else { continue };
            if seen.contains(&game.id) {
                continue;
            }
            seen.push(game.id.clone());
            games.push(game);
        }
    }
    games
}

/// The name, the command and the picture of one entry of the menu, or nothing
/// when the entry is not a game or the user hid it.
fn menu_game(path: &Path) -> Option<Game> {
    let text = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut command = None;
    let mut icon = None;
    let mut is_game = false;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        // Only the main group: an action of the entry has a name and an Exec
        // of its own.
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            command = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Icon=") {
            icon = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Categories=") {
            is_game = value.split(';').any(|category| category == "Game");
        } else if line == "Hidden=true" {
            // The user took it away from the menu, so it is not a game of this
            // machine any more.
            return None;
        }
    }
    if !is_game {
        return None;
    }
    let name = name.filter(|value| !value.is_empty())?;
    // The panel and the clients of the stores say Game in their entries too,
    // and so do the shortcuts Lutris and Heroic write for a store client.
    let program = crate::governor::games::exec_program(&command?).filter(|name| !crate::governor::games::is_launcher(name))?;
    if is_store_client(&program, &name) {
        return None;
    }
    Some(Game {
        id: format!("menu:{}", path.display()),
        name,
        source: "menu",
        process: Some(program),
        desktop: Some(path.to_path_buf()),
        app_id: None,
        icon: icon.and_then(|value| icon_file(&value)),
        picture: None,
        lutris: None,
    })
}

/// Where the icon of an entry is. A full path is used as it is; a name is
/// looked for in the folders of the themes, which is where the desktop keeps
/// them.
fn icon_file(icon: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(icon);
    if direct.is_absolute() {
        return direct.exists().then_some(direct);
    }
    for dir in icon_dirs() {
        for size in ["256x256", "128x128", "64x64", "48x48", "32x32", "scalable"] {
            for extension in ["png", "svg"] {
                let candidate = dir.join(size).join("apps").join(format!("{icon}.{extension}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    for dir in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
        for extension in ["png", "svg"] {
            let candidate = Path::new(dir).join(format!("{icon}.{extension}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn icon_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/icons/hicolor"),
        PathBuf::from("/usr/local/share/icons/hicolor"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons/hicolor"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(Path::new(&home).join(".local/share/icons/hicolor"));
        dirs.push(Path::new(&home).join(".local/share/flatpak/exports/share/icons/hicolor"));
    }
    dirs
}

// ── Steam ───────────────────────────────────────────────────────────────────

fn from_steam(already: Vec<String>) -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    for root in steam_roots() {
        for folder in steam_folders(&root) {
            let steamapps = folder.join("steamapps");
            let Ok(entries) = fs::read_dir(&steamapps) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("appmanifest_") || path.extension().and_then(|value| value.to_str()) != Some("acf") {
                    continue;
                }
                let Some(game) = steam_game(&root, &path) else { continue };
                // Steam and the menu often install the same game; the entry of
                // the menu is kept because it starts without the client.
                if already.contains(&game.name.to_lowercase()) {
                    continue;
                }
                if games.iter().any(|one| one.id == game.id) {
                    continue;
                }
                games.push(game);
            }
        }
    }
    games
}

fn steam_game(root: &Path, manifest: &Path) -> Option<Game> {
    let text = fs::read_to_string(manifest).ok()?;
    let app_id = acf_value(&text, "appid")?;
    let name = acf_value(&text, "name")?;
    // Steam writes 4 in the state of a game that is fully installed.
    let state: u64 = acf_value(&text, "StateFlags").and_then(|value| value.parse().ok()).unwrap_or(4);
    if state & 4 == 0 || is_steam_tool(&app_id, &name) {
        return None;
    }
    Some(Game {
        id: format!("steam:{app_id}"),
        // The daemon names a running game of Steam after its manifest, so a
        // profile given here is found again when the game starts.
        process: Some(name.clone()),
        name,
        source: "steam",
        desktop: None,
        app_id: Some(app_id.clone()),
        icon: None,
        picture: steam_picture(root, &app_id),
        lutris: None,
    })
}

/// What Steam installs to run the games and is not one: Proton, the Steam
/// Linux Runtime and the common redistributables.
fn is_steam_tool(app_id: &str, name: &str) -> bool {
    app_id == "228980"
        || name.starts_with("Proton")
        || name.starts_with("Steam Linux Runtime")
        || name.starts_with("Steamworks Common")
}

/// The value of a key of a manifest of Steam, which is written with the key
/// and the value between quotes, one after the other.
pub(crate) fn acf_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split('"').map(str::trim).filter(|part| !part.is_empty());
        let (Some(one), Some(two)) = (parts.next(), parts.next()) else { continue };
        if one == key {
            return Some(two.to_string());
        }
    }
    None
}

/// The cover Steam keeps of a game. The tall one is the one the library of the
/// client shows; the wide one is the fallback.
fn steam_picture(root: &Path, app_id: &str) -> Option<PathBuf> {
    let base = root.join("appcache").join("librarycache").join(app_id);
    for name in ["library_600x900.jpg", "header.jpg", "library_header.jpg", "logo.png"] {
        let candidate = base.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var("HOME").ok().filter(|value| !value.is_empty()) else { return Vec::new() };
    steam_roots_in(Path::new(&home))
}

/// The folders where Steam may live in one home: the daemon runs as root and
/// asks for the homes of every user.
pub(crate) fn steam_roots_in(home: &Path) -> Vec<PathBuf> {
    [
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ]
    .into_iter()
    .filter(|path| path.join("steamapps").is_dir())
    .collect()
}

/// The folders of the libraries: the one of the root and the ones the user
/// added on another disk, which `libraryfolders.vdf` names.
pub(crate) fn steam_folders(root: &Path) -> Vec<PathBuf> {
    let mut folders = vec![root.to_path_buf()];
    let text = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")).unwrap_or_default();
    for line in text.lines() {
        let mut parts = line.split('"').map(str::trim).filter(|part| !part.is_empty());
        if let (Some(one), Some(two)) = (parts.next(), parts.next()) {
            if one == "path" {
                let path = PathBuf::from(two);
                if path.is_dir() && !folders.contains(&path) {
                    folders.push(path);
                }
            }
        }
    }
    folders
}

// ── Lutris ──────────────────────────────────────────────────────────────────

/// The databases of the Lutris of this user: the one of the system, the
/// Flatpak one, or both, each with whether it is the Flatpak one.
///
/// The database is read instead of asking `lutris -l`: Lutris is a single
/// instance application, and when its window is open the question goes to
/// that window and the list never comes back.
fn lutris_databases() -> Vec<(PathBuf, bool)> {
    let Some(home) = std::env::var("HOME").ok().filter(|value| !value.is_empty()) else { return Vec::new() };
    let home = Path::new(&home);
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    [(data.join("lutris/pga.db"), false), (home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db"), true)]
        .into_iter()
        .filter(|(path, _)| path.is_file())
        .collect()
}

/// The installed games of one database of Lutris: slug and name. The ones the
/// user hid in Lutris are left out.
fn lutris_games(database: &Path) -> Vec<(String, String)> {
    use rusqlite::{Connection, OpenFlags};
    // Read only, and without waiting: Lutris may have it open.
    let Ok(connection) = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) else {
        return Vec::new();
    };
    let hidden: Vec<i64> = connection
        .prepare(
            "SELECT game_id FROM games_categories JOIN categories ON categories.id = games_categories.category_id \
             WHERE categories.name = '.hidden'",
        )
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .unwrap_or_default();
    let Ok(mut statement) = connection.prepare("SELECT id, slug, name FROM games WHERE installed = 1") else { return Vec::new() };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?))
    }) else {
        return Vec::new();
    };
    rows.flatten()
        .filter(|(id, _, _)| !hidden.contains(id))
        .filter_map(|(_, slug, name)| {
            let slug = slug?.trim().to_string();
            let name = name?.trim().to_string();
            (!slug.is_empty() && !name.is_empty() && !is_store_client(&slug, &name)).then_some((slug, name))
        })
        .collect()
}

/// The clients of other stores, which Lutris installs as if they were games:
/// they open a store, not a game.
const STORE_CLIENTS: [&str; 13] = [
    "battlenet",
    "battle.net",
    "blizzard",
    "epic games",
    "epic-games",
    "ea app",
    "ea-app",
    "ubisoft connect",
    "ubisoft-connect",
    "uplay",
    "gog galaxy",
    "gog-galaxy",
    "amazon games",
];

fn is_store_client(slug: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    let slug = slug.to_lowercase();
    STORE_CLIENTS.iter().any(|client| slug.starts_with(client) || name.starts_with(client))
        // "Origin" alone: a game may start with the word.
        || slug == "origin"
        || name == "origin"
        || name.contains("launcher")
        || slug.contains("launcher")
}

/// The cover Lutris downloaded for a game, when it did.
fn lutris_picture(slug: &str, flatpak: bool) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").ok()?);
    let roots = if flatpak {
        vec![home.join(".var/app/net.lutris.Lutris/data/lutris"), home.join(".var/app/net.lutris.Lutris/cache/lutris")]
    } else {
        vec![home.join(".local/share/lutris"), home.join(".cache/lutris")]
    };
    for root in roots {
        for folder in ["coverart", "banners"] {
            for extension in ["jpg", "png"] {
                let candidate = root.join(folder).join(format!("{slug}.{extension}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn from_lutris(already: Vec<String>) -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    for (database, flatpak) in lutris_databases() {
        for (slug, name) in lutris_games(&database) {
            // A game that the menu or Steam already list keeps that entry.
            if already.contains(&name.to_lowercase()) || games.iter().any(|one| one.id == format!("lutris:{slug}")) {
                continue;
            }
            games.push(Game {
                id: format!("lutris:{slug}"),
                picture: lutris_picture(&slug, flatpak),
                name,
                source: "lutris",
                // Lutris starts its games through Wine or an emulator: there is no
                // program of its own to look for in /proc.
                process: None,
                desktop: None,
                app_id: None,
                icon: None,
                lutris: Some(Lutris { slug, flatpak }),
            });
        }
    }
    games
}

/// Starts a game of Lutris the way its own menu does.
pub fn launch_lutris(game: &Lutris) -> std::io::Result<std::process::Child> {
    let mut command = if game.flatpak {
        let mut command = std::process::Command::new("flatpak");
        command.args(["run", "net.lutris.Lutris"]);
        command
    } else {
        std::process::Command::new("lutris")
    };
    command.arg(format!("lutris:rungame/{}", game.slug)).spawn()
}

// ── Covers of the user ──────────────────────────────────────────────────────

fn data_dir() -> Option<PathBuf> {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|home| Path::new(&home).join(".local/share")))?;
    Some(base.join("soplos-gaming-panel"))
}

pub fn covers_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("covers"))
}

/// A name of its own for each game, so a cover never collides with another.
fn cover_stem(id: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn cover_of(id: &str) -> Option<PathBuf> {
    let dir = covers_dir()?;
    for extension in ["png", "jpg", "jpeg", "webp"] {
        let candidate = dir.join(format!("{}.{extension}", cover_stem(id)));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Copies the picture the user chose next to the covers of the panel, so it
/// still shows after the original is moved or deleted.
pub fn set_cover(id: &str, from: &Path) -> Result<PathBuf, String> {
    let extension = from
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_lowercase())
        .ok_or("the picture has no extension")?;
    if !["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
        return Err(format!("«{extension}» is not a picture the window can draw"));
    }
    let dir = covers_dir().ok_or("no home folder for this user")?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    clear_cover(id)?;
    let target = dir.join(format!("{}.{extension}", cover_stem(id)));
    fs::copy(from, &target).map_err(|error| error.to_string())?;
    Ok(target)
}

pub fn clear_cover(id: &str) -> Result<(), String> {
    if let Some(path) = cover_of(id) {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

// ── Pictures for the window ─────────────────────────────────────────────────

/// A picture as something an `<img>` can draw without reaching the disk: the
/// window is a web page and cannot read a file by its path.
pub fn data_url(path: &Path) -> Option<String> {
    let size = fs::metadata(path).ok()?.len();
    if size == 0 || size > PICTURE_LIMIT {
        return None;
    }
    let mime = match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    let bytes = fs::read(path).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_games_of_lutris_are_read_from_its_database() {
        let dir = scratch("sgp-library", "lutris");
        let database = dir.join("pga.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE games (id INTEGER PRIMARY KEY, name TEXT, slug TEXT, runner TEXT, installed INTEGER);
                 CREATE TABLE categories (id INTEGER PRIMARY KEY, name TEXT);
                 CREATE TABLE games_categories (game_id INTEGER, category_id INTEGER);
                 INSERT INTO games VALUES (1, 'Crisis Core', 'crisis-core', 'ppsspp', 1);
                 INSERT INTO games VALUES (2, 'Battle.net', 'battlenet', 'wine', 1);
                 INSERT INTO games VALUES (3, 'Not here', 'not-here', 'wine', 0);
                 INSERT INTO games VALUES (4, 'Hidden one', 'hidden-one', 'ppsspp', 1);
                 INSERT INTO games VALUES (5, 'Epic Games Store', 'epic-games-store', 'wine', 1);
                 INSERT INTO categories VALUES (1, '.hidden');
                 INSERT INTO games_categories VALUES (4, 1);",
            )
            .unwrap();
        drop(connection);
        assert_eq!(lutris_games(&database), vec![("crisis-core".to_string(), "Crisis Core".to_string())]);
        assert!(lutris_games(&dir.join("missing.db")).is_empty());
    }

    #[test]
    fn the_clients_of_the_stores_are_not_games() {
        assert!(is_store_client("battlenet", "Battle.net"));
        assert!(is_store_client("epic-games-store", "Epic Games Store"));
        assert!(is_store_client("ubisoft-connect", "Ubisoft Connect"));
        assert!(is_store_client("rockstar-games-launcher", "Rockstar Games Launcher"));
        assert!(!is_store_client("origin-of-the-species", "Origin of the Species"));
        assert!(!is_store_client("diablo-ii", "Diablo II"));
    }

    #[test]
    fn the_tools_of_steam_are_not_games() {
        assert!(is_steam_tool("1493710", "Proton Experimental"));
        assert!(is_steam_tool("1628350", "Steam Linux Runtime 3.0 (sniper)"));
        assert!(is_steam_tool("228980", "Steamworks Common Redistributables"));
        assert!(!is_steam_tool("292030", "The Witcher 3: Wild Hunt"));
    }

    use super::*;
    use crate::testing::scratch;

    #[test]
    fn an_entry_of_the_menu_is_a_game_only_when_it_says_so() {
        let dir = scratch("sgp-library", "menu");
        let game = dir.join("game.desktop");
        fs::write(
            &game,
            "[Desktop Entry]\nType=Application\nName=Un juego\nExec=/opt/juego/bin/juego %f\nIcon=juego\nCategories=Game;ActionGame;\n",
        )
        .unwrap();
        let other = dir.join("editor.desktop");
        fs::write(&other, "[Desktop Entry]\nName=Editor\nExec=/usr/bin/editor\nCategories=Development;\n").unwrap();

        let found = menu_game(&game).expect("a game");
        assert_eq!(found.name, "Un juego");
        assert_eq!(found.process.as_deref(), Some("juego"));
        assert_eq!(found.source, "menu");
        assert!(menu_game(&other).is_none());
    }

    #[test]
    fn a_hidden_entry_is_not_a_game_of_this_machine() {
        let dir = scratch("sgp-library", "hidden");
        let game = dir.join("gone.desktop");
        fs::write(&game, "[Desktop Entry]\nName=Ido\nExec=juego\nCategories=Game;\nHidden=true\n").unwrap();
        assert!(menu_game(&game).is_none());
    }

    #[test]
    fn the_manifest_of_steam_is_read_without_its_client() {
        let dir = scratch("sgp-library", "steam");
        let games = dir.join("steamapps");
        fs::create_dir_all(&games).unwrap();
        fs::write(
            games.join("appmanifest_620.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"StateFlags\"\t\t\"4\"\n}\n",
        )
        .unwrap();
        fs::write(
            games.join("appmanifest_999.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"999\"\n\t\"name\"\t\t\"Sin instalar\"\n\t\"StateFlags\"\t\t\"2\"\n}\n",
        )
        .unwrap();

        let manifest = games.join("appmanifest_620.acf");
        let game = steam_game(&dir, &manifest).expect("one installed game");
        assert_eq!(game.name, "Portal 2");
        assert_eq!(game.id, "steam:620");
        assert!(steam_game(&dir, &games.join("appmanifest_999.acf")).is_none());
    }

    #[test]
    fn a_cover_of_the_user_is_copied_where_the_panel_keeps_them() {
        let dir = scratch("sgp-library", "cover");
        let picture = dir.join("elegida.png");
        fs::write(&picture, [0x89, b'P', b'N', b'G']).unwrap();
        let saved = set_cover("steam:620", &picture).expect("copied");
        assert!(saved.exists());
        assert_eq!(cover_of("steam:620").as_deref(), Some(saved.as_path()));
        clear_cover("steam:620").unwrap();
        assert!(cover_of("steam:620").is_none());
    }

    #[test]
    fn a_picture_becomes_something_the_window_can_draw() {
        let dir = scratch("sgp-library", "data");
        let picture = dir.join("uno.png");
        fs::write(&picture, [0x89, b'P', b'N', b'G']).unwrap();
        let url = data_url(&picture).expect("a data url");
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
        let notes = dir.join("notas.txt");
        fs::write(&notes, "hola").unwrap();
        assert!(data_url(&notes).is_none());
    }
}
