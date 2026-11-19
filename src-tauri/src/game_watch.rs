// What the window does on its own when the game mode changes: a notification
// of the desktop, and the overlay that comes and goes with the game.
//
// The daemon is not asked: asking it for its state makes it walk every game
// entry and every process. It leaves two files in /run that say the same, and
// looking at them every two seconds costs nothing.

use std::path::Path;
use std::time::Duration;
use tauri::AppHandle;

use crate::daemon::state;

/// What the files of the daemon say: whether a profile is applied, and the
/// game being played, when there is one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    pub active: bool,
    pub game: Option<String>,
}

pub fn seen_in(dir: &Path) -> Seen {
    Seen {
        // The state is readable only by root; that it is there is enough.
        active: state::file_in(dir).exists(),
        game: state::load_session(dir).map(|session| session.name),
    }
}

/// What has to happen between two looks.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    ModeOn(Option<String>),
    ModeOff,
    GameStarted,
    GameEnded,
}

pub fn changes(before: &Seen, now: &Seen) -> Vec<Change> {
    let mut found = Vec::new();
    match (before.active, now.active) {
        (false, true) => found.push(Change::ModeOn(now.game.clone())),
        (true, false) => found.push(Change::ModeOff),
        _ => {}
    }
    match (&before.game, &now.game) {
        (None, Some(_)) => found.push(Change::GameStarted),
        (Some(_), None) => found.push(Change::GameEnded),
        _ => {}
    }
    found
}

fn notify(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .appname("Soplos Gaming Panel")
        .summary(summary)
        .body(body)
        .icon("soplos-gaming-panel")
        .timeout(notify_rust::Timeout::Milliseconds(4000))
        .show();
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let dir = state::root();
        // What is there when the program starts is not news.
        let mut before = seen_in(&dir);
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let now = seen_in(&dir);
            if now == before {
                continue;
            }
            let settings = crate::config::load();
            let language = crate::i18n::language(&settings);
            for change in changes(&before, &now) {
                match change {
                    Change::ModeOn(game) if settings.notify_game_mode => {
                        let body = game
                            .map(|name| crate::i18n::t(&language, "playing").replace("{game}", &name))
                            .unwrap_or_default();
                        notify(crate::i18n::t(&language, "mode-on"), &body);
                    }
                    Change::ModeOff if settings.notify_game_mode => {
                        notify(crate::i18n::t(&language, "mode-off"), crate::i18n::t(&language, "back"));
                    }
                    Change::GameStarted if settings.hud.auto_show => {
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || crate::present_hud(&handle));
                    }
                    // An overlay the user turned on stays: it was not the game
                    // that brought it.
                    Change::GameEnded if settings.hud.auto_show && !settings.hud.enabled => {
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || crate::withdraw_hud(&handle));
                    }
                    _ => {}
                }
            }
            before = now;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(active: bool, game: Option<&str>) -> Seen {
        Seen { active, game: game.map(str::to_string) }
    }

    #[test]
    fn nothing_changes_nothing_happens() {
        assert!(changes(&seen(true, Some("Hades")), &seen(true, Some("Hades"))).is_empty());
    }

    #[test]
    fn a_game_that_starts_turns_the_mode_on_with_its_name() {
        assert_eq!(
            changes(&seen(false, None), &seen(true, Some("Hades"))),
            vec![Change::ModeOn(Some("Hades".to_string())), Change::GameStarted]
        );
    }

    #[test]
    fn the_mode_turned_on_by_hand_has_no_game() {
        assert_eq!(changes(&seen(false, None), &seen(true, None)), vec![Change::ModeOn(None)]);
    }

    #[test]
    fn a_game_that_ends_turns_everything_off() {
        assert_eq!(changes(&seen(true, Some("Hades")), &seen(false, None)), vec![Change::ModeOff, Change::GameEnded]);
    }

    #[test]
    fn the_files_of_the_daemon_are_read() {
        let dir = crate::testing::scratch("sgp-watch", "files");
        assert_eq!(seen_in(&dir), seen(false, None));
        std::fs::write(state::file_in(&dir), "{}").unwrap();
        assert_eq!(seen_in(&dir), seen(true, None));
    }
}
