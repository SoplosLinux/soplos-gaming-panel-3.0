// Which running process is a game.
//
// Reading /proc and guessing by name would flag a compiler or a video player.
// The honest source is the desktop files: an entry whose Categories include
// Game names the program it starts, and that is what is looked for in /proc.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub pid: u32,
    pub name: String,
}

/// Programs whose entry says Game but that are not one: this panel (and its
/// daemon, which is the same program), and the clients of the stores, which
/// stay open for hours without a game running. A game they start is found by
/// its own process.
const NOT_GAMES: [&str; 11] = [
    "soplos-gaming-panel",
    "steam",
    "steamwebhelper",
    "heroic",
    "lutris",
    "bottles",
    "itch",
    "minigalaxy",
    "legendary",
    "gogdl",
    "gamehub",
];

pub(crate) fn is_launcher(program: &str) -> bool {
    NOT_GAMES.contains(&program)
}

/// The program an `Exec=` line starts, as /proc will name it:
/// - `/usr/games/foo --fullscreen` is `foo`;
/// - `env VAR=1 foo` is `foo`;
/// - `flatpak run ... --command=retroarch org.libretro.RetroArch` is
///   `retroarch`, which is how every Flatpak exports its entries, and the name
///   its process has inside the sandbox.
///
/// A shell gives nothing: the program is hidden in its script.
pub(crate) fn exec_program(exec: &str) -> Option<String> {
    let words: Vec<&str> = exec.split_whitespace().map(|word| word.trim_matches('"')).collect();
    let name = |word: &str| Path::new(word).file_name().map(|n| n.to_string_lossy().to_string());
    let mut index = 0;
    if words.first().and_then(|w| name(w)).as_deref() == Some("env") {
        index += 1;
        while words.get(index).is_some_and(|word| word.contains('=') && !word.starts_with('-')) {
            index += 1;
        }
    }
    let program = name(words.get(index)?)?;
    if program == "flatpak" {
        return words
            .iter()
            .find_map(|word| word.strip_prefix("--command="))
            .and_then(name)
            .filter(|command| !command.is_empty());
    }
    (!program.is_empty() && !matches!(program.as_str(), "sh" | "bash" | "env")).then_some(program)
}

/// The programs of the games installed for this machine: apt, Flatpak and
/// AppImage all end up as a desktop entry somewhere in these folders.
fn game_programs() -> HashSet<String> {
    let mut programs = HashSet::new();
    for dir in desktop_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(name) = program_of(&path).filter(|name| !is_launcher(name)) {
                programs.insert(name);
            }
        }
    }
    programs
}

/// The entries of the system and of the user running this program: the
/// library of the window shows the games of whoever opens it, not those of the
/// other people of the computer.
pub(crate) fn user_desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/usr/share/applications"), PathBuf::from("/var/lib/flatpak/exports/share/applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(Path::new(&home).join(".local/share/applications"));
        dirs.push(Path::new(&home).join(".local/share/flatpak/exports/share/applications"));
    }
    dirs
}

pub(crate) fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = user_desktop_dirs();
    // The daemon runs as root, so its HOME is not the one of the player: the
    // entries of every user are read as well.
    if let Ok(entries) = fs::read_dir("/home") {
        for entry in entries.flatten() {
            let home = entry.path();
            dirs.push(home.join(".local/share/applications"));
            dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
        }
    }
    dirs
}

/// The name of the executable a game entry starts, or nothing when the entry
/// is not a game.
fn program_of(entry: &Path) -> Option<String> {
    let text = fs::read_to_string(entry).ok()?;
    let mut is_game = false;
    let mut command = None;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        // Only the main group: the actions of an entry ([Desktop Action ...])
        // have an Exec of their own.
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Categories=") {
            is_game = value.split(';').any(|category| category == "Game");
        } else if let Some(value) = line.strip_prefix("Exec=") {
            command = exec_program(value);
        }
    }
    if is_game {
        command
    } else {
        None
    }
}

/// The app of Steam a process runs, when it is the `reaper` Steam starts every
/// game under: `reaper SteamLaunch AppId=570 -- ...`. Its arguments come
/// separated by zero bytes, as /proc keeps them.
fn steam_app_of(cmdline: &[u8]) -> Option<String> {
    let mut args = cmdline.split(|byte| *byte == 0).map(|arg| String::from_utf8_lossy(arg).to_string());
    let first = args.next()?;
    if Path::new(&first).file_name()?.to_string_lossy() != "reaper" {
        return None;
    }
    let rest: Vec<String> = args.take_while(|arg| arg != "--").collect();
    if !rest.iter().any(|arg| arg == "SteamLaunch") {
        return None;
    }
    rest.iter()
        .find_map(|arg| arg.strip_prefix("AppId="))
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        // The tools of Steam (Proton, the runtime) have no AppId of a game.
        .filter(|id| *id != "0")
        .map(str::to_string)
}

/// The name of a game of Steam, read from its manifest in the library of any
/// user; the AppId when no manifest names it.
fn steam_name(app_id: &str) -> String {
    let mut homes: Vec<PathBuf> = std::env::var("HOME").ok().filter(|home| !home.is_empty()).map(PathBuf::from).into_iter().collect();
    if let Ok(entries) = fs::read_dir("/home") {
        homes.extend(entries.flatten().map(|entry| entry.path()));
    }
    for home in homes {
        for root in crate::library::steam_roots_in(&home) {
            for folder in crate::library::steam_folders(&root) {
                let manifest = folder.join("steamapps").join(format!("appmanifest_{app_id}.acf"));
                if let Some(name) = fs::read_to_string(manifest).ok().and_then(|text| crate::library::acf_value(&text, "name")) {
                    return name;
                }
            }
        }
    }
    format!("Steam {app_id}")
}

/// The games running right now, leaving out the names the user excluded. The
/// names added by hand count as games even when they have no desktop entry.
pub fn running(exclusions: &[String], manual: &[String]) -> Vec<Game> {
    let mut programs = game_programs();
    programs.extend(manual.iter().cloned());
    let Ok(entries) = fs::read_dir("/proc") else { return Vec::new() };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else { continue };
        // A game of Steam: its own binary is not in any desktop entry, but the
        // reaper that starts it names the app.
        if let Some(app_id) = fs::read(entry.path().join("cmdline")).ok().as_deref().and_then(steam_app_of) {
            let game = steam_name(&app_id);
            if !exclusions.iter().any(|excluded| excluded == &game) {
                found.push(Game { pid, name: game });
            }
            continue;
        }
        let Ok(command) = fs::read_link(entry.path().join("exe")) else { continue };
        let Some(program) = command.file_name().map(|value| value.to_string_lossy().to_string()) else { continue };
        if !programs.contains(&program) || exclusions.iter().any(|excluded| excluded == &program) {
            continue;
        }
        found.push(Game { pid, name: program });
    }
    found.sort_by_key(|game| game.pid);
    found.dedup_by(|one, other| one.name == other.name);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str, body: &str) -> (crate::testing::Scratch, PathBuf) {
        let dir = crate::testing::scratch("sgp-games", name);
        let file = dir.join("game.desktop");
        fs::write(&file, body).unwrap();
        (dir, file)
    }

    #[test]
    fn a_game_entry_names_its_program() {
        let (_dir, file) = temp("game", "[Desktop Entry]\nName=Game\nCategories=Game;ActionGame;\nExec=/usr/games/foo --fullscreen\n");
        assert_eq!(program_of(&file), Some("foo".to_string()));
    }

    #[test]
    fn an_entry_that_is_not_a_game_is_left_out() {
        let (_dir, file) = temp("editor", "[Desktop Entry]\nName=Editor\nCategories=Utility;\nExec=editor\n");
        assert_eq!(program_of(&file), None);
    }

    #[test]
    fn a_flatpak_without_a_command_names_nothing() {
        let (_dir, file) = temp("flatpak", "[Desktop Entry]\nCategories=Game;\nExec=flatpak run org.example.Game\n");
        assert_eq!(program_of(&file), None);
    }

    #[test]
    fn a_flatpak_game_is_named_by_its_command() {
        // How Flatpak exports the entry of an emulator.
        let (_dir, file) = temp(
            "retroarch",
            "[Desktop Entry]\nName=RetroArch\nCategories=Game;Emulator;\nExec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=retroarch --file-forwarding org.libretro.RetroArch @@u %U @@\n",
        );
        assert_eq!(program_of(&file), Some("retroarch".to_string()));
    }

    #[test]
    fn the_variables_before_a_program_are_skipped() {
        assert_eq!(exec_program("env DXVK_HUD=1 MANGOHUD=1 /opt/game/run --x"), Some("run".to_string()));
        assert_eq!(exec_program("sh -c 'cd /x && ./game'"), None);
        assert_eq!(exec_program("\"/usr/games/foo\" %U"), Some("foo".to_string()));
    }

    #[test]
    fn the_exec_of_an_action_is_not_the_one_of_the_game() {
        let (_dir, file) = temp(
            "actions",
            "[Desktop Entry]\nCategories=Game;\nExec=foo\nActions=config;\n\n[Desktop Action config]\nExec=foo-config\n",
        );
        assert_eq!(program_of(&file), Some("foo".to_string()));
    }

    #[test]
    fn this_panel_and_the_store_clients_are_not_games() {
        // The entry of the panel says Game, and so does the one of Steam.
        let (_dir, file) = temp("panel", "[Desktop Entry]\nCategories=Utility;Game;\nExec=soplos-gaming-panel\n");
        assert_eq!(program_of(&file).filter(|name| !is_launcher(name)), None);
        let (_dir, file) = temp("steam", "[Desktop Entry]\nCategories=Network;FileTransfer;Game;\nExec=/usr/games/steam %U\n");
        assert_eq!(program_of(&file).filter(|name| !is_launcher(name)), None);
        assert!(!is_launcher("foo"));
    }

    #[test]
    fn a_game_of_steam_is_found_by_its_reaper() {
        let line = b"/home/u/.local/share/Steam/ubuntu12_32/reaper\0SteamLaunch\0AppId=570\0--\0/home/u/game\0";
        assert_eq!(steam_app_of(line).as_deref(), Some("570"));
        // The client itself, a tool of Steam and any other program are not.
        assert_eq!(steam_app_of(b"/home/u/.local/share/Steam/ubuntu12_32/steam\0-silent\0"), None);
        assert_eq!(steam_app_of(b"reaper\0SteamLaunch\0AppId=0\0--\0x\0"), None);
        assert_eq!(steam_app_of(b"reaper\0AppId=570\0"), None);
        // An AppId after the "--" belongs to the game, not to the reaper.
        assert_eq!(steam_app_of(b"reaper\0SteamLaunch\0--\0AppId=570\0"), None);
    }

    #[test]
    fn nothing_running_matches_no_game() {
        // Whatever is running on the build machine, an exclusion of every
        // program cannot return anything.
        let games = running(&["a".to_string(), "b".to_string()], &[]);
        assert!(games.iter().all(|game| game.name != "a" && game.name != "b"));
    }
}
