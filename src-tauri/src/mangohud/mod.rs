// The MangoHud configuration file, read and written without losing anything.
//
// The file belongs to the user and is full of comments they wrote: every line
// is kept as it is, and a change only touches the line of the key it changes.
// Nothing is rewritten from a model of what the file "should" be.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One line of the file: either a key with its value, or anything else
/// (comment, blank line) kept verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Raw(String),
    Pair { key: String, value: String },
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

    /// The keys the file already has, in its order.
    pub fn entries(&self) -> Vec<(String, String)> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Pair { key, value } => Some((key.clone(), value.clone())),
                Line::Raw(_) => None,
            })
            .collect()
    }

    /// Changes one key where it already is, or adds it at the end. Returns
    /// false when the value is not acceptable for that key.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        if !valid(key, value) {
            return false;
        }
        if let Some(Line::Pair { value: current, .. }) = self
            .lines
            .iter_mut()
            .find(|line| matches!(line, Line::Pair { key: existing, .. } if existing == key))
        {
            *current = value.to_string();
            return true;
        }
        self.lines.push(Line::Pair { key: key.to_string(), value: value.to_string() });
        true
    }

    /// Removes a key, leaving the rest of the file as it was.
    pub fn remove(&mut self, key: &str) {
        self.lines.retain(|line| !matches!(line, Line::Pair { key: existing, .. } if existing == key));
    }
}

/// A key is a bare name; a value may not carry a new line or a comment mark,
/// which would turn one setting into two lines of the file.
fn valid(key: &str, value: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.chars().all(|one| one.is_ascii_alphanumeric() || one == '_')
        && value.len() <= 256
        && !value.contains('\n')
        && !value.contains('#')
}

/// Where the file of this user is. The Flatpak keeps its own copy, and it is
/// the one that counts when MangoHud runs inside the sandbox.
pub fn path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().filter(|value| !value.is_empty())?;
    let native = Path::new(&home).join(".config/MangoHud/MangoHud.conf");
    let flatpak = Path::new(&home).join(".var/app/io.github.flightlessmango.mangohud/config/MangoHud/MangoHud.conf");
    if native.exists() {
        Some(native)
    } else if flatpak.exists() {
        Some(flatpak)
    } else {
        Some(native)
    }
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
}

const fn flag(key: &'static str, group: &'static str) -> Setting {
    Setting { key, group, kind: "flag", min: None, max: None, choices: &[] }
}

const fn number(key: &'static str, group: &'static str, min: f64, max: f64) -> Setting {
    Setting { key, group, kind: "number", min: Some(min), max: Some(max), choices: &[] }
}

const fn text(key: &'static str, group: &'static str, choices: &'static [&'static str]) -> Setting {
    Setting { key, group, kind: "text", min: None, max: None, choices }
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
    text("position", "look", &POSITIONS),
    number("font_size", "look", 8.0, 48.0),
    number("background_alpha", "look", 0.0, 1.0),
    flag("text_outline", "look"),
    number("fps_limit", "limits", 0.0, 480.0),
    number("vsync", "limits", 0.0, 3.0),
    text("fps_limit_method", "limits", &FPS_METHODS),
    text("toggle_hud", "keys", &[]),
];

/// The file MangoHud writes its log to, when the configuration names one with
/// a full path. A relative name lands next to the game, which cannot be
/// followed from here.
pub fn log_path(document: &Document) -> Option<PathBuf> {
    let (_, value) = document.entries().into_iter().find(|(key, _)| key == "output_file")?;
    let path = PathBuf::from(value.trim());
    path.is_absolute().then_some(path)
}

/// The log the panel offers: inside its own folder, so it is one the panel can
/// read while the game runs.
pub fn default_log_path() -> Option<PathBuf> {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|home| Path::new(&home).join(".local/share")))?;
    Some(base.join("soplos-gaming-panel").join("mangohud-fps.csv"))
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
        assert_eq!(document.to_text(), format!("{SAMPLE}vram=\n"));
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
    fn the_log_is_only_followed_when_the_path_is_full() {
        let absolute = Document::parse("output_file=/tmp/mangohud.csv\n");
        assert_eq!(log_path(&absolute), Some(PathBuf::from("/tmp/mangohud.csv")));
        let relative = Document::parse("output_file=mangohud.csv\n");
        assert_eq!(log_path(&relative), None);
        assert_eq!(log_path(&Document::parse("fps\n")), None);
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
        assert_eq!(text, "position=top-left\nfont_size=24\ntoggle_hud=Shift_R+F12\nfps=\n");
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
