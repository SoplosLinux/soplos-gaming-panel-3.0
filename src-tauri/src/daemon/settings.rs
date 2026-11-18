// What the daemon does on its own: whether a profile goes on when a game
// starts, which one, and the names that are never taken for a game.
//
// It lives in the folder of the daemon, not in the configuration of the user,
// because it is read by the service; the window changes it over the socket.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

use crate::governor;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Auto {
    pub enabled: bool,
    /// Profile applied when a game starts.
    pub profile: String,
    pub exclusions: Vec<String>,
    /// A game that asks for its own profile, by name of its program.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Programs the user says are games, for those that have no desktop entry
    /// (Wine, builds of their own).
    #[serde(default)]
    pub manual: Vec<String>,
}

/// "This game always goes with this profile."
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub profile: String,
    /// The scheduler of sched_ext this game asks for, when it asks for one.
    /// The games use it while they run and it is let go afterwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// Its mode: 0 auto, 1 gaming, 2 powersave, 3 lowlatency, 4 server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

impl Default for Auto {
    fn default() -> Self {
        Self {
            enabled: true,
            profile: default_profile(detects_battery()).to_string(),
            exclusions: Vec::new(),
            rules: Vec::new(),
            manual: Vec::new(),
        }
    }
}

/// A laptop goes quiet to keep the fan down; a desktop goes for everything.
fn default_profile(laptop: bool) -> &'static str {
    if laptop {
        governor::profile::BALANCED
    } else {
        governor::profile::MAX
    }
}

fn detects_battery() -> bool {
    fs::read_dir("/sys/class/power_supply")
        .map(|entries| {
            entries.flatten().any(|entry| {
                fs::read_to_string(entry.path().join("type"))
                    .map(|kind| kind.trim() == "Battery")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

impl Auto {
    /// The profile a game gets: its own rule, or the one of the mode.
    /// The rule of one game, if it has one of its own.
    pub fn rule_for(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.name == name)
    }

    pub fn profile_for(&self, name: &str) -> String {
        self.rule_for(name)
            .map(|rule| rule.profile.clone())
            .unwrap_or_else(|| self.profile.clone())
    }
}

pub fn load(dir: &Path) -> Auto {
    fs::read_to_string(dir.join("auto.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, auto: &Auto) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let body = serde_json::to_string_pretty(auto).map_err(io::Error::other)?;
    fs::write(dir.join("auto.json"), body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> crate::testing::Scratch {
        crate::testing::scratch("sgp-auto", name)
    }

    #[test]
    fn without_a_file_the_daemon_is_already_on() {
        let auto = load(&dir("empty"));
        assert!(auto.enabled);
        assert_eq!(auto.exclusions.len(), 0);
    }

    #[test]
    fn a_laptop_goes_quiet_and_a_desktop_goes_for_everything() {
        assert_eq!(default_profile(true), governor::profile::BALANCED);
        assert_eq!(default_profile(false), governor::profile::MAX);
    }

    #[test]
    fn what_is_saved_is_read_back() {
        let root = dir("round");
        let auto = Auto {
            enabled: true,
            profile: "quiet".to_string(),
            exclusions: vec!["foo".to_string()],
            rules: vec![Rule {
                name: "doom".to_string(),
                profile: "max".to_string(),
                scheduler: Some("scx_lavd".to_string()),
                mode: Some(1),
            }],
            manual: vec!["mybuild".to_string()],
        };
        save(&root, &auto).unwrap();
        assert_eq!(load(&root), auto);
    }

    #[test]
    fn a_game_with_a_rule_keeps_its_own_profile() {
        let auto = Auto {
            enabled: true,
            profile: "quiet".to_string(),
            exclusions: Vec::new(),
            rules: vec![Rule {
                name: "doom".to_string(),
                profile: "max".to_string(),
                scheduler: None,
                mode: None,
            }],
            manual: vec!["mybuild".to_string()],
        };
        assert_eq!(auto.profile_for("doom"), "max");
        assert_eq!(auto.profile_for("indie"), "quiet");
        assert!(auto.rule_for("doom").is_some());
        assert!(auto.rule_for("indie").is_none());
    }

    #[test]
    fn a_rule_of_an_older_file_has_no_scheduler_and_is_read_all_the_same() {
        let old = r#"{"enabled":true,"profile":"max","exclusions":[],"rules":[{"name":"doom","profile":"max"}],"manual":[]}"#;
        let auto: Auto = serde_json::from_str(old).unwrap();
        assert_eq!(auto.profile_for("doom"), "max");
        assert_eq!(auto.rule_for("doom").and_then(|rule| rule.scheduler.clone()), None);
    }
}
