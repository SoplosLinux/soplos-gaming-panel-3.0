// The choices of the user, kept in ~/.config/soplos-gaming-panel/config.json.
//
// The file is read by the window and by the part of the program with no
// window (tray, shortcuts), so every value has a default and a file that is
// missing, broken or half written never stops the program from starting.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

pub const CORNERS: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];

/// The two shapes of the overlay: the small card in a corner, and the panel of
/// the height of the screen against one edge with everything in it.
pub const HUD_MODES: [&str; 2] = ["simple", "advanced"];

/// What the overlay can draw, in the order the settings show it.
pub const HUD_METRICS: [&str; 19] = [
    "cpu", "cpuTemp", "cpuWatts", "clocks", "gpu", "gpuTemp", "vram", "ram", "fps", "frametime", "net", "disk", "fans",
    "voltage", "battery", "scheduler", "profile", "session", "clock",
];

/// How often the overlay and the monitor read the machine, in milliseconds.
pub const REFRESH_STEPS: [u32; 3] = [500, 1000, 2000];

/// The units the temperatures are shown in.
pub const TEMP_UNITS: [&str; 2] = ["c", "f"];

/// What the small overlay draws and where it sits.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Hud {
    pub enabled: bool,
    /// `simple` or `advanced`, of `HUD_MODES`.
    pub mode: String,
    pub corner: String,
    /// 20-100.
    pub opacity: u8,
    /// 50-200, as a percentage of the designed size.
    pub scale: u16,
    /// The cells to draw, of `HUD_METRICS`. Empty means the user asked for
    /// none.
    #[serde(default)]
    pub metrics: Vec<String>,
    /// Shows the overlay when a game starts and hides it when the game ends.
    pub auto_show: bool,
    /// Milliseconds between two readings, of `REFRESH_STEPS`.
    pub refresh_ms: u32,
    /// The temperature of the processor, in degrees Celsius, from which its
    /// figure is drawn in the colour of danger. 0 never.
    pub cpu_alert: u8,
    /// The same for the graphic card.
    pub gpu_alert: u8,
    // The five switches of the versions before the list. They are read only
    // to bring an old file forward, and never written again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_cpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_gpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_ram: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_temps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_fps: Option<bool>,
}

impl Hud {
    /// Keeps the cells the overlay knows, once each, and turns the switches of
    /// an older file into the list the first time that file is read.
    fn brought_forward(mut self) -> Self {
        if self.metrics.is_empty() {
            if let Some(before) = self.switches_as_metrics() {
                self.metrics = before;
            }
        }
        let mut kept: Vec<String> = Vec::new();
        for one in std::mem::take(&mut self.metrics) {
            if HUD_METRICS.contains(&one.as_str()) && !kept.contains(&one) {
                kept.push(one);
            }
        }
        self.metrics = kept;
        self.show_cpu = None;
        self.show_gpu = None;
        self.show_ram = None;
        self.show_temps = None;
        self.show_fps = None;
        self
    }

    /// The cells an old file asked for, when it had any of the switches.
    fn switches_as_metrics(&self) -> Option<Vec<String>> {
        let switches = [
            (self.show_cpu, "cpu"),
            (self.show_gpu, "gpu"),
            (self.show_ram, "ram"),
            (self.show_temps, "cpuTemp"),
            (self.show_fps, "fps"),
        ];
        if switches.iter().all(|(on, _)| on.is_none()) {
            return None;
        }
        Some(
            switches
                .into_iter()
                .filter(|(on, _)| *on == Some(true))
                .map(|(_, cell)| cell.to_string())
                .collect(),
        )
    }
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: "simple".to_string(),
            corner: "top-left".to_string(),
            opacity: 85,
            scale: 100,
            metrics: ["cpu", "cpuTemp", "gpu", "gpuTemp", "vram", "fps", "frametime"]
                .map(str::to_string)
                .to_vec(),
            auto_show: false,
            refresh_ms: 1000,
            cpu_alert: 90,
            gpu_alert: 85,
            show_cpu: None,
            show_gpu: None,
            show_ram: None,
            show_temps: None,
            show_fps: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// None follows the theme of the desktop.
    pub theme: Option<String>,
    /// None follows the language of the desktop.
    pub language: Option<String>,
    pub panel_hotkey: String,
    pub hud_hotkey: String,
    pub start_with_session: bool,
    /// When the session starts the panel, it stays in the tray instead of
    /// opening its window.
    pub start_minimized: bool,
    /// Closing the window leaves the panel in the tray; off, it quits.
    pub close_to_tray: bool,
    /// A notification of the desktop when the game mode turns on or off.
    pub notify_game_mode: bool,
    /// `c` or `f`, of `TEMP_UNITS`.
    pub temp_unit: String,
    pub hud: Hud,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: None,
            language: None,
            panel_hotkey: "Ctrl+Shift+G".to_string(),
            hud_hotkey: "Ctrl+Shift+H".to_string(),
            start_with_session: false,
            start_minimized: true,
            close_to_tray: true,
            notify_game_mode: false,
            temp_unit: "c".to_string(),
            hud: Hud::default(),
        }
    }
}

impl Settings {
    /// Keeps what comes from a file or from the window inside what the program
    /// can draw with.
    fn sanitized(mut self) -> Self {
        if let Some(theme) = &self.theme {
            if theme != "dark" && theme != "light" {
                self.theme = None;
            }
        }
        if let Some(language) = &self.language {
            if !["en", "es", "de", "fr", "it", "pt", "ro", "ru"].contains(&language.as_str()) {
                self.language = None;
            }
        }
        if self.panel_hotkey.trim().is_empty() {
            self.panel_hotkey = Settings::default().panel_hotkey;
        }
        if self.hud_hotkey.trim().is_empty() {
            self.hud_hotkey = Settings::default().hud_hotkey;
        }
        if !CORNERS.contains(&self.hud.corner.as_str()) {
            self.hud.corner = Hud::default().corner;
        }
        if !HUD_MODES.contains(&self.hud.mode.as_str()) {
            self.hud.mode = Hud::default().mode;
        }
        if !TEMP_UNITS.contains(&self.temp_unit.as_str()) {
            self.temp_unit = Settings::default().temp_unit;
        }
        self.hud = self.hud.brought_forward();
        self.hud.opacity = self.hud.opacity.clamp(20, 100);
        self.hud.scale = self.hud.scale.clamp(50, 200);
        if !REFRESH_STEPS.contains(&self.hud.refresh_ms) {
            self.hud.refresh_ms = Hud::default().refresh_ms;
        }
        // 0 is "never"; any other value is a temperature a chip can reach.
        for alert in [&mut self.hud.cpu_alert, &mut self.hud.gpu_alert] {
            if *alert != 0 {
                *alert = (*alert).clamp(50, 110);
            }
        }
        self
    }
}

/// The settings of a file chosen by the user, kept inside what the program
/// can draw with: what an import reads.
pub fn from_text(text: &str) -> Result<Settings, String> {
    serde_json::from_str::<Settings>(text).map(Settings::sanitized).map_err(|error| error.to_string())
}

pub fn config_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("soplos-gaming-panel")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load() -> Settings {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .map(Settings::sanitized)
        .unwrap_or_default()
}

/// Writes the whole file at once, so a program stopped halfway leaves the old
/// one instead of half a file.
pub fn save(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("config.json");
    let tmp = dir.join(format!("config.json.{}.tmp", std::process::id()));
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Changes only the keys of the patch (and of its "hud" object), saves the
/// result and returns the settings as they stay.
pub fn apply_patch(patch: serde_json::Value) -> Result<Settings, String> {
    let mut base = serde_json::to_value(load()).map_err(|e| e.to_string())?;
    if let (Some(base_map), Some(patch_map)) = (base.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_map {
            if key == "hud" {
                if let (Some(hud), Some(change)) =
                    (base_map.get_mut("hud").and_then(|v| v.as_object_mut()), value.as_object())
                {
                    for (hud_key, hud_value) in change {
                        hud.insert(hud_key.clone(), hud_value.clone());
                    }
                    continue;
                }
            }
            base_map.insert(key.clone(), value.clone());
        }
    }
    let settings: Settings = serde_json::from_value(base).map_err(|e| e.to_string())?;
    let settings = settings.sanitized();
    save(&settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Hud {
        serde_json::from_str::<Settings>(text).expect("valid settings").sanitized().hud
    }

    #[test]
    fn the_switches_of_an_old_file_become_the_list_of_cells() {
        let hud = read(r#"{"hud":{"showCpu":true,"showGpu":false,"showRam":true,"showTemps":false,"showFps":true}}"#);
        assert_eq!(hud.metrics, ["cpu", "ram", "fps"]);
        assert!(hud.show_cpu.is_none());
        let written = serde_json::to_string(&hud).unwrap();
        assert!(!written.contains("showCpu"), "{written}");
    }

    #[test]
    fn the_list_keeps_the_cells_that_exist_once_each() {
        let hud = read(r#"{"hud":{"metrics":["gpu","nonsense","gpu","vram"]}}"#);
        assert_eq!(hud.metrics, ["gpu", "vram"]);
    }

    #[test]
    fn an_empty_list_is_kept_empty_instead_of_filling_itself_up() {
        assert!(read(r#"{"hud":{"metrics":[]}}"#).metrics.is_empty());
    }

    #[test]
    fn a_file_without_a_list_gets_the_default_cells() {
        assert_eq!(read("{}").metrics, Hud::default().metrics);
    }

    #[test]
    fn the_new_cells_are_kept() {
        let hud = read(r#"{"hud":{"metrics":["fans","battery","clock","session"]}}"#);
        assert_eq!(hud.metrics, ["fans", "battery", "clock", "session"]);
    }

    #[test]
    fn the_refresh_and_the_alerts_stay_inside_what_makes_sense() {
        let hud = read(r#"{"hud":{"refreshMs":7,"cpuAlert":200,"gpuAlert":0}}"#);
        assert_eq!(hud.refresh_ms, 1000);
        assert_eq!(hud.cpu_alert, 110);
        assert_eq!(hud.gpu_alert, 0);
    }

    #[test]
    fn an_old_file_gets_the_new_settings_with_their_defaults() {
        let settings = from_text(r#"{"panelHotkey":"Ctrl+Shift+G","tempUnit":"kelvin"}"#).unwrap();
        assert!(settings.close_to_tray);
        assert!(settings.start_minimized);
        assert!(!settings.notify_game_mode);
        assert_eq!(settings.temp_unit, "c");
        assert_eq!(settings.hud.refresh_ms, 1000);
    }

    #[test]
    fn a_file_that_is_not_settings_is_refused() {
        assert!(from_text("not json").is_err());
    }
}
