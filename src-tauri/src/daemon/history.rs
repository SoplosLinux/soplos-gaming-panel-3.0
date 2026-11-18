// What has been played, so the window can answer the only question a player
// asks after a week: how long was I at this, and with which profile.
//
// One line of JSON per session, appended when the game closes. A small file
// that is read from its end and can be deleted without losing anything.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub name: String,
    pub profile: String,
    /// How long the game was open.
    pub seconds: u64,
    /// When it closed.
    pub at: u64,
}

fn file(dir: &Path) -> std::path::PathBuf {
    dir.join("history.jsonl")
}

pub fn append(dir: &Path, session: &Session) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let line = serde_json::to_string(session).map_err(std::io::Error::other)?;
    let mut out = fs::OpenOptions::new().create(true).append(true).open(file(dir))?;
    out.write_all(format!("{line}\n").as_bytes())
}

/// The last sessions, newest first.
pub fn recent(dir: &Path, limit: usize) -> Vec<Session> {
    let Ok(text) = fs::read_to_string(file(dir)) else { return Vec::new() };
    let mut sessions: Vec<Session> = text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Session>(line).ok())
        .take(limit)
        .collect();
    sessions.truncate(limit);
    sessions
}

/// Total time per game, longest first: the answer to "which one have I been
/// playing all month".
pub fn totals(dir: &Path, limit: usize) -> Vec<(String, u64)> {
    let Ok(text) = fs::read_to_string(file(dir)) else { return Vec::new() };
    let mut sums: Vec<(String, u64)> = Vec::new();
    for session in text.lines().filter_map(|line| serde_json::from_str::<Session>(line).ok()) {
        match sums.iter_mut().find(|(name, _)| name == &session.name) {
            Some((_, seconds)) => *seconds += session.seconds,
            None => sums.push((session.name, session.seconds)),
        }
    }
    sums.sort_by_key(|(_, seconds)| std::cmp::Reverse(*seconds));
    sums.truncate(limit);
    sums
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> crate::testing::Scratch {
        crate::testing::scratch("sgp-history", name)
    }

    #[test]
    fn without_a_file_there_is_no_history() {
        assert!(recent(&dir("empty"), 5).is_empty());
    }

    #[test]
    fn the_newest_session_is_the_first_one_returned() {
        let root = dir("two");
        append(&root, &Session { name: "uno".to_string(), profile: "max".to_string(), seconds: 60, at: 1 }).unwrap();
        append(&root, &Session { name: "dos".to_string(), profile: "quiet".to_string(), seconds: 30, at: 2 }).unwrap();
        let sessions = recent(&root, 5);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "dos");
        assert_eq!(sessions[1].name, "uno");
    }

    #[test]
    fn a_broken_line_does_not_hide_the_rest() {
        let root = dir("broken");
        append(&root, &Session { name: "uno".to_string(), profile: "max".to_string(), seconds: 60, at: 1 }).unwrap();
        fs::OpenOptions::new().append(true).open(file(&root)).unwrap().write_all(b"{ medio\n").unwrap();
        assert_eq!(recent(&root, 5).len(), 1);
    }

    #[test]
    fn the_totals_add_up_and_put_the_longest_first() {
        let root = dir("totals");
        append(&root, &Session { name: "uno".to_string(), profile: "max".to_string(), seconds: 60, at: 1 }).unwrap();
        append(&root, &Session { name: "dos".to_string(), profile: "max".to_string(), seconds: 30, at: 2 }).unwrap();
        append(&root, &Session { name: "uno".to_string(), profile: "quiet".to_string(), seconds: 100, at: 3 }).unwrap();
        let totals = totals(&root, 5);
        assert_eq!(totals[0], ("uno".to_string(), 160));
        assert_eq!(totals[1], ("dos".to_string(), 30));
    }
}
