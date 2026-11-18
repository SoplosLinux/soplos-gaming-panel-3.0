// The window side of the governor daemon.
//
// The daemon runs as the system service of the package and answers over a
// socket in /run. The window never escalates privileges: it only asks for the
// three named operations, and the daemon decides what they mean.

use crate::daemon::{self, state};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

fn ask(op: &str) -> Result<Value, String> {
    send(&json!({ "op": op }).to_string())
}

fn send(request: &str) -> Result<Value, String> {
    let path = daemon::socket(&state::root());
    let mut stream =
        UnixStream::connect(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    stream
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|error| error.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    let answer: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
    if answer["ok"] == Value::Bool(true) {
        Ok(answer["result"].clone())
    } else {
        Err(answer["error"].as_str().unwrap_or("the daemon refused the request").to_string())
    }
}

/// What the governor has now: whether a profile is on, and which levers this
/// kernel has.
#[tauri::command]
pub fn governor_status() -> Result<Value, String> {
    ask("status")
}

/// The levers of this machine, read in this process. The page has something to
/// show even with the service stopped, which is also how the user finds out
/// what is missing before turning it on.
#[tauri::command]
pub fn governor_catalogue() -> Value {
    let mut value = crate::governor::catalogue_json();
    // What each profile would change and what has been played are read here
    // too, so the page has them with the service stopped.
    value["preview"] = crate::daemon::preview();
    value["totals"] = crate::daemon::totals_json(5);
    value
}

/// The kernels of this machine, read from the disk, with the one that runs
/// marked. Building or installing them is the job of Kernel Installer.
#[tauri::command]
pub fn kernels_status() -> Value {
    let kernels = crate::governor::kernels::installed();
    json!({
        "running": crate::governor::kernels::running(),
        "kernels": kernels,
    })
}

#[tauri::command]
pub fn governor_apply(profile: String) -> Result<Value, String> {
    send(&json!({ "op": "apply", "profile": profile }).to_string())
}

/// Adds one game by hand to what counts as a game, or removes it.
#[tauri::command]
pub fn governor_manual(name: String, add: bool) -> Result<Value, String> {
    send(&json!({ "op": "manual", "name": name, "add": add }).to_string())
}

#[tauri::command]
pub fn governor_restore() -> Result<Value, String> {
    ask("restore")
}

/// Turns the automatic profile on or off and chooses which one it applies.
#[tauri::command]
pub fn governor_auto(enabled: bool, profile: String) -> Result<Value, String> {
    send(&json!({ "op": "auto", "enabled": enabled, "profile": profile }).to_string())
}

/// Adds or removes a process name from the list of what is never a game.
#[tauri::command]
pub fn governor_exclusion(name: String, exclude: bool) -> Result<Value, String> {
    let op = if exclude { "exclude" } else { "include" };
    send(&json!({ "op": op, "name": name }).to_string())
}

/// Gives one game its own profile and, if it wants one, its scheduler; an
/// empty profile removes the rule.
#[tauri::command]
pub fn governor_rule(
    name: String,
    profile: String,
    scheduler: Option<String>,
    mode: Option<u32>,
) -> Result<Value, String> {
    send(
        &json!({
            "op": "rule",
            "name": name,
            "profile": profile,
            "scheduler": scheduler,
            "mode": mode,
        })
        .to_string(),
    )
}

/// The schedulers of sched_ext as this machine has them. Reading it never
/// asks for a password: it is the kernel and a service talking.
#[tauri::command]
pub fn scx_status() -> crate::governor::scx::Scx {
    crate::governor::scx::status()
}

/// Starts or swaps the scheduler of sched_ext, or lets go of it with no name.
#[tauri::command]
pub fn scx_set(scheduler: Option<String>, mode: u32) -> Result<(), String> {
    match scheduler {
        Some(name) => crate::governor::scx::switch(&name, mode),
        None => crate::governor::scx::stop(),
    }
}
