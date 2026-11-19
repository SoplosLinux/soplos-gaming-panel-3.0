mod autostart;
mod commands;
mod config;
mod daemon;
mod environment;
mod game_watch;
mod governor;
mod hotkey;
mod gamemode;
mod i18n;
#[cfg(target_os = "linux")]
mod layer;
mod library;
mod mangohud;
mod monitor;
mod system_theme;
#[cfg(test)]
mod testing;
mod tray;
mod tweaks;

use tauri::{AppHandle, Emitter, Manager};

/// Privileged mode: pkexec starts the program again as `--run-ops <file>`
/// (with password) or `--run-tweaks <file>` (without) and main() hands over
/// here before any window exists.
pub fn run_ops_file(path: &str, with_password: bool) -> i32 {
    commands::privileged::run_ops_file(path, with_password)
}

/// Service mode: the governor, with no window, as the system service of the
/// package. It answers the window over a socket in /run.
pub fn run_daemon() -> i32 {
    daemon::serve()
}

/// The ExecStopPost of the unit: puts back whatever the governor had changed,
/// even when the daemon was killed or crashed.
pub fn run_daemon_restore() -> i32 {
    daemon::restore_and_report()
}

/// Where this program is, to start it again (pkexec, the shortcuts, the start
/// with the session). After an upgrade the program that is still running was
/// replaced on the disk, and the kernel names it "<path> (deleted)": that path
/// does not exist, so the mark is taken away to point at the new program.
pub fn program_path() -> String {
    let path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let path = path.trim_end_matches(" (deleted)").to_string();
    if path.is_empty() {
        "/usr/bin/soplos-gaming-panel".to_string()
    } else {
        path
    }
}

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The tray button and the shortcut: shown becomes hidden and the other way.
pub fn toggle_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let visible = window.is_visible().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    if visible && !minimized {
        let _ = window.hide();
    } else {
        show_window(app);
    }
}

/// The overlay: a window of its own, without frame, over everything, that
/// the tray and the shortcut open and close. It is created the first time and
/// kept afterwards, so opening it again is immediate.
fn overlay(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("overlay") {
        return Some(window);
    }
    let window = tauri::WebviewWindowBuilder::new(app, "overlay", tauri::WebviewUrl::App("overlay.html".into()))
        // A title of its own, so the properties of this window can be set by
        // the window manager tools without touching the window of the panel.
        .title("soplos-gaming-overlay")
        .inner_size(360.0, 300.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible_on_all_workspaces(true)
        .visible(false)
        .build()
        .ok()?;
    // Before it is on screen, which is when the desktop decides what to do
    // with it: on Wayland it becomes a widget of the desktop (layer-shell), on
    // X11 it is marked as a panel. Both touch GTK, so on its main thread.
    let prepared = window.clone();
    let _ = window.run_on_main_thread(move || {
        #[cfg(target_os = "linux")]
        let _ = layer::make_widget(&prepared);
        mark_as_panel(&prepared);
        place_overlay(&prepared);
        let _ = prepared.show();
    });
    keep_on_top(window.clone());
    Some(window)
}

/// What keeps the overlay out of the tiling and in its corner.
///
/// The tiling of a desktop decides what to do with a window the moment it
/// appears: Pop Shell, Bismuth and Polonium on GNOME, and Polonium and
/// Karousel on Plasma, take any window and give it a slot. A window of the
/// type "dock" is not a window for them but a panel, and they leave it where
/// it is. On X11 that type is a property of the window, written here with
/// xprop; Plasma is also told with a rule of its own window manager. On
/// Wayland the property cannot be written by the program, and only the rule
/// of the window manager helps.
fn mark_as_panel(_window: &tauri::WebviewWindow) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if environment::desktop_environment() == "gnome" {
            ensure_gnome_extension();
            ensure_forge_float();
        } else {
            ensure_kwin_rule();
        }
        return;
    }
    let Ok(found) = std::process::Command::new("xdotool")
        .args(["search", "--name", "soplos-gaming-overlay"])
        .output()
    else {
        return;
    };
    let stdout = String::from_utf8_lossy(&found.stdout);
    for id in stdout.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let _ = std::process::Command::new("xprop")
            .args([
                "-id",
                id,
                "-f",
                "_NET_WM_WINDOW_TYPE",
                "32a",
                "-set",
                "_NET_WM_WINDOW_TYPE",
                "_NET_WM_WINDOW_TYPE_DOCK",
            ])
            .output();
        let _ = std::process::Command::new("xprop")
            .args([
                "-id",
                id,
                "-f",
                "_NET_WM_STATE",
                "32a",
                "-set",
                "_NET_WM_STATE",
                "_NET_WM_STATE_ABOVE,_NET_WM_STATE_STICKY,_NET_WM_STATE_SKIP_TASKBAR,_NET_WM_STATE_SKIP_PAGER",
            ])
            .output();
        let _ = std::process::Command::new("wmctrl")
            .args(["-i", "-r", id, "-b", "add,above,sticky,skip_taskbar,skip_pager"])
            .output();
    }
    ensure_kwin_rule();
}

/// The extension of the package that keeps the overlay in its place on GNOME,
/// where the Shell is the window manager. It is installed for the whole
/// system and turned on here for the user the first time the overlay shows.
const GNOME_EXTENSION: &str = "soplos-gaming-overlay@soplos.org";

fn ensure_gnome_extension() {
    let Ok(info) = std::process::Command::new("gnome-extensions").args(["info", GNOME_EXTENSION]).output() else { return };
    let text = String::from_utf8_lossy(&info.stdout).to_lowercase();
    // "State: ACTIVE" once it runs; anything else is turned on.
    if !text.contains("active") || text.contains("inactive") {
        let _ = std::process::Command::new("gnome-extensions").args(["enable", GNOME_EXTENSION]).output();
    }
}

/// Forge, the tiling of GNOME, takes every window into its tiles unless its
/// list of exceptions says the window floats. The overlay is added to the list
/// of the user, the one Forge itself keeps, when Forge is there.
fn ensure_forge_float() {
    let config = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|home| std::path::PathBuf::from(home).join(".config")));
    let Some(file) = config.map(|dir| dir.join("forge/config/windows.json")) else { return };
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let Ok(mut windows) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let Some(overrides) = windows.get_mut("overrides").and_then(|list| list.as_array_mut()) else { return };
    let present = overrides
        .iter()
        .any(|rule| rule.get("wmTitle").and_then(|title| title.as_str()) == Some("soplos-gaming-overlay"));
    if present {
        return;
    }
    overrides.push(serde_json::json!({
        "wmClass": "soplos-gaming-panel",
        "wmTitle": "soplos-gaming-overlay",
        "mode": "float"
    }));
    if let Ok(body) = serde_json::to_string_pretty(&windows) {
        let _ = std::fs::write(&file, body);
    }
}

/// Plasma keeps the state of every window in rules of its own, and a rule of
/// this program says the overlay is above, without border and out of the task
/// bar. It is written once and survives a restart of the session.
fn ensure_kwin_rule() {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if !desktop.contains("kde") && !desktop.contains("plasma") {
        return;
    }
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            std::path::PathBuf::from(home).join(".config")
        });
    if !config_dir.exists() {
        return;
    }
    let rules = config_dir.join("kwinrulesrc");
    let content = std::fs::read_to_string(&rules).unwrap_or_default();
    if content.contains("[soplos-gaming-overlay]") {
        return;
    }
    let rule = r#"
[soplos-gaming-overlay]
Description=Soplos Gaming Panel overlay
above=true
aboverule=2
noborder=true
noborderrule=2
skiptaskbar=true
skiptaskbarrule=2
skippager=true
skippagerrule=2
placement=0
placementrule=2
title=soplos-gaming-overlay
titlematch=1
"#;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&rules) {
        use std::io::Write;
        let _ = file.write_all(rule.as_bytes());
        for dbus in ["qdbus6", "qdbus"] {
            if std::path::Path::new(&format!("/usr/bin/{dbus}")).exists() {
                let _ = std::process::Command::new(dbus)
                    .args(["org.kde.KWin", "/KWin", "reconfigure"])
                    .status();
                break;
            }
        }
    }
}

/// Some window managers drop the "above" flag when the window is shown again
/// or when another program asks for the front, so it is asked for again while
/// the overlay is up. It is the most a plain window can do; a game drawing its
/// own fullscreen surface is a separate problem, with its own backends.
fn keep_on_top(window: tauri::WebviewWindow) {
    std::thread::spawn(move || loop {
        // A widget of the desktop is on its layer by itself.
        #[cfg(target_os = "linux")]
        if layer::active() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(4));
        if window.is_visible().unwrap_or(false) {
            let _ = window.set_always_on_top(true);
            let _ = window.set_visible_on_all_workspaces(true);
        }
    });
}

/// The corner of the screen the overlay asks for, out of the way of what a
/// game draws in the middle, and the size its own settings ask for. It is a
/// panel of the height of the work area, against the edge of the screen, the
/// same shape as the assistant of ianox: long, narrow, out of the way of the
/// window that is being used.
fn place_overlay(window: &tauri::WebviewWindow) {
    let hud = config::load().hud;
    // A widget of the desktop is placed with anchors and margins; GTK is
    // touched on its main thread.
    #[cfg(target_os = "linux")]
    if layer::active() {
        let widget = window.clone();
        let _ = window.run_on_main_thread(move || layer::place(&widget, &hud.mode, &hud.corner, hud.scale));
        return;
    }
    let factor = f64::from(hud.scale.clamp(50, 200)) / 100.0;
    let (area_x, area_y, area_width, area_height) = work_area(window);

    if hud.mode == "advanced" {
        // The panel of the height of the screen, against the edge: long and
        // narrow, out of the way of the window that is being used.
        let width = (420.0 * factor).min(area_width);
        let x = if hud.corner.ends_with("right") {
            area_x + area_width - width
        } else {
            area_x
        };
        let _ = window.set_size(tauri::LogicalSize::new(width, area_height));
        let _ = window.set_position(tauri::LogicalPosition::new(x, area_y));
        return;
    }

    // The small card in the corner the settings ask for.
    let width = 360.0 * factor;
    let height = 300.0 * factor;
    let margin = 24.0;
    let left = area_x + margin;
    let right = area_x + area_width - width - margin;
    let top = area_y + margin;
    let bottom = area_y + area_height - height - margin;
    let x = if hud.corner.ends_with("right") { right } else { left };
    let y = if hud.corner.starts_with("bottom") { bottom } else { top };
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.set_position(tauri::LogicalPosition::new(
        x.max(area_x),
        y.max(area_y),
    ));
}

/// The part of the screen the desktop leaves free. On X11 it is asked to the
/// window manager, so the bar and the dock do not cover the panel; without an
/// answer, the whole monitor is used.
fn work_area(window: &tauri::WebviewWindow) -> (f64, f64, f64, f64) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Ok(result) = std::process::Command::new("xprop")
            .args(["-root", "-notype", "_NET_WORKAREA"])
            .output()
        {
            let text = String::from_utf8_lossy(&result.stdout);
            if let Some(values) = text.split('=').nth(1) {
                let numbers: Vec<f64> = values
                    .split(',')
                    .filter_map(|number| number.trim().parse::<f64>().ok())
                    .collect();
                if numbers.len() >= 4 && numbers[2] > 0.0 && numbers[3] > 0.0 {
                    return (numbers[0], numbers[1], numbers[2], numbers[3]);
                }
            }
        }
    }
    match window.current_monitor() {
        Ok(Some(monitor)) => {
            let factor = monitor.scale_factor();
            let size = monitor.size().to_logical::<f64>(factor);
            let position = monitor.position().to_logical::<f64>(factor);
            (position.x, position.y, size.width, size.height)
        }
        _ => (0.0, 0.0, 1920.0, 1080.0),
    }
}

/// The window of the overlay puts itself where the settings say, after they
/// have been changed.
pub fn place_hud(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        place_overlay(&window);
    }
}

/// Shown becomes hidden and the other way, and the choice is kept so the
/// window and the tray say the same.
pub fn toggle_hud(app: &AppHandle) {
    let shown = app
        .get_webview_window("overlay")
        .map(|window| window.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if shown {
        hide_hud(app);
    } else {
        show_hud(app);
    }
}

/// Puts the overlay on screen and remembers that it is up. The switch of the
/// settings, the tray and the shortcut all come through here, so the file and
/// the screen never disagree.
pub fn show_hud(app: &AppHandle) {
    present_hud(app);
    remember_hud(app, true);
}

/// Takes the overlay off the screen and remembers that it is down.
pub fn hide_hud(app: &AppHandle) {
    withdraw_hud(app);
    remember_hud(app, false);
}

/// Puts the overlay on screen without touching the choice of the user: what
/// a game that starts does when the overlay is set to come with the game.
pub fn present_hud(app: &AppHandle) {
    let Some(window) = overlay(app) else { return };
    mark_as_panel(&window);
    place_overlay(&window);
    let _ = window.show();
    let _ = window.set_always_on_top(true);
}

/// Takes the overlay off the screen without touching the choice of the user.
pub fn withdraw_hud(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

fn remember_hud(app: &AppHandle, enabled: bool) {
    let mut settings = config::load();
    settings.hud.enabled = enabled;
    if config::save(&settings).is_ok() {
        tray::refresh_hud_item(app, enabled);
        let _ = app.emit("hud-enabled", enabled);
    }
}

/// What a second launch asked for with no window to say it to.
fn apply_action(app: &AppHandle, action: &str) {
    match action {
        "toggle" => toggle_window(app),
        "show" => show_window(app),
        "hud-toggle" => toggle_hud(app),
        _ => {}
    }
}

fn action_in(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .find_map(|arg| match arg.as_str() {
            "--toggle" => Some("toggle".to_string()),
            "--show" => Some("show".to_string()),
            "--hud-toggle" => Some("hud-toggle".to_string()),
            _ => None,
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let launched_as = action_in(&std::env::args().collect::<Vec<_>>());

    tauri::Builder::default()
        // The single instance plugin goes first: the program lives in the tray,
        // so a second launch only has to show or hide the one that is open.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(action) = action_in(&args) {
                apply_action(app, &action);
            } else {
                show_window(app);
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            system_theme::get_system_theme,
            environment::get_environment_info,
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::config_path,
            commands::settings::apply_hotkeys,
            commands::settings::set_hud_enabled,
            commands::settings::settings_export,
            commands::settings::settings_import,
            commands::settings::settings_reset,
            commands::governor::history_clear,
            commands::monitor::monitor_snapshot,
            commands::governor::governor_status,
            commands::governor::governor_catalogue,
            commands::governor::governor_apply,
            commands::governor::governor_restore,
            commands::governor::governor_auto,
            commands::governor::governor_exclusion,
            commands::governor::governor_rule,
            commands::governor::scx_status,
            commands::governor::scx_set,
            commands::governor::governor_manual,
            commands::governor::kernels_status,
            commands::service::governor_service,
            commands::service::governor_service_control,
            commands::service::gamemode_state,
            commands::gamemode_ini::gamemode_ini_status,
            commands::gamemode_ini::gamemode_ini_set,
            commands::gamemode_ini::gamemode_ini_remove,
            commands::gamemode_ini::gamemode_ini_text,
            commands::mangohud::mangohud_status,
            commands::mangohud::mangohud_set,
            commands::mangohud::mangohud_remove,
            commands::mangohud::mangohud_preset,
            commands::mangohud::mangohud_text,
            commands::mangohud::mangohud_fps_log,
            commands::mangohud::mangohud_fps_log_off,
            commands::library::library_list,
            commands::library::library_launch,
            commands::library::library_set_cover,
            commands::library::library_clear_cover,
            commands::privileged::install_gaming_tools,
            commands::privileged::install_packages,
            commands::privileged::tweak_set,
            commands::privileged::tweaks_revert_all,
            commands::privileged::tweaks_apply_gaming,
            commands::privileged::install_overlay_tools,
            commands::tweaks::tweaks_status,
            commands::tweaks::overlay_tools_installed,
            commands::tweaks::ryzenadj_installed,
            commands::tweaks::sensor_modules,
            commands::tweaks::ask_sensor_modules,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::setup(&handle)?;
            let _ = hotkey::register_all(&handle);
            // The readings of the machine go on while the program runs, so the
            // overlay and the window show the latest one at once.
            monitor::start(handle.clone());
            // Notifications and the overlay that comes with the game.
            game_watch::start(handle.clone());
            // An entry of the session written by an older version has no
            // flag to start in the tray: it is written again.
            if autostart::is_enabled() {
                let _ = autostart::set(true);
            }
            // A launch from the shortcut of the desktop opens the panel at once.
            if let Some(action) = &launched_as {
                apply_action(&handle, action);
            } else {
                // The window is created hidden: the session can start the
                // panel straight into the tray, when the user asked for that.
                let autostarted = std::env::args().any(|arg| arg == autostart::FLAG);
                if !(autostarted && config::load().start_minimized) {
                    show_window(&handle);
                }
            }
            Ok(())
        })
        // The panel lives in the tray: closing the window only hides it, and
        // the way out is the tray menu, unless the user asked for the window
        // to close the program.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !config::load().close_to_tray {
                    window.app_handle().exit(0);
                    return;
                }
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Soplos Gaming Panel");
}
