// The governor daemon: the part that switches the profile on when a game
// starts and puts everything back when it ends.
//
// It runs as a system service, because writing in /sys and /proc/sys needs the
// administrator and asking for a password in the middle of a game is not an
// option. The window talks to it over a socket in /run: named operations only,
// never an arbitrary path, so a local user cannot use the daemon to write
// wherever it likes.
//
// It is started twice by systemd: `--daemon` serves, and `--daemon-restore` is
// the ExecStopPost, which puts the machine back even after a kill or a crash.

pub mod history;
pub mod settings;
pub mod state;

use crate::governor::{self, games, lever};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Applying and restoring read and write the same file: one at a time, whether
/// the request comes from the window or from the watcher of games.
static GOVERNOR: Mutex<()> = Mutex::new(());

pub fn socket(dir: &std::path::Path) -> PathBuf {
    dir.join("daemon.sock")
}

/// The automatic settings outlive a reboot, so they do not live in /run with
/// the state of the current profile.
pub fn config_dir() -> PathBuf {
    match std::env::var("SOPLOS_GAMING_PANEL_CONFIG") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from("/var/lib/soplos-gaming-panel"),
    }
}

/// The dirty page limits come in pairs: the kernel uses either the bytes or the
/// ratio, and the one not in use reads 0. Writing 0 to a bytes node is refused,
/// so a machine that used the ratio (the default) gets its ratio back instead,
/// which sets the bytes to 0 by itself.
const DIRTY_PAIRS: [(&str, &str, &str); 2] = [
    ("vm.dirty_bytes", "vm.dirty_ratio", "/proc/sys/vm/dirty_ratio"),
    ("vm.dirty_background_bytes", "vm.dirty_background_ratio", "/proc/sys/vm/dirty_background_ratio"),
];

/// The ratios of the pairs whose bytes this profile writes, read before the
/// profile. They go after the levers, so restoring in reverse writes them first.
fn dirty_ratios(entries: &[state::Entry]) -> Vec<state::Entry> {
    DIRTY_PAIRS
        .iter()
        .filter(|(bytes, _, _)| entries.iter().any(|entry| entry.key == *bytes))
        .filter_map(|(_, ratio, path)| {
            lever::read(Path::new(path)).ok().map(|value| state::Entry {
                key: ratio.to_string(),
                path: path.to_string(),
                value,
            })
        })
        .collect()
}

/// A bytes node of a pair that was not in use before the profile: its ratio,
/// restored first, already put it back to 0.
fn restored_by_its_ratio(entry: &state::Entry) -> bool {
    entry.value.trim() == "0" && DIRTY_PAIRS.iter().any(|(bytes, _, _)| entry.key == *bytes)
}

/// Whether a game was started through GameMode (`gamemoderun`, which preloads
/// its library in the game). GameMode then sets the CPU governor itself and
/// puts its own back when the game ends: if the profile wrote it too, the one
/// that finished last would win.
fn gamemode_in_use() -> bool {
    let Ok(entries) = fs::read_dir("/proc") else { return false };
    entries.flatten().any(|entry| {
        let is_process = entry.file_name().to_str().is_some_and(|name| name.chars().all(|c| c.is_ascii_digit()));
        is_process && fs::read(entry.path().join("environ")).map(|environ| preloads_gamemode(&environ)).unwrap_or(false)
    })
}

/// Whether the environment of a process preloads the library of GameMode.
fn preloads_gamemode(environ: &[u8]) -> bool {
    const LIBRARY: &[u8] = b"libgamemodeauto";
    environ
        .split(|byte| *byte == 0)
        .filter(|variable| variable.starts_with(b"LD_PRELOAD="))
        .any(|variable| variable.windows(LIBRARY.len()).any(|part| part == LIBRARY))
}

/// Reads the current value of every lever, saves it and only then writes the
/// profile. If anything fails halfway, what was already written is put back.
pub fn apply(chosen: &str) -> Result<Value, String> {
    apply_leaving(chosen, &[])
}

/// The same, leaving alone the levers named, which something else governs.
fn apply_leaving(chosen: &str, leave: &[&str]) -> Result<Value, String> {
    let _guard = GOVERNOR.lock().map_err(|_| "the governor is busy".to_string())?;
    if let Some(current) = state::load(&state::root()) {
        if current.profile == chosen {
            return Ok(status());
        }
        // Another profile: the machine goes back to its own values first, so
        // what is saved below is still what it had before any profile.
        restore_in(&state::root())?;
    }
    let levers = governor::levers();
    if levers.is_empty() {
        return Err("this kernel offers no lever the governor can use".to_string());
    }
    // The profile is checked before anything is read or written.
    if governor::profile::value(chosen, "cpu.governor", "performance").is_none() {
        return Err(format!("unknown profile: {chosen}"));
    }

    // What each lever gets under this profile, decided before anything is
    // read: a lever the profile leaves alone is neither saved nor written.
    let mut plan: Vec<(&Box<dyn governor::lever::Lever>, String)> = Vec::with_capacity(levers.len());
    for lever in &levers {
        let action = governor::profile::value(chosen, lever.key(), &lever.game_value())
            .ok_or_else(|| format!("unknown profile: {chosen}"))?;
        if leave.contains(&lever.key()) {
            continue;
        }
        if let governor::profile::Action::Set(value) = action {
            plan.push((lever, value));
        }
    }

    // Every node is saved, one per core for the levers of cpufreq.
    let mut entries = Vec::with_capacity(plan.len());
    for (lever, _) in &plan {
        for path in lever.paths() {
            let value = lever::read(&path).map_err(|error| format!("{}: {error}", lever.key()))?;
            entries.push(state::Entry {
                key: lever.key().to_string(),
                path: path.display().to_string(),
                value,
            });
        }
    }
    let ratios = dirty_ratios(&entries);
    entries.extend(ratios);
    let saved = state::State {
        pid: std::process::id(),
        seconds: state::now(),
        profile: chosen.to_string(),
        entries,
    };
    state::save(&state::root(), &saved).map_err(|error| format!("estado.json: {error}"))?;

    let mut applied = Vec::with_capacity(plan.len());
    for (lever, value) in plan {
        if let Some(accepted) = lever.allowed() {
            if !accepted.iter().any(|candidate| candidate == &value) {
                let _ = restore_in(&state::root());
                return Err(format!("{}: this kernel does not offer {value}", lever.key()));
            }
        }
        for path in lever.paths() {
            if let Err(error) = lever::write(&path, &value) {
                let _ = restore_in(&state::root());
                return Err(format!("{}: {error}", lever.key()));
            }
        }
        applied.push(json!({ "key": lever.key(), "value": value }));
    }
    Ok(json!({ "profile": chosen, "applied": applied }))
}

/// Writes the saved values back, in reverse order, and forgets the state.
pub fn restore() -> Result<Value, String> {
    let _guard = GOVERNOR.lock().map_err(|_| "the governor is busy".to_string())?;
    let dir = state::root();
    restore_in(&dir)
}

fn restore_in(dir: &std::path::Path) -> Result<Value, String> {
    let Some(saved) = state::load(dir) else {
        return Ok(json!({ "restored": [] }));
    };
    let mut done = Vec::with_capacity(saved.entries.len());
    let mut pending: Vec<&state::Entry> = Vec::new();
    for entry in saved.entries.iter().rev() {
        if restored_by_its_ratio(entry) {
            done.push(json!({ "key": entry.key, "value": entry.value }));
            continue;
        }
        match lever::write(std::path::Path::new(&entry.path), &entry.value) {
            Ok(()) => done.push(json!({ "key": entry.key, "value": entry.value })),
            Err(_) => pending.push(entry),
        }
    }
    // Some nodes take a value only once another one is back: the energy
    // preference of a core is refused while its governor is still at
    // "performance". What failed is tried once more, after the rest; a state
    // saved by an older version, with the governor before the preference,
    // comes back this way too.
    let mut failures = Vec::new();
    for entry in pending {
        match lever::write(std::path::Path::new(&entry.path), &entry.value) {
            Ok(()) => done.push(json!({ "key": entry.key, "value": entry.value })),
            Err(error) => failures.push(json!({ "key": entry.key, "error": error.to_string() })),
        }
    }
    // A failure keeps the file, so the next restore path tries again instead of
    // forgetting the original value forever.
    if failures.is_empty() {
        state::clear(dir).map_err(|error| format!("estado.json: {error}"))?;
        Ok(json!({ "restored": done }))
    } else {
        Err(json!({ "restored": done, "failed": failures }).to_string())
    }
}

pub fn status() -> Value {
    let dir = state::root();
    let saved = state::load(&dir);
    let mut value = governor::catalogue_json();
    value["active"] = json!(saved.is_some());
    value["since"] = json!(saved.as_ref().map(|state| state.seconds));
    value["profile"] = json!(saved.as_ref().map(|state| state.profile.clone()));
    value["original"] = json!(saved.map(|state| state.entries));
    value["profiles"] = json!(governor::profile::ids());
    let auto = settings::load(&config_dir());
    value["auto"] = json!(auto);
    value["games"] = json!(games::running(&auto.exclusions, &auto.manual));
    value["session"] = match state::load_session(&dir) {
        Some(session) => json!(session),
        None => Value::Null,
    };
    value["history"] = json!(history::recent(&config_dir(), 8));
    value["totals"] = totals_json(5);
    value["preview"] = preview();
    value
}

/// The ExecStopPost of the unit, and the first thing the daemon does when it
/// starts: a state file that survived means the machine is still in game mode.
pub fn restore_and_report() -> i32 {
    match restore() {
        Ok(value) => {
            println!("{value}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

pub fn serve() -> i32 {
    let dir = state::root();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("{}: {error}", dir.display());
        return 1;
    }
    if let Ok(value) = restore() {
        if value["restored"].as_array().is_some_and(|list| !list.is_empty()) {
            println!("leftover profile restored: {value}");
        }
    }

    let path = socket(&dir);
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return 1;
        }
    };
    // The socket is open to everybody only while there is nobody to close it
    // to: once the user of this machine has the settings of the program, only
    // that user may talk to the daemon. The user is looked for again on every
    // connection until there is one, so the first launch of the window after
    // the boot closes it without a restart of the service.
    let mut restricted = restrict_socket(&path);

    watch(dir.clone(), config_dir());

    for stream in listener.incoming() {
        if !restricted {
            restricted = restrict_socket(&path);
        }
        match stream {
            Ok(stream) => {
                if let Err(error) = answer(stream) {
                    eprintln!("request: {error}");
                }
            }
            Err(error) => eprintln!("connection: {error}"),
        }
    }
    0
}

/// Gives the socket to the user of the settings, or leaves it open to all
/// while there is none yet. Returns whether it is closed to that user.
fn restrict_socket(path: &Path) -> bool {
    match served_uid() {
        Some(uid) => {
            let _ = std::os::unix::fs::chown(path, Some(uid), None);
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            true
        }
        None => {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666));
            false
        }
    }
}

/// The longest request the window sends is a few hundred bytes.
const REQUEST_LIMIT: u64 = 16 * 1024;

fn answer(mut stream: UnixStream) -> std::io::Result<()> {
    // One request at a time: a client that connects and says nothing must not
    // keep the daemon from answering the next one.
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    let mut line = String::new();
    let mut reader = BufReader::new(std::io::Read::take(stream.try_clone()?, REQUEST_LIMIT));
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
    let profile = request["profile"].as_str().unwrap_or(governor::profile::MAX);
    let response = match request["op"].as_str().unwrap_or("status") {
        "apply" => ok_or_error(apply(profile)),
        "auto" => ok_or_error(set_auto(&request)),
        "rule" => ok_or_error(set_rule(&request)),
        "manual" => ok_or_error(set_manual(&request)),
        "exclude" => ok_or_error(set_exclusion(&request, true)),
        "include" => ok_or_error(set_exclusion(&request, false)),
        "restore" => ok_or_error(restore()),
        "status" => json!({ "ok": true, "result": status() }),
        other => json!({ "ok": false, "error": format!("unknown operation: {other}") }),
    };
    stream.write_all(format!("{response}\n").as_bytes())?;
    stream.flush()
}

/// The user this daemon works for: the one that has the settings of the
/// program in their home. With one desk and one person in front, that is the
/// only one that may ask it to change the machine.
fn uid_with_settings(passwd: &str, has_settings: impl Fn(&Path) -> bool) -> Option<u32> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 6 {
            return None;
        }
        let uid: u32 = fields[2].parse().ok()?;
        // The system accounts have no home worth looking at.
        if uid < 1000 {
            return None;
        }
        let home = Path::new(fields[5]);
        has_settings(&home.join(".config/soplos-gaming-panel/config.json")).then_some(uid)
    })
}

fn served_uid() -> Option<u32> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    uid_with_settings(&passwd, |settings| settings.is_file())
}

/// Total time per game as objects, which is what the window draws.
pub fn totals_json(limit: usize) -> Value {
    let list: Vec<Value> = history::totals(&config_dir(), limit)
        .into_iter()
        .map(|(name, seconds)| json!({ "name": name, "seconds": seconds }))
        .collect();
    json!(list)
}

/// What each profile would change, before it is applied: how many levers it
/// touches, and the value of every one of them.
pub fn preview() -> Value {
    let levers = governor::levers();
    let mut all = serde_json::Map::new();
    for id in governor::profile::ids() {
        let mut changes = serde_json::Map::new();
        for lever in &levers {
            if let Some(governor::profile::Action::Set(value)) =
                governor::profile::value(id, lever.key(), &lever.game_value())
            {
                changes.insert(lever.key().to_string(), json!(value));
            }
        }
        all.insert(id.to_string(), Value::Object(changes));
    }
    Value::Object(all)
}

fn ok_or_error(outcome: Result<Value, String>) -> Value {
    match outcome {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

/// Turns the automatic profile on or off, or changes which one it applies.
fn set_auto(request: &Value) -> Result<Value, String> {
    let dir = config_dir();
    let mut auto = settings::load(&dir);
    if let Some(enabled) = request["enabled"].as_bool() {
        auto.enabled = enabled;
    }
    if let Some(profile) = request["profile"].as_str() {
        auto.profile = profile.to_string();
    }
    settings::save(&dir, &auto).map_err(|error| error.to_string())?;
    Ok(json!({ "auto": auto }))
}

/// Gives one game its own profile, or removes the rule when the profile is
/// empty.
fn set_rule(request: &Value) -> Result<Value, String> {
    let dir = config_dir();
    let mut auto = settings::load(&dir);
    let name = request["name"].as_str().unwrap_or_default().trim().to_string();
    let profile = request["profile"].as_str().unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err("a game needs a name".to_string());
    }
    auto.rules.retain(|rule| rule.name != name);
    if !profile.is_empty() {
        if governor::profile::value(&profile, "cpu.governor", "performance").is_none() {
            return Err(format!("unknown profile: {profile}"));
        }
        let scheduler = request["scheduler"]
            .as_str()
            .map(str::trim)
            .filter(|scheduler| !scheduler.is_empty())
            .map(str::to_string);
        if let Some(scheduler) = &scheduler {
            if !governor::scx::valid_name(scheduler) {
                return Err(format!("not a scheduler name: {scheduler}"));
            }
        }
        let mode = request["mode"].as_u64().map(|mode| mode.min(4) as u32);
        auto.rules.push(settings::Rule { name, profile, scheduler, mode });
    }
    settings::save(&dir, &auto).map_err(|error| error.to_string())?;
    Ok(json!({ "auto": auto }))
}

/// Adds a program by hand to what counts as a game, for the ones that have no
/// desktop entry, or removes it.
fn set_manual(request: &Value) -> Result<Value, String> {
    let dir = config_dir();
    let mut auto = settings::load(&dir);
    let name = request["name"].as_str().unwrap_or_default().trim().to_string();
    if name.is_empty() || name.contains('/') || name.len() > 128 {
        return Err("a program needs a name, without a path".to_string());
    }
    let add = request["add"].as_bool().unwrap_or(true);
    auto.manual.retain(|one| one != &name);
    if add {
        auto.manual.push(name);
    }
    settings::save(&dir, &auto).map_err(|error| error.to_string())?;
    Ok(json!({ "auto": auto }))
}

/// Adds or removes a name from the list of what is never taken for a game.
fn set_exclusion(request: &Value, exclude: bool) -> Result<Value, String> {
    let dir = config_dir();
    let mut auto = settings::load(&dir);
    if let Some(name) = request["name"].as_str() {
        let name = name.trim();
        if !name.is_empty() {
            if exclude {
                if !auto.exclusions.iter().any(|one| one == name) {
                    auto.exclusions.push(name.to_string());
                }
            } else {
                auto.exclusions.retain(|one| one != name);
            }
        }
    }
    settings::save(&dir, &auto).map_err(|error| error.to_string())?;
    Ok(json!({ "auto": auto }))
}

/// Watches the games and turns the profile on and off by itself. It is the
/// whole point of the daemon: nobody wants to press a button after launching.
fn watch(state_dir: PathBuf, config: PathBuf) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let auto = settings::load(&config);
        if !auto.enabled {
            continue;
        }
        let playing = games::running(&auto.exclusions, &auto.manual);
        let active = state::load(&state_dir).is_some();
        let running = !playing.is_empty();
        let session = state::load_session(&state_dir);
        match (running, active) {
            (true, false) => {
                let game = &playing[0];
                // The profile of this game was turned off by hand while it
                // runs: it stays off until the game ends.
                if session.as_ref().is_some_and(|session| session.pid == game.pid) {
                    continue;
                }
                let profile = auto.profile_for(&game.name);
                // What sched_ext has running now, to give it back when the
                // game ends.
                let before = governor::scx::status();
                // A game started through GameMode has its governor already.
                let leave: &[&str] = if gamemode_in_use() { &["cpu.governor"] } else { &[] };
                if let Err(error) = apply_leaving(&profile, leave) {
                    eprintln!("automatic profile: {error}");
                    continue;
                }
                let mut started = None;
                if let Some(rule) = auto.rule_for(&game.name) {
                    if let Some(scheduler) = rule.scheduler.clone() {
                        match governor::scx::switch(&scheduler, rule.mode.unwrap_or(1)) {
                            Ok(()) => started = Some(scheduler),
                            Err(error) => eprintln!("automatic scheduler: {error}"),
                        }
                    }
                }
                let previous = if before.available() && before.current != "unknown" {
                    Some(before.current)
                } else {
                    None
                };
                let _ = state::save_session(
                    &state_dir,
                    &state::Session {
                        name: game.name.clone(),
                        pid: game.pid,
                        seconds: state::now(),
                        profile,
                        scheduler: started,
                        previous,
                        previous_mode: Some(before.mode),
                    },
                );
            }
            // A profile turned on by hand, with no game, stays until it is
            // turned off by hand: only the one this watcher turned on for a game
            // (the one with a session) goes away with the game.
            (false, true) if session.is_none() => {}
            (false, true) | (false, false) if session.is_some() => {
                if active {
                    if let Err(error) = restore() {
                        eprintln!("automatic restore: {error}");
                        continue;
                    }
                }
                if let Some(session) = session {
                    // The scheduler that the panel started for the game goes
                    // away, and the one that was there comes back.
                    if session.scheduler.is_some() {
                        let back = match &session.previous {
                            Some(name) => governor::scx::switch(name, session.previous_mode.unwrap_or(0)),
                            None => governor::scx::stop(),
                        };
                        if let Err(error) = back {
                            eprintln!("automatic scheduler back: {error}");
                        }
                    }
                    let played = state::now().saturating_sub(session.seconds);
                    // Under a few seconds it was not a game, it was a false
                    // positive: it is not worth a line in the history.
                    if played >= 5 {
                        let _ = history::append(
                            &config,
                            &history::Session {
                                name: session.name,
                                profile: session.profile,
                                seconds: played,
                                at: state::now(),
                            },
                        );
                    }
                }
                state::clear_session(&state_dir);
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sandbox(name: &str) -> crate::testing::Scratch {
        let dir = crate::testing::scratch("sgp-daemon", name);
        fs::create_dir_all(dir.join("cpufreq")).unwrap();
        fs::write(dir.join("cpufreq/scaling_governor"), "schedutil\n").unwrap();
        fs::write(dir.join("cpufreq/scaling_available_governors"), "schedutil performance\n").unwrap();
        dir
    }

    #[test]
    fn a_failure_leaves_the_state_so_it_can_be_tried_again() {
        let dir = sandbox("partial");
        let node = dir.join("cpufreq/scaling_governor");
        lever::write(&node, "performance").unwrap();
        let entries = vec![
            state::Entry { key: "cpu.governor".to_string(), path: node.display().to_string(), value: "schedutil".to_string() },
            state::Entry { key: "gone".to_string(), path: dir.join("gone").display().to_string(), value: "x".to_string() },
        ];
        state::save(&dir, &state::State { pid: 1, seconds: state::now(), profile: "max".to_string(), entries }).unwrap();

        // One of the two nodes is gone, so the restore reports a failure, puts
        // back what it could and keeps the file for the next restore path.
        assert!(restore_in(&dir).is_err());
        assert_eq!(lever::read(&node).unwrap(), "schedutil");
        assert!(state::load(&dir).is_some());
    }

    #[test]
    fn a_dirty_limit_that_was_not_in_use_comes_back_through_its_ratio() {
        let dir = sandbox("dirty");
        let ratio = dir.join("dirty_ratio");
        let bytes = dir.join("dirty_bytes");
        // What the game left: the bytes in use, the ratio at 0.
        fs::write(&ratio, "0\n").unwrap();
        fs::write(&bytes, "268435456\n").unwrap();
        let entries = vec![
            state::Entry { key: "vm.dirty_bytes".to_string(), path: bytes.display().to_string(), value: "0".to_string() },
            state::Entry { key: "vm.dirty_ratio".to_string(), path: ratio.display().to_string(), value: "20".to_string() },
        ];
        state::save(&dir, &state::State { pid: 1, seconds: state::now(), profile: "max".to_string(), entries }).unwrap();

        // The 0 of the bytes is never written (the kernel refuses it): the ratio
        // is, and the state goes away because nothing failed.
        assert!(restore_in(&dir).is_ok());
        assert_eq!(lever::read(&ratio).unwrap(), "20");
        assert_eq!(lever::read(&bytes).unwrap(), "268435456");
        assert!(state::load(&dir).is_none());
    }

    #[test]
    fn a_dirty_limit_in_bytes_is_written_back() {
        let entry = state::Entry { key: "vm.dirty_bytes".to_string(), path: String::new(), value: "134217728".to_string() };
        assert!(!restored_by_its_ratio(&entry));
        let other = state::Entry { key: "vm.swappiness".to_string(), path: String::new(), value: "0".to_string() };
        assert!(!restored_by_its_ratio(&other));
    }

    #[test]
    fn a_game_through_gamemode_is_told_by_its_preload() {
        assert!(preloads_gamemode(b"HOME=/home/u\0LD_PRELOAD=/usr/$LIB/libgamemodeauto.so.0\0PATH=/usr/bin\0"));
        assert!(preloads_gamemode(b"LD_PRELOAD=libmangohud.so:libgamemodeauto.so.0\0"));
        // The name somewhere else is not GameMode running the game.
        assert!(!preloads_gamemode(b"NOTE=libgamemodeauto\0LD_PRELOAD=libmangohud.so\0"));
        assert!(!preloads_gamemode(b""));
    }

    #[test]
    fn restoring_without_state_does_nothing() {
        let dir = sandbox("none");
        assert_eq!(restore_in(&dir).unwrap()["restored"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn the_user_with_the_settings_is_the_one_the_daemon_serves() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\n\
                      daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n\
                      soplos:x:1000:1000:Soplos:/home/soplos:/bin/bash\n\
                      otro:x:1001:1001:Otro:/home/otro:/bin/bash\n";
        // Only the second real user has the settings of the program.
        let found = uid_with_settings(passwd, |path| path.starts_with("/home/otro"));
        assert_eq!(found, Some(1001));
    }

    #[test]
    fn a_machine_with_no_settings_serves_nobody() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nsoplos:x:1000:1000:Soplos:/home/soplos:/bin/bash\n";
        assert_eq!(uid_with_settings(passwd, |_| false), None);
    }
}
