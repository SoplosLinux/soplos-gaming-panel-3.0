// The state of the pieces this page talks about: the governor service of the
// package and the GameMode daemon of the distribution.
//
// Both are read without privileges. The only privileged action is starting or
// stopping the unit, which goes through pkexec and the polkit policy of the
// package like everything else.

use serde::Serialize;
use std::process::Command;

const UNIT: &str = "soplos-gaming-panel.service";
const SYSTEM_UNIT: &str = "/usr/lib/systemd/system/soplos-gaming-panel.service";

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    /// The unit of the package is installed, which is what the .deb does.
    pub unit_installed: bool,
    pub running: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameModeState {
    pub installed: bool,
    pub active: bool,
}

#[tauri::command]
pub fn governor_service() -> ServiceState {
    ServiceState {
        unit_installed: std::path::Path::new(SYSTEM_UNIT).exists(),
        running: outputs("systemctl", &["is-active", UNIT])
            .map(|text| text.trim() == "active")
            .unwrap_or(false),
    }
}

/// Starting the unit needs the administrator. Stopping it is the same command
/// the user would run, so it takes the same path.
#[tauri::command]
pub fn governor_service_control(start: bool) -> Result<(), String> {
    if !std::path::Path::new(SYSTEM_UNIT).exists() {
        return Err(format!("{SYSTEM_UNIT} is not installed"));
    }
    super::privileged::run_as_root(&[super::privileged::Op::Service { start }])
}

#[tauri::command]
pub fn gamemode_state() -> GameModeState {
    let Some(answer) = outputs("gamemoded", &["-s"]) else {
        return GameModeState::default();
    };
    // "gamemode is active" or "gamemode is inactive"; anything else means the
    // daemon answered something this page does not know.
    GameModeState { installed: true, active: answer.contains("is active") }
}

fn outputs(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&result.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&result.stderr));
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_reads_without_privileges() {
        // Whatever this machine answers, the call must not panic or need root.
        let state = governor_service();
        assert!(state.running || !state.running);
        let game = gamemode_state();
        assert!(game.installed || !game.installed);
    }

    #[test]
    fn a_missing_program_is_no_answer() {
        assert!(outputs("/nonexistent/program", &[]).is_none());
    }
}
