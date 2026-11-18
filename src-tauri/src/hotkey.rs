// The two shortcuts of the panel. On X11 the plugin of Tauri hears the keys
// itself. On Wayland no program may listen to the keyboard, so the
// combination is written in the desktop, which starts the program again with
// `--toggle` or `--hud-toggle`.

use std::process::Command;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Clone, Copy)]
pub enum Action {
    Toggle,
    Hud,
}

// The combinations this program holds right now, so a change only lets go of
// the ones that go away and never takes the same one twice.
static HELD: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase()
}

fn command_for(action: Action) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "soplos-gaming-panel".to_string());
    format!("{exe} {}", match action {
        Action::Toggle => "--toggle",
        Action::Hud => "--hud-toggle",
    })
}

/// Removes the shortcuts this program wrote into the desktop of XFCE, so an
/// old entry cannot be launched by the desktop on top of the combination that
/// this session holds in memory.
fn forget_desktop_shortcuts(wanted: &[(String, Action)]) {
    if !desktop().contains("xfce") {
        return;
    }
    for (combo, _) in wanted {
        let property = format!("/commands/custom/{}", xfce_binding(combo));
        let _ = Command::new("xfconf-query")
            .args(["-c", "xfce4-keyboard-shortcuts", "-p", &property, "-r"])
            .output();
    }
}

fn suffix(action: Action) -> &'static str {    match action {
        Action::Toggle => "toggle",
        Action::Hud => "hud",
    }
}

fn error_of(result: &std::process::Output) -> String {
    let said = String::from_utf8_lossy(&result.stderr).trim().to_string();
    if said.is_empty() {
        "The desktop did not take the combination".to_string()
    } else {
        said
    }
}

fn run_checked(program: &str, args: &[&str]) -> Result<(), String> {
    let result = Command::new(program).args(args).output().map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        Err(error_of(&result))
    }
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    result.status.success().then(|| String::from_utf8_lossy(&result.stdout).trim().to_string())
}

/// "Ctrl+Shift+G" as GNOME writes it: "<Control><Shift>g".
fn gnome_binding(combo: &str) -> String {
    let mut out = String::new();
    let parts: Vec<&str> = combo.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => out.push_str("<Control>"),
            "shift" => out.push_str("<Shift>"),
            "alt" | "mod1" => out.push_str("<Alt>"),
            "super" | "meta" | "win" => out.push_str("<Super>"),
            other => out.push_str(other),
        }
    }
    out
}

/// "Ctrl+Shift+G" as Xfce writes it: "<Primary><Shift>g".
fn xfce_binding(combo: &str) -> String {
    let parts: Vec<&str> = combo.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut out = String::new();
    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "primary" => out.push_str("<Primary>"),
            "shift" => out.push_str("<Shift>"),
            "alt" | "mod1" => out.push_str("<Alt>"),
            "super" | "meta" | "win" => out.push_str("<Super>"),
            other => out.push_str(other),
        }
    }
    out
}

fn gnome(combo: &str, action: Action, command: &str) -> Result<(), String> {
    let schema = "org.gnome.settings-daemon.plugins.media-keys";
    let path = format!(
        "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/soplos-gaming-panel-{}/",
        suffix(action)
    );

    // The list of custom shortcuts has to hold this one before it is set.
    let current = run("gsettings", &["get", schema, "custom-keybindings"]).unwrap_or_else(|| "[]".to_string());
    if !current.contains(&path) {
        let inner = current.trim().trim_start_matches('[').trim_end_matches(']').trim().trim_end_matches(',').trim();
        let entry = format!("'{}'", path);
        let list = if inner.is_empty() { format!("[{entry}]") } else { format!("[{inner}, {entry}]") };
        run_checked("gsettings", &["set", schema, "custom-keybindings", &list])?;
    }

    let key = format!("org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{path}");
    run_checked("gsettings", &["set", &key, "name", &format!("Soplos Gaming Panel ({})", suffix(action))])?;
    run_checked("gsettings", &["set", &key, "command", command])?;
    run_checked("gsettings", &["set", &key, "binding", &gnome_binding(combo)])
}

fn xfce(combo: &str, command: &str) -> Result<(), String> {
    let property = format!("/commands/custom/{}", xfce_binding(combo));
    // -n creates the key of the shortcut; on a later run it is already there
    // and the key is only given its new value.
    let created = Command::new("xfconf-query")
        .args(["-c", "xfce4-keyboard-shortcuts", "-p", &property, "-n", "-t", "string", "-s", command])
        .output();
    match created {
        Ok(result) if result.status.success() => Ok(()),
        _ => run_checked(
            "xfconf-query",
            &["-c", "xfce4-keyboard-shortcuts", "-p", &property, "-s", command],
        ),
    }
}

/// "Ctrl+Shift+G" as KDE writes it: the same, with Meta for the Super key.
fn kde_binding(combo: &str) -> String {
    combo
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|part| match part.to_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" | "mod1" => "Alt".to_string(),
            "super" | "meta" | "win" => "Meta".to_string(),
            _ => part.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Plasma on Wayland keeps the global shortcuts in KWin, and the way a program
/// that is not a KDE one gets one is the shortcut of a launcher: a hidden entry
/// of the menu that starts the program with `--toggle` or `--hud-toggle`, and
/// its combination in kglobalshortcutsrc, as System Settings writes it.
fn kde(combo: &str, action: Action, command: &str) -> Result<(), String> {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty()).ok_or("The home folder is not known")?;
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(&home).join(".local/share"));
    let name = format!("soplos-gaming-panel-{}.desktop", suffix(action));
    let dir = data.join("applications");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let binding = kde_binding(combo);
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Soplos Gaming Panel ({})\nExec={command}\nIcon=soplos-gaming-panel\nTerminal=false\nNoDisplay=true\nX-KDE-Shortcuts={binding}\n",
        suffix(action)
    );
    std::fs::write(dir.join(&name), entry).map_err(|e| e.to_string())?;

    let writer = if run("which", &["kwriteconfig6"]).is_some() { "kwriteconfig6" } else { "kwriteconfig5" };
    run_checked(
        writer,
        &["--file", "kglobalshortcutsrc", "--group", "services", "--group", &name, "--key", "_launch", &binding],
    )?;
    // The menu of KDE is a cache: the new entry is read after it is rebuilt.
    let _ = Command::new("kbuildsycoca6").output();
    Ok(())
}

fn register_in_desktop(combo: &str, action: Action) -> Result<(), String> {
    let de = desktop();
    let command = command_for(action);
    if de.contains("gnome") || de.contains("unity") || de.contains("pantheon") {
        gnome(combo, action, &command)
    } else if de.contains("xfce") {
        xfce(combo, &command)
    } else if de.contains("kde") || de.contains("plasma") {
        kde(combo, action, &command)
    } else {
        Err(format!(
            "The desktop «{de}» is not known, so the shortcut cannot be registered. The tray still opens the panel."
        ))
    }
}

fn register_in_app(app: &AppHandle, combo: &str, action: Action) -> Result<(), String> {
    let shortcut = Shortcut::from_str(combo).map_err(|_| format!("Not a valid combination: {combo}"))?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let handle = handle.clone();
                let inner = handle.clone();
                let _ = handle.run_on_main_thread(move || match action {
                    Action::Toggle => crate::toggle_window(&inner),
                    Action::Hud => crate::toggle_hud(&inner),
                });
            }
        })
        .map_err(|e| format!("{combo}: {e}"))
}

/// On X11 the combinations already held are left alone: taking the same one
/// again is an error at the level of the window system, not a change.
fn register_on_x11(app: &AppHandle, wanted: &[(String, Action)]) -> Vec<String> {
    let mut notes = Vec::new();
    // A shortcut written into the desktop in an earlier Wayland session stays
    // there and is tried on every key press, pointing at a program that may no
    // longer be in the same place. On X11 the combination is held here, so
    // those leftovers are removed.
    forget_desktop_shortcuts(wanted);
    let held_now: Vec<String> = wanted.iter().map(|(combo, _)| combo.clone()).collect();

    let mut held = match HELD.lock() {
        Ok(held) => held,
        Err(_) => return vec!["The shortcuts cannot be kept in memory".to_string()],
    };

    // The combinations that are no longer wanted are let go.
    for combo in held.iter().filter(|combo| !held_now.contains(combo)) {
        if let Ok(shortcut) = Shortcut::from_str(combo) {
            let _ = app.global_shortcut().unregister(shortcut);
        }
    }
    held.retain(|combo| held_now.contains(combo));

    for (combo, action) in wanted {
        if held.contains(combo) {
            continue;
        }
        match register_in_app(app, combo, *action) {
            Ok(()) => held.push(combo.clone()),
            Err(e) => notes.push(e),
        }
    }
    notes
}

/// Registers the two shortcuts of the settings. Returns what the user has to
/// know when the desktop did not take one of them.
pub fn register_all(app: &AppHandle) -> Result<String, String> {
    let settings = crate::config::load();
    let wanted = [(settings.panel_hotkey.clone(), Action::Toggle), (settings.hud_hotkey.clone(), Action::Hud)];

    let mut notes = Vec::new();
    if is_wayland() {
        for (combo, action) in &wanted {
            if let Err(e) = register_in_desktop(combo, *action) {
                notes.push(e);
            }
        }
    } else {
        notes.extend(register_on_x11(app, &wanted));
    }
    Ok(notes.join(" "))
}
