// The original value of every lever the governor has turned.
//
// It is written before the first write and removed only after everything has
// been put back. While the file exists, the machine is not in its normal state
// and any of the four restore paths can fix it.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub path: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// Who applied the profile, to tell a live run from a leftover.
    pub pid: u32,
    pub seconds: u64,
    /// Which profile was applied: "max", "balanced" or "quiet".
    #[serde(default)]
    pub profile: String,
    pub entries: Vec<Entry>,
}

/// Where the daemon keeps its state. The environment override exists so the
/// tests and a development run never write into /run.
pub fn root() -> PathBuf {
    match std::env::var("SOPLOS_GAMING_PANEL_RUN") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from("/run/soplos-gaming-panel"),
    }
}

pub fn file_in(dir: &Path) -> PathBuf {
    dir.join("estado.json")
}

/// The game being played right now, for the window to show it. It is not part
/// of the state of the profile: it lives in its own file so a crash does not
/// leave the window saying a game is running.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub name: String,
    pub pid: u32,
    /// When it started.
    pub seconds: u64,
    pub profile: String,
    /// The scheduler of sched_ext this program started for the game, if it
    /// started one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// The one that was running before, to give it back on the way out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_mode: Option<u32>,
}

pub fn session_in(dir: &Path) -> PathBuf {
    dir.join("sesion.json")
}

pub fn load_session(dir: &Path) -> Option<Session> {
    let raw = fs::read_to_string(session_in(dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_session(dir: &Path, session: &Session) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let body = serde_json::to_string_pretty(session).map_err(io::Error::other)?;
    fs::write(session_in(dir), body)
}

pub fn clear_session(dir: &Path) {
    let _ = fs::remove_file(session_in(dir));
}

pub fn load(dir: &Path) -> Option<State> {
    let raw = fs::read_to_string(file_in(dir)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Atomic write: a reader never sees a half written file, and the state is
/// readable only by its owner.
pub fn save(dir: &Path, state: &State) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let temporary = dir.join("estado.json.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    let mut file = options.open(&temporary)?;
    use std::io::Write;
    let body = serde_json::to_string_pretty(state).map_err(io::Error::other)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary, file_in(dir))
}

pub fn clear(dir: &Path) -> io::Result<()> {
    match fs::remove_file(file_in(dir)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> crate::testing::Scratch {
        crate::testing::scratch("sgp-state", name)
    }

    #[test]
    fn saves_loads_and_clears() {
        let root = dir("round");
        let state = State {
            pid: 42,
            seconds: now(),
            profile: "balanced".to_string(),
            entries: vec![Entry {
                key: "cpu.governor".to_string(),
                path: "/sys/x".to_string(),
                value: "schedutil".to_string(),
            }],
        };
        save(&root, &state).unwrap();
        assert_eq!(load(&root), Some(state.clone()));
        clear(&root).unwrap();
        assert_eq!(load(&root), None);
        // Clearing twice is not an error: ExecStopPost may run after the daemon
        // restored on its own.
        clear(&root).unwrap();
    }

    #[test]
    fn a_broken_file_is_treated_as_no_state() {
        let root = dir("broken");
        fs::create_dir_all(&root).unwrap();
        fs::write(file_in(&root), "{ not json").unwrap();
        assert_eq!(load(&root), None);
    }

    #[test]
    fn no_leftover_means_no_state() {
        assert_eq!(load(&dir("empty")), None);
    }
}
