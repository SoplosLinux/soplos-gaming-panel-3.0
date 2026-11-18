// What the window asks about the gaming tweaks of Soplos Welcome: whether each
// one is in place, and whether this machine can have it at all.

use serde::Serialize;
use std::path::Path;
use std::process::Command;

use crate::tweaks::{self, Tweak};

/// The chips that read the temperatures, the fans and the voltages, and that
/// some kernels and some boards do not load on their own.
const SENSOR_MODULES: [&str; 3] = crate::tweaks::SENSOR_MODULES;

/// Whether the module exists for this kernel: `modinfo` answers with its file.
fn module_exists(name: &str) -> bool {
    Command::new("modinfo")
        .args(["-n", name])
        .output()
        .map(|answer| answer.status.success())
        .unwrap_or(false)
}

/// Whether it is loaded already: the kernel lists it under `/sys/module`.
fn module_loaded(name: &str) -> bool {
    Path::new("/sys/module").join(name).is_dir()
}

/// The sensor modules of this machine that are not loaded yet and could be.
/// Nothing here needs a password: it is only asking the kernel.
#[tauri::command]
pub fn sensor_modules() -> Vec<String> {
    SENSOR_MODULES
        .into_iter()
        .filter(|name| module_exists(name) && !module_loaded(name))
        .map(str::to_string)
        .collect()
}

/// Asks the kernel for them, with the one permission of the package.
#[tauri::command]
pub fn ask_sensor_modules() -> Result<(), String> {
    crate::commands::privileged::load_sensor_modules()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakState {
    pub key: String,
    pub on: bool,
    pub available: bool,
}

/// The tweaks of Welcome, with the state of each one right now.
#[tauri::command]
pub fn tweaks_status() -> Vec<TweakState> {
    let vendor = tweaks::detect_vendor();
    Tweak::all()
        .into_iter()
        .map(|tweak| TweakState {
            key: tweak.key().to_string(),
            on: tweak.on(),
            available: match tweak {
                Tweak::Gpu => vendor.is_some(),
                Tweak::PrimeRun => vendor == Some("nvidia"),
                _ => true,
            },
        })
        .collect()
}

/// The overlay tools of Welcome are Flatpaks, and this is how they are asked.
#[tauri::command]
pub fn overlay_tools_installed() -> bool {
    Command::new("flatpak")
        .args(["info", "io.github.benjamimgois.goverlay"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// RyzenAdj is compiled from source by Welcome; here only its result is read.
#[tauri::command]
pub fn ryzenadj_installed() -> bool {
    Path::new(crate::tweaks::RYZENADJ_PATH).exists()
        || Path::new("/etc/systemd/system/ryzenadj.service").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_lists_every_tweak_exactly_once() {
        let status = tweaks_status();
        assert_eq!(status.len(), Tweak::all().len());
        for tweak in Tweak::all() {
            let times = status.iter().filter(|one| one.key == tweak.key()).count();
            assert_eq!(times, 1, "{} appears {times} times", tweak.key());
        }
    }
}
