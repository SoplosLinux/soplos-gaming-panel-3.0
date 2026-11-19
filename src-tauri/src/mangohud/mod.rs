// The MangoHud configuration file, read and written without losing anything.
//
// The file belongs to the user and is full of comments they wrote: every line
// is kept as it is, and a change only touches the line of the key it changes.
// Nothing is rewritten from a model of what the file "should" be.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One line of the file: a key with its value, a key alone, or anything else
/// (comment, blank line) kept verbatim.
///
/// A key alone is how MangoHud turns an option on: it reads `gpu_stats` as
/// `gpu_stats=1`. `gpu_stats=` is not the same thing, MangoHud reads the empty
/// value as 0 and turns the option off, so an option on is always written alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Raw(String),
    Flag(String),
    Pair { key: String, value: String },
}

/// The name of a key, as MangoHud and the panel accept it.
fn is_key(text: &str) -> bool {
    !text.is_empty() && text.len() <= 64 && text.chars().all(|one| one.is_ascii_alphanumeric() || one == '_')
}

impl Line {
    fn key(&self) -> Option<&str> {
        match self {
            Line::Flag(key) | Line::Pair { key, .. } => Some(key),
            Line::Raw(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub lines: Vec<Line>,
}

impl Document {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return Line::Raw(line.to_string());
                }
                match trimmed.split_once('=') {
                    Some((key, value)) if !key.trim().is_empty() && !key.contains('#') && !key.contains(char::is_whitespace) => {
                        Line::Pair { key: key.trim().to_string(), value: value.trim().to_string() }
                    }
                    None if is_key(trimmed) => Line::Flag(trimmed.to_string()),
                    _ => Line::Raw(line.to_string()),
                }
            })
            .collect();
        Self { lines }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            match line {
                Line::Raw(text) => out.push_str(text),
                Line::Flag(key) => out.push_str(key),
                Line::Pair { key, value } => {
                    let _ = write!(out, "{key}={value}");
                }
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    /// The keys the file has, each once, with the value MangoHud uses: the
    /// last one in the file. A key alone has an empty value here.
    pub fn entries(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = Vec::new();
        for line in &self.lines {
            let (key, value) = match line {
                Line::Flag(key) => (key, String::new()),
                Line::Pair { key, value } => (key, value.clone()),
                Line::Raw(_) => continue,
            };
            match found.iter_mut().find(|(existing, _)| existing == key) {
                Some(entry) => entry.1 = value,
                None => found.push((key.clone(), value)),
            }
        }
        found
    }

    /// Changes one key where it first is, or adds it at the end, and drops any
    /// later copy of it: MangoHud keeps the last one, so a copy left behind
    /// would undo the change. An empty value turns the option on, written as
    /// the key alone. Returns false when the value is not acceptable.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        if !valid(key, value) {
            return false;
        }
        let new = if value.is_empty() {
            Line::Flag(key.to_string())
        } else {
            Line::Pair { key: key.to_string(), value: value.to_string() }
        };
        match self.lines.iter().position(|line| line.key() == Some(key)) {
            Some(first) => {
                self.lines[first] = new;
                let mut index = 0;
                self.lines.retain(|line| {
                    let keep = index <= first || line.key() != Some(key);
                    index += 1;
                    keep
                });
            }
            None => self.lines.push(new),
        }
        true
    }

    /// Removes a key, leaving the rest of the file as it was.
    pub fn remove(&mut self, key: &str) {
        self.lines.retain(|line| line.key() != Some(key));
    }
}

/// A key is a bare name; a value may not carry a new line or a comment mark,
/// which would turn one setting into two lines of the file.
fn valid(key: &str, value: &str) -> bool {
    is_key(key)
        && value.len() <= 256
        && !value.contains('\n')
        && !value.contains('#')
}

fn home() -> Option<PathBuf> {
    std::env::var("HOME").ok().filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// Where the file of this user is. It is the same one for every install:
/// MangoHud from apt reads it, and the Flatpaks of Steam, Lutris and Heroic
/// mount this folder of the host (`xdg-config/MangoHud:ro`) and read it too.
/// GOverlay, from apt or Flatpak, writes its global configuration here as well.
pub fn path() -> Option<PathBuf> {
    Some(home()?.join(".config/MangoHud/MangoHud.conf"))
}

pub fn read(path: &Path) -> io::Result<Document> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Document::parse(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Document::default()),
        Err(error) => Err(error),
    }
}

/// Writes through a temporary file, so a crash never leaves half a
/// configuration behind.
pub fn write(path: &Path, document: &Document) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("conf.tmp");
    fs::write(&temporary, document.to_text())?;
    fs::rename(&temporary, path)
}

/// One key the window can offer as a box, a number or a small text field.
///
/// The names are the ones MangoHud itself uses, so the file stays readable by
/// whoever opens it: the window translates the titles, never the keys.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: &'static str,
    /// "metrics", "look", "limits" or "keys".
    pub group: &'static str,
    /// "flag" for a box, "number" for a box with a value, "text" for a field.
    pub kind: &'static str,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// The words the file takes, when it takes one of a few.
    pub choices: &'static [&'static str],
    /// What is written when the box is ticked. Empty for an option that is
    /// only on or off; a key with a value never goes in alone, MangoHud would
    /// read it as 1.
    pub default: &'static str,
}

const fn flag(key: &'static str, group: &'static str) -> Setting {
    Setting { key, group, kind: "flag", min: None, max: None, choices: &[], default: "" }
}

const fn number(key: &'static str, group: &'static str, min: f64, max: f64, default: &'static str) -> Setting {
    Setting { key, group, kind: "number", min: Some(min), max: Some(max), choices: &[], default }
}

const fn text(key: &'static str, group: &'static str, choices: &'static [&'static str], default: &'static str) -> Setting {
    Setting { key, group, kind: "text", min: None, max: None, choices, default }
}

const POSITIONS: [&str; 9] = [
    "top-left",
    "top-center",
    "top-right",
    "middle-left",
    "middle-center",
    "middle-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

const FPS_METHODS: [&str; 2] = ["early", "late"];

/// What the window offers, grouped as a player thinks of it: what to show, how
/// it looks, what it limits and which key hides it.
pub const SETTINGS: &[Setting] = &[
    flag("fps", "metrics"),
    flag("frame_timing", "metrics"),
    flag("gpu_stats", "metrics"),
    flag("gpu_temp", "metrics"),
    flag("gpu_core_clock", "metrics"),
    flag("gpu_mem_clock", "metrics"),
    flag("gpu_power", "metrics"),
    flag("vram", "metrics"),
    flag("cpu_stats", "metrics"),
    flag("cpu_temp", "metrics"),
    flag("cpu_power", "metrics"),
    flag("cpu_mhz", "metrics"),
    flag("ram", "metrics"),
    flag("network", "metrics"),
    // The defaults are the ones of MangoHud itself, except vsync, which has
    // none to write ("leave it to the game"): a ticked box means on (3).
    text("position", "look", &POSITIONS, "top-left"),
    number("font_size", "look", 8.0, 48.0, "24"),
    number("background_alpha", "look", 0.0, 1.0, "0.5"),
    flag("text_outline", "look"),
    number("fps_limit", "limits", 0.0, 480.0, "0"),
    number("vsync", "limits", 0.0, 3.0, "3"),
    text("fps_limit_method", "limits", &FPS_METHODS, "late"),
    text("toggle_hud", "keys", &[], "Shift_R+F12"),
];

/// The folder MangoHud writes its logs to, when the configuration names one
/// with a full path. MangoHud writes one file per game and session in it.
pub fn log_folder(document: &Document) -> Option<PathBuf> {
    let (_, value) = document.entries().into_iter().find(|(key, _)| key == "output_folder")?;
    let path = PathBuf::from(value.trim());
    path.is_absolute().then_some(path)
}

/// The folder the panel offers for the logs. A game inside the Flatpak of
/// Steam can only write in the folder of Steam (the folder of MangoHud is
/// read only there), and that folder is also writable for Heroic, Lutris and
/// any game outside a sandbox; without the Flatpak of Steam, the folder of
/// the panel does.
pub fn default_log_folder() -> Option<PathBuf> {
    let home = home()?;
    let steam_flatpak = home.join(".var/app/com.valvesoftware.Steam");
    if steam_flatpak.is_dir() {
        return Some(steam_flatpak.join("mangohud-logs"));
    }
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    Some(data.join("soplos-gaming-panel").join("mangohud-logs"))
}

/// The newest log in the folder, if it was written in the last seconds: an
/// older one is a game that is no longer running. The summary MangoHud writes
/// when a log ends has no frames per second over time and is left out.
pub fn live_log(folder: &Path) -> Option<PathBuf> {
    let fresh = std::time::Duration::from_secs(5);
    fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.ends_with(".csv") && !name.ends_with("_summary.csv")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .filter(|(modified, _)| modified.elapsed().map(|age| age <= fresh).unwrap_or(true))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// The three settings people actually want, by name.
pub fn preset(name: &str) -> Option<&'static [(&'static str, &'static str)]> {    match name {
        "performance" => Some(&[
            ("fps_limit", "0"),
            ("cpu_stats", ""),
            ("gpu_stats", ""),
            ("gpu_temp", ""),
            ("vram", ""),
            ("ram", ""),
            ("frame_timing", ""),
            ("gpu_load_change", ""),
            ("cpu_load_change", ""),
            ("toggle_hud", "Shift_R+F12"),
        ]),
        "readable" => Some(&[
            ("font_size", "28"),
            ("background_alpha", "0.45"),
            ("text_outline", ""),
            ("gpu_stats", ""),
            ("cpu_stats", ""),
            ("fps", ""),
            ("gpu_temp", ""),
            ("cpu_temp", ""),
        ]),
        "minimal" => Some(&[("gpu_stats", ""), ("cpu_stats", ""), ("fps", "")]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch;

    const SAMPLE: &str = "# MangoHud escrito por mí\n\
gpu_stats\n\
# el tamaño que me gusta\n\
font_size=24\n\
\n\
fps_limit=60\n";

    #[test]
    fn reading_and_writing_keeps_the_file_exactly_as_it_was() {
        let document = Document::parse(SAMPLE);
        assert_eq!(document.to_text(), SAMPLE);
    }

    #[test]
    fn changing_one_key_leaves_the_comments_where_they_are() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("font_size", "32"));
        let text = document.to_text();
        assert!(text.starts_with("# MangoHud escrito por mí\ngpu_stats\n# el tamaño que me gusta\nfont_size=32\n"));
        assert!(text.contains("\nfps_limit=60\n"));
    }

    #[test]
    fn a_new_key_is_added_at_the_end() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("vram", ""));
        assert_eq!(document.to_text(), format!("{SAMPLE}vram\n"));
    }

    #[test]
    fn a_key_alone_is_an_option_that_is_on() {
        let document = Document::parse(SAMPLE);
        assert!(document.entries().contains(&("gpu_stats".to_string(), String::new())));
    }

    #[test]
    fn an_option_turned_on_is_never_written_with_an_empty_value() {
        // MangoHud reads `fps=` as fps=0, which turns the option off.
        let mut document = Document::parse("fps=\n");
        assert!(document.set("fps", ""));
        assert_eq!(document.to_text(), "fps\n");
    }

    #[test]
    fn turning_on_an_option_already_on_does_not_copy_it() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("gpu_stats", ""));
        assert_eq!(document.to_text(), SAMPLE);
    }

    #[test]
    fn a_later_copy_of_a_key_is_dropped_when_it_is_set() {
        // What the panel left behind before: the line of GOverlay and an
        // empty copy at the end that turned it off.
        let mut document = Document::parse("gpu_stats\nfont_size=24\ngpu_stats=\n");
        assert_eq!(document.entries()[0], ("gpu_stats".to_string(), String::new()));
        assert!(document.set("gpu_stats", ""));
        assert_eq!(document.to_text(), "gpu_stats\nfont_size=24\n");
    }

    #[test]
    fn the_value_shown_is_the_last_one_of_the_file() {
        let document = Document::parse("font_size=20\nfont_size=30\n");
        assert_eq!(document.entries(), vec![("font_size".to_string(), "30".to_string())]);
    }

    #[test]
    fn removing_a_key_takes_its_line_alone_too() {
        let mut document = Document::parse(SAMPLE);
        document.remove("gpu_stats");
        assert!(!document.to_text().contains("gpu_stats"));
    }

    #[test]
    fn every_setting_with_a_value_has_one_to_write() {
        for setting in SETTINGS {
            assert_eq!(setting.kind == "flag", setting.default.is_empty(), "{}", setting.key);
        }
    }

    #[test]
    fn a_key_can_be_removed_without_touching_the_rest() {
        let mut document = Document::parse(SAMPLE);
        document.remove("fps_limit");
        assert!(!document.to_text().contains("fps_limit"));
        assert!(document.to_text().contains("# el tamaño que me gusta"));
    }

    #[test]
    fn a_value_that_would_break_the_line_is_refused() {
        let mut document = Document::parse(SAMPLE);
        assert!(!document.set("font_size", "24\nmalicious=1"));
        assert!(!document.set("font_size", "24 # comentario"));
        assert!(!document.set("font size", "24"));
    }

    #[test]
    fn the_presets_exist_and_the_unknown_ones_do_not() {
        assert!(preset("performance").is_some());
        assert!(preset("readable").is_some());
        assert!(preset("minimal").is_some());
        assert!(preset("turbo").is_none());
    }

    #[test]
    fn the_log_folder_is_only_followed_when_the_path_is_full() {
        let absolute = Document::parse("output_folder=/tmp/mangohud\n");
        assert_eq!(log_folder(&absolute), Some(PathBuf::from("/tmp/mangohud")));
        let relative = Document::parse("output_folder=mangohud\n");
        assert_eq!(log_folder(&relative), None);
        assert_eq!(log_folder(&Document::parse("output_file=/tmp/old.csv\n")), None);
    }

    #[test]
    fn the_live_log_is_the_newest_and_never_the_summary() {
        let dir = scratch("sgp-mangohud", "logs");
        fs::write(dir.join("game_2026-11-19_10-00-00.csv"), "fps\n60\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(dir.join("game_2026-11-19_10-00-00.csv_summary.csv"), "x\n").unwrap();
        assert_eq!(live_log(&dir), Some(dir.join("game_2026-11-19_10-00-00.csv")));
        assert_eq!(live_log(&dir.join("missing")), None);
    }

    #[test]
    fn every_setting_offered_has_a_group_the_window_knows() {
        for setting in SETTINGS {
            assert!(
                ["metrics", "look", "limits", "keys"].contains(&setting.group),
                "{} has no group",
                setting.key
            );
            assert!(valid(setting.key, ""), "{} is not a key of the file", setting.key);
        }
    }

    #[test]
    fn the_settings_offered_can_be_written_into_the_file() {
        let mut document = Document::parse("");
        assert!(document.set("position", "top-left"));
        assert!(document.set("font_size", "24"));
        assert!(document.set("toggle_hud", "Shift_R+F12"));
        assert!(document.set("fps", ""));
        let text = document.to_text();
        assert_eq!(text, "position=top-left\nfont_size=24\ntoggle_hud=Shift_R+F12\nfps\n");
    }

    #[test]
    fn a_file_that_is_not_there_is_an_empty_document() {
        let dir = scratch("sgp-mangohud", "missing");
        let document = read(&dir.join("MangoHud.conf")).unwrap();
        assert!(document.lines.is_empty());
    }

    #[test]
    fn writing_then_reading_gives_the_same_keys() {
        let dir = scratch("sgp-mangohud", "write");
        let path = dir.join("MangoHud.conf");
        let mut document = Document::parse(SAMPLE);
        document.set("font_size", "30");
        write(&path, &document).unwrap();
        let back = read(&path).unwrap();
        assert_eq!(back.entries().iter().find(|(key, _)| key == "font_size").map(|(_, value)| value.as_str()), Some("30"));
    }
}
