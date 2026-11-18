// What needs the administrator: installing GameMode and MangoHud when Soplos
// Welcome has not done it yet.
//
// The window never runs as root and never sends a script. It writes a request
// with named operations, and pkexec starts this same program again as
// `--run-ops <file>` under the polkit action of the package. That second run
// never opens a window: it checks everything again as root, because the file
// can be written by any user, and only then does the work.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::tweaks::{self, Tweak};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// The fallback installation, with the same packages as the Gaming
    /// Center of Soplos Welcome.
    InstallPackages {
        apt: Vec<String>,
        #[serde(default)]
        flatpak: Vec<String>,
    },
    /// Starts or stops the governor service. Only this one unit, by name.
    Service { start: bool },
    /// Writes or removes one of the gaming tweaks of Soplos Welcome, and
    /// reloads what has to be reloaded. The window sends the name of the
    /// tweak, never a path: the root side owns the files.
    Tweak { which: String, on: bool },
    /// Asks the kernel for the chips that read temperatures, fans and
    /// voltages, when the machine does not load them on its own.
    Modules { names: Vec<String> },
}

#[derive(Serialize, Deserialize)]
struct Request {
    ops: Vec<Op>,
}

// ── User side ────────────────────────────────────────────────────────────────

fn cache_root() -> PathBuf {
    std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("soplos-gaming-panel")
}

fn command_error(result: std::process::Output) -> String {
    let said = String::from_utf8_lossy(&result.stderr).trim().to_string();
    if said.is_empty() {
        "The administrator step did not finish".to_string()
    } else {
        said
    }
}

/// Asks pkexec to run the operations as root. One call is one password.
pub fn run_as_root(ops: &[Op]) -> Result<(), String> {
    if ops.is_empty() {
        return Ok(());
    }
    let dir = cache_root();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("request-{}.json", std::process::id()));

    let request = serde_json::to_string(&Request { ops: ops.to_vec() }).map_err(|e| e.to_string())?;
    {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&file)
            .map_err(|e| e.to_string())?;
        out.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let output = Command::new("pkexec").arg(exe).arg("--run-ops").arg(&file).output();
    let _ = fs::remove_file(&file);

    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(match (String::from_utf8_lossy(&o.stderr).trim().is_empty(), o.status.code()) {
            (false, _) => command_error(o),
            (true, Some(126)) => "The administrator password was not given".to_string(),
            _ => "The administrator step did not finish".to_string(),
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// The fallback installer of the plan: what the panel needs to control the
/// game, installed with the same criterion as Soplos Welcome.
#[tauri::command]
pub fn install_gaming_tools() -> Result<(), String> {
    run_as_root(&[Op::InstallPackages {
        apt: vec!["gamemode".to_string(), "mangohud".to_string()],
        flatpak: Vec::new(),
    }])
}

/// Installs one package asked for by a lever of the governor. The names come
/// from the window but are checked again as root, and only plain package names
/// are accepted.
#[tauri::command]
pub fn install_packages(apt: Vec<String>, flatpak: Vec<String>) -> Result<(), String> {
    run_as_root(&[Op::InstallPackages { apt, flatpak }])
}

/// Puts one gaming tweak of Soplos Welcome in place, or takes it away.
#[tauri::command]
pub fn tweak_set(which: String, on: bool) -> Result<(), String> {
    Tweak::from_key(&which).ok_or_else(|| format!("Unknown tweak: {which}"))?;
    run_as_root(&[Op::Tweak { which, on }])
}

/// Takes every gaming tweak away, in one password.
#[tauri::command]
pub fn tweaks_revert_all() -> Result<(), String> {
    let ops: Vec<Op> = Tweak::all()
        .into_iter()
        .map(|tweak| Op::Tweak { which: tweak.key().to_string(), on: false })
        .collect();
    run_as_root(&ops)
}

/// What the plain view does with one button: the four tweaks that make the
/// machine ready to play, in one password.
#[tauri::command]
pub fn tweaks_apply_gaming() -> Result<(), String> {
    let ops: Vec<Op> = [Tweak::Sysctl, Tweak::Gpu, Tweak::Schedulers, Tweak::Performance]
        .into_iter()
        .map(|tweak| Op::Tweak { which: tweak.key().to_string(), on: true })
        .collect();
    run_as_root(&ops)
}

/// The overlay tools of Welcome, with the same Flatpaks it installs.
#[tauri::command]
pub fn install_overlay_tools() -> Result<(), String> {    let flatpak = [
        "io.github.benjamimgois.goverlay//stable",
        "org.freedesktop.Platform.VulkanLayer.MangoHud//25.08",
        "org.freedesktop.Platform.VulkanInfo//25.08",
        "org.freedesktop.Platform.VulkanLayer.gamescope//25.08",
        "org.winehq.Wine.DLLs.dxvk//stable-23.08",
        "org.freedesktop.Platform.VulkanLayer.vkBasalt//25.08",
    ]
    .iter()
    .map(|name| name.to_string())
    .collect();
    run_as_root(&[Op::InstallPackages { apt: Vec::new(), flatpak }])
}

/// Asks the kernel for the chips that read temperatures, fans and voltages,
/// when the machine does not load them on its own. Only the three of the list.
#[tauri::command]
pub fn load_sensor_modules() -> Result<(), String> {
    let names = tweaks::SENSOR_MODULES.iter().map(|name| name.to_string()).collect();
    run_as_root(&[Op::Modules { names }])
}

// ── Root side ────────────────────────────────────────────────────────────────

// The home folder of a user, read from the password file.
fn home_of(uid: u32) -> Option<PathBuf> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() >= 6 && fields[2].parse::<u32>().ok() == Some(uid)).then(|| PathBuf::from(fields[5]))
    })
}

// Only plain package names: no options, no paths, no spaces.
fn valid_package(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | ':' | '_'))
}

fn user_cache(uid: u32) -> Result<PathBuf, String> {
    let home = home_of(uid).ok_or("Unknown user")?;
    home.join(".cache").join("soplos-gaming-panel").canonicalize().map_err(|e| e.to_string())
}

fn write_root_file(path: &str, contents: &str, mode: u32) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, contents).map_err(|e| e.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())
}

fn remove_if_present(path: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Where the copy of what was there before is kept. One copy, the first one:
/// it is what the machine had, not what this program wrote.
const BACKUP_DIR: &str = "/var/lib/soplos-gaming-panel/backup";

fn backup_of(root: &Path, path: &str) -> PathBuf {
    root.join(path.trim_start_matches('/'))
}

fn keep_copy(path: &str) -> Result<(), String> {
    keep_copy_in(Path::new(BACKUP_DIR), path)
}

fn give_back(path: &str) -> Result<(), String> {
    give_back_in(Path::new(BACKUP_DIR), path)
}

/// Keeps a copy of the file before it is touched. A second call for the same
/// file does nothing: the copy that matters is the one from the first time.
fn keep_copy_in(root: &Path, path: &str) -> Result<(), String> {
    let source = Path::new(path);
    if !source.is_file() {
        return Ok(());
    }
    let copy = backup_of(root, path);
    if copy.exists() {
        return Ok(());
    }
    if let Some(parent) = copy.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(source, &copy)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", copy.display()))
}

/// Puts back what was there before the tweak, or leaves no file at all when
/// there was nothing to keep.
fn give_back_in(root: &Path, path: &str) -> Result<(), String> {
    let copy = backup_of(root, path);
    if !copy.exists() {
        return remove_if_present(path);
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&copy, path).map_err(|e| format!("{}: {e}", path))?;
    let _ = fs::remove_file(&copy);
    Ok(())
}

fn run_program(program: &str, args: &[&str]) -> Result<(), String> {
    let result = Command::new(program).args(args).output().map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        Err(command_error(result))
    }
}

/// Puts one of the gaming tweaks of Soplos Welcome in place, or takes it away,
/// and reloads only what that tweak needs. Before a file is touched, what was
/// there is kept, and taking the tweak away gives it back.
fn run_tweak(tweak: Tweak, on: bool) -> Result<(), String> {
    match tweak {
        Tweak::Sysctl => {
            if on {
                keep_copy(tweaks::SYSCTL_PATH)?;
                write_root_file(tweaks::SYSCTL_PATH, tweaks::SYSCTL_CONF, 0o644)?;
            } else {
                give_back(tweaks::SYSCTL_PATH)?;
            }
            run_program("sysctl", &["--system"])
        }
        Tweak::Schedulers => {
            if on {
                keep_copy(tweaks::SCHEDULERS_PATH)?;
                write_root_file(tweaks::SCHEDULERS_PATH, tweaks::SCHEDULERS_RULES, 0o644)?;
            } else {
                give_back(tweaks::SCHEDULERS_PATH)?;
            }
            run_program("udevadm", &["control", "--reload-rules"])?;
            // Only the disks: a trigger of every device makes the network and
            // everything else go through their rules again.
            run_program("udevadm", &["trigger", "--subsystem-match=block", "--action=change"])
        }
        Tweak::Performance => {
            if on {
                keep_copy(tweaks::PERFORMANCE_PATH)?;
                write_root_file(tweaks::PERFORMANCE_PATH, tweaks::PERFORMANCE_SCRIPT, 0o755)
            } else {
                give_back(tweaks::PERFORMANCE_PATH)
            }
        }
        Tweak::PrimeRun => {
            if on {
                keep_copy(tweaks::PRIME_RUN_PATH)?;
                write_root_file(tweaks::PRIME_RUN_PATH, tweaks::PRIME_RUN_SCRIPT, 0o755)
            } else {
                give_back(tweaks::PRIME_RUN_PATH)
            }
        }
        Tweak::Gpu => {
            if !on {
                for vendor in ["nvidia", "amd", "intel"] {
                    give_back(tweaks::gpu_conf_for(vendor).0)?;
                }
                return Ok(());
            }
            // The card is known before anything is removed: a machine whose
            // graphic card cannot be named keeps what it had.
            let vendor = tweaks::detect_vendor().ok_or("No supported graphic card was found")?;
            for other in ["nvidia", "amd", "intel"] {
                if other == vendor {
                    continue;
                }
                let (path, _) = tweaks::gpu_conf_for(other);
                give_back(path)?;
            }
            let (path, contents) = tweaks::gpu_conf_for(vendor);
            keep_copy(path)?;
            write_root_file(path, contents, 0o644)
        }
    }
}

fn run_op(op: &Op) -> Result<(), String> {
    match op {
        Op::Tweak { which, on } => {
            let tweak = Tweak::from_key(which).ok_or_else(|| format!("Unknown tweak: {which}"))?;
            run_tweak(tweak, *on)
        }
        Op::Modules { names } => {
            for name in names {
                if !tweaks::SENSOR_MODULES.contains(&name.as_str()) {
                    return Err(format!("Not a sensor module: {name}"));
                }
                run_program("modprobe", &[name])?;
            }
            Ok(())
        }
        Op::InstallPackages { apt, flatpak } => {
            for name in apt.iter().chain(flatpak.iter()) {
                if !valid_package(name) {
                    return Err(format!("Not a valid package name: {name}"));
                }
            }
            if !apt.is_empty() {
                let result = Command::new("apt-get")
                    .env("DEBIAN_FRONTEND", "noninteractive")
                    .args(["install", "-y", "--no-install-recommends"])
                    .args(apt)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !result.status.success() {
                    return Err(command_error(result));
                }
            }
            if !flatpak.is_empty() {
                let result = Command::new("flatpak")
                    .args(["install", "-y", "--noninteractive", "flathub"])
                    .args(flatpak)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !result.status.success() {
                    return Err(command_error(result));
                }
            }
            Ok(())
        }
        Op::Service { start } => {
            let action = if *start { "enable" } else { "disable" };
            let result = Command::new("systemctl")
                .args([action, "--now", "soplos-gaming-panel.service"])
                .output()
                .map_err(|e| e.to_string())?;
            if result.status.success() {
                Ok(())
            } else {
                Err(command_error(result))
            }
        }
    }
}

/// Called from main() before Tauri starts when pkexec starts the program again.
/// Returns the exit code.
pub fn run_ops_file(path: &str) -> i32 {
    let uid = match std::env::var("PKEXEC_UID").ok().and_then(|v| v.parse::<u32>().ok()) {
        Some(uid) => uid,
        None => {
            eprintln!("This mode is only for pkexec");
            return 2;
        }
    };
    let result = (|| -> Result<(), String> {
        let cache = user_cache(uid)?;
        let file = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
        let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
        // The request has to be a file of the user who asked, in their cache.
        if !file.starts_with(&cache) || !meta.is_file() || meta.uid() != uid {
            return Err("The request is not valid".to_string());
        }
        let request: Request =
            serde_json::from_str(&fs::read_to_string(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for op in &request.ops {
            run_op(op)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> crate::testing::Scratch {
        crate::testing::scratch("sgp-copias", name)
    }

    #[test]
    fn what_was_there_before_comes_back_after() {
        let folder = scratch("vuelta");
        let file = folder.join("etc/ambiente.conf");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "lo de antes\n").unwrap();

        keep_copy_in(&folder, &file.display().to_string()).unwrap();
        fs::write(&file, "lo del programa\n").unwrap();
        give_back_in(&folder, &file.display().to_string()).unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "lo de antes\n");
    }

    #[test]
    fn a_file_that_was_not_there_is_not_left_behind() {
        let folder = scratch("vacio");
        let file = folder.join("etc/nuevo.conf");
        fs::create_dir_all(file.parent().unwrap()).unwrap();

        keep_copy_in(&folder, &file.display().to_string()).unwrap();
        fs::write(&file, "puesto por el programa\n").unwrap();
        give_back_in(&folder, &file.display().to_string()).unwrap();

        assert!(!file.exists());
    }

    #[test]
    fn the_first_copy_is_the_one_that_stays() {
        let folder = scratch("primera");
        let file = folder.join("etc/ambiente.conf");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "original\n").unwrap();
        keep_copy_in(&folder, &file.display().to_string()).unwrap();

        fs::write(&file, "segunda escritura\n").unwrap();
        keep_copy_in(&folder, &file.display().to_string()).unwrap();
        give_back_in(&folder, &file.display().to_string()).unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "original\n");
    }
}
