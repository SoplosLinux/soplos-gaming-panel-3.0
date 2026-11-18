// One knob of the kernel the governor can turn.
//
// Every lever says where its value lives, what it is worth now and what it is
// worth while a game runs. A lever whose node does not exist in this kernel is
// simply not built, so the window shows it as unavailable instead of pretending
// it was applied.

use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub trait Lever {
    /// Stable name used by the state file, e.g. "cpu.governor". The window turns
    /// it into a readable label with the key `lever.<name>`.
    fn key(&self) -> &'static str;

    /// "cpu", "memory", "kernel" or "disk": how the window groups the rows.
    fn group(&self) -> &'static str;

    /// The node this lever reads, and writes when it has only one.
    fn path(&self) -> &Path;

    /// Every node the lever writes: one, or one per core for the levers of
    /// cpufreq, where each core (or group of cores) has a policy of its own.
    fn paths(&self) -> Vec<std::path::PathBuf> {
        vec![self.path().to_path_buf()]
    }

    /// The value to write while a game is running.
    fn game_value(&self) -> String;

    /// The values the node accepts, when the node can say it itself.
    fn allowed(&self) -> Option<Vec<String>> {
        None
    }
}

/// How a lever that this machine does not have could be brought in, if at all.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fix {
    /// A package that brings the node. The Flatpak id is only filled in when
    /// the application really exists there, never guessed.
    // The first lever that needs a package is the SCX one, in F7.
    #[allow(dead_code)]
    Package { apt: Option<String>, flatpak: Option<String> },
    /// Only another kernel brings it: the running one has no such node.
    Kernel,
    /// Nothing to install: the hardware does not offer it.
    Hardware,
}

/// A lever the interface shows as unavailable, with the reason and the way out.
/// The reason is the name of a translated string, never the text itself: the
/// window is in eight languages.
#[derive(Serialize, Clone, Debug)]
pub struct Missing {
    pub key: &'static str,
    pub group: &'static str,
    pub reason: &'static str,
    pub fix: Fix,
}

pub fn read(path: &Path) -> io::Result<String> {
    Ok(fs::read_to_string(path)?.trim().to_string())
}

pub fn write(path: &Path, value: &str) -> io::Result<()> {
    // Truncating is what a shell does with `echo value > node`, and it is what
    // keeps a shorter value from leaving the tail of the old one behind.
    let mut file = fs::OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")
}

/// Splits a space separated list of accepted values, as the kernel writes it.
pub fn list(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

/// The value in use, from a node that writes all its options and marks the
/// chosen one in brackets: "[always] madvise never", "none [mq-deadline]".
/// Comparing that raw text with the value just written would lie.
pub fn current(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed[start..].find(']') {
            return trimmed[start + 1..start + end].to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::current;

    #[test]
    fn the_bracketed_option_is_the_one_in_use() {
        assert_eq!(current("[always] madvise never"), "always");
        assert_eq!(current("none [mq-deadline] kyber"), "mq-deadline");
        assert_eq!(current("schedutil"), "schedutil");
    }
}
