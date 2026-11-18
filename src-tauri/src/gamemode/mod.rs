// The configuration of Feral GameMode, read and written without losing a line.
//
// gamemode.ini is an INI file with sections: [general] for the priorities and
// the governor, [gpu] for the card, [custom] for the orders that run before and
// after the game. The panel edits the file of the user, which GameMode reads
// after the one in /etc, and keeps every comment where it is.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// Comment, blank line or anything that is not a key of this section.
    Raw(String),
    /// "[general]".
    Header(String),
    /// A key of a section, with the spaces the file puts around the "=" so a
    /// line that is not changed comes back exactly as it was.
    Key { section: String, key: String, value: String, before: String, after: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub lines: Vec<Line>,
}

impl Document {
    pub fn parse(text: &str) -> Self {
        let mut section = String::new();
        let lines = text
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    section = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
                    return Line::Header(section.clone());
                }
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                    return Line::Raw(line.to_string());
                }
                match line.split_once('=') {
                    Some((left, right)) if !left.trim().is_empty() => {
                        let key = left.trim();
                        let before = left[key.len()..].to_string();
                        let value = right.trim();
                        let after = right[..right.len() - right.trim_start().len()].to_string();
                        Line::Key {
                            section: section.clone(),
                            key: key.to_string(),
                            value: value.to_string(),
                            before,
                            after,
                        }
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
                Line::Header(section) => {
                    let _ = write!(out, "[{section}]");
                }
                Line::Key { key, value, before, after, .. } => {
                    let _ = write!(out, "{key}{before}={after}{value}");
                }
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| match line {
            Line::Key { section: current, key: current_key, value, .. } if current == section && current_key == key => Some(value.as_str()),
            _ => None,
        })
    }

    /// Sets a key inside its section. The section is created at the end when it
    /// is not there yet; nothing else moves.
    pub fn set(&mut self, section: &str, key: &str, value: &str) -> bool {
        if !valid(key, value) || !valid(section, section) {
            return false;
        }
        let mut found = false;
        for line in self.lines.iter_mut() {
            if let Line::Key { section: current, key: current_key, value: current_value, .. } = line {
                if current == section && current_key == key {
                    *current_value = value.to_string();
                    found = true;
                }
            }
        }
        if found {
            return true;
        }
        if let Some(index) = self.lines.iter().rposition(|line| matches!(line, Line::Header(name) if name == section)) {
            self.lines.insert(
                index + 1,
                Line::Key {
                    section: section.to_string(),
                    key: key.to_string(),
                    value: value.to_string(),
                    before: " ".to_string(),
                    after: " ".to_string(),
                },
            );
        } else {
            if !self.lines.is_empty() {
                self.lines.push(Line::Raw(String::new()));
            }
            self.lines.push(Line::Header(section.to_string()));
            self.lines.push(Line::Key {
                section: section.to_string(),
                key: key.to_string(),
                value: value.to_string(),
                before: " ".to_string(),
                after: " ".to_string(),
            });
        }
        true
    }

    pub fn remove(&mut self, section: &str, key: &str) {
        self.lines.retain(|line| !matches!(line, Line::Key { section: current, key: current_key, .. }
            if current == section && current_key == key));
    }

    /// The keys of one section, in the order of the file.
    pub fn section(&self, section: &str) -> Vec<(String, String)> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Key { section: current, key, value, .. } if current == section => Some((key.clone(), value.clone())),
                _ => None,
            })
            .collect()
    }
}

fn valid(key: &str, value: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.chars().all(|one| one.is_ascii_alphanumeric() || one == '_' || one == '-' || one == ' ' || one == '[' || one == ']')
        && value.len() <= 512
        && !value.contains('\n')
}

/// The file the panel writes: the one of the user, which GameMode reads after
/// the one of the system. The system one is only read, never touched.
pub fn path() -> Option<PathBuf> {
    let config = std::env::var("XDG_CONFIG_HOME").ok().filter(|value| !value.is_empty());
    let base = match config {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var("HOME").ok().filter(|value| !value.is_empty())?).join(".config"),
    };
    Some(base.join("gamemode.ini"))
}

pub fn system_path() -> PathBuf {
    PathBuf::from("/etc/gamemode.ini")
}

pub fn read(path: &Path) -> io::Result<Document> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Document::parse(&text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Document::default()),
        Err(error) => Err(error),
    }
}

pub fn write(path: &Path, document: &Document) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("ini.tmp");
    fs::write(&temporary, document.to_text())?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch;

    const SAMPLE: &str = "# lo que yo quiero\n[general]\nrenice = 0\ninhibit_screensaver = 1\n\n# la tarjeta\n[gpu]\napply_gpu_optimisations = accept_edid\n";

    #[test]
    fn the_file_survives_a_round_trip() {
        assert_eq!(Document::parse(SAMPLE).to_text(), SAMPLE);
    }

    #[test]
    fn a_key_keeps_its_section_and_the_comments_stay() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("general", "renice", "-5"));
        assert_eq!(document.get("general", "renice"), Some("-5"));
        assert_eq!(document.get("gpu", "renice"), None);
        let text = document.to_text();
        assert!(text.contains("# lo que yo quiero"));
        assert!(text.contains("# la tarjeta"));
    }

    #[test]
    fn a_new_key_goes_inside_its_section() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("general", "desiredgov", "performance"));
        let text = document.to_text();
        let general = text.find("[general]").unwrap();
        let gpu = text.find("[gpu]").unwrap();
        let added = text.find("desiredgov = performance").unwrap();
        assert!(general < added && added < gpu);
    }

    #[test]
    fn a_section_that_is_not_there_is_added() {
        let mut document = Document::parse(SAMPLE);
        assert!(document.set("custom", "start", "/usr/bin/true"));
        assert_eq!(document.get("custom", "start"), Some("/usr/bin/true"));
    }

    #[test]
    fn a_value_with_a_line_break_is_refused() {
        let mut document = Document::parse(SAMPLE);
        assert!(!document.set("custom", "start", "a\nb"));
    }

    #[test]
    fn writing_and_reading_gives_the_same_sections() {
        let dir = scratch("sgp-gamemode", "ini");
        let path = dir.join("gamemode.ini");
        let mut document = Document::parse(SAMPLE);
        document.set("gpu", "amd_performance_level", "high");
        write(&path, &document).unwrap();
        let back = read(&path).unwrap();
        assert_eq!(back.section("gpu").len(), 2);
        assert_eq!(back.get("gpu", "amd_performance_level"), Some("high"));
    }

    #[test]
    fn a_file_that_is_not_there_is_an_empty_document() {
        let dir = scratch("sgp-gamemode", "missing");
        assert!(read(&dir.join("gamemode.ini")).unwrap().lines.is_empty());
    }
}
