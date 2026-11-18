// The gaming tweaks of Soplos Welcome, with the same files and the same
// criterion. The difference is who writes them: the root side of the panel,
// with named operations, instead of a script sent through pkexec.
//
// The window never names a path. It asks for a tweak by name and the root side
// knows where it goes.

use std::fs;
use std::path::Path;

pub const SYSCTL_PATH: &str = "/etc/sysctl.d/99-soplos-gaming.conf";

pub const SYSCTL_CONF: &str = r#"# Soplos Gaming Sysctl Profile
# Adapted from CachyOS-Settings

# === MEMORY ===
vm.max_map_count = 2147483642
vm.swappiness = 10
vm.page-cluster = 0

# === I/O AND CACHE ===
vm.vfs_cache_pressure = 50
vm.dirty_bytes = 268435456
vm.dirty_background_bytes = 134217728
vm.dirty_writeback_centisecs = 1500

# === NETWORK ===
net.ipv4.tcp_fastopen = 3
net.core.netdev_max_backlog = 16384
net.core.rmem_default = 1048576
net.core.rmem_max = 16777216
net.core.wmem_default = 1048576
net.core.wmem_max = 16777216

# === SYSTEM ===
fs.file-max = 2097152
kernel.nmi_watchdog = 0
kernel.split_lock_mitigate = 0
"#;

pub const SCHEDULERS_PATH: &str = "/etc/udev/rules.d/60-soplos-ioschedulers.rules";

pub const SCHEDULERS_RULES: &str = r#"# Soplos I/O Schedulers Gaming Optimization
ACTION=="add|change", KERNEL=="sd[a-z]", ATTR{queue/rotational}=="1", ATTR{queue/scheduler}="bfq"
ACTION=="add|change", KERNEL=="sd[a-z]", ATTR{queue/rotational}=="0", ATTR{queue/scheduler}="mq-deadline"
ACTION=="add|change", KERNEL=="nvme[0-9]n[0-9]", ATTR{queue/scheduler}="none"
"#;

pub const PERFORMANCE_PATH: &str = "/usr/local/bin/soplos-game-performance";

pub const PERFORMANCE_SCRIPT: &str = r#"#!/usr/bin/env bash
# Soplos Gaming Performance Wrapper
# Temporarily switches the power profile to "performance" while a game runs.

if ! command -v powerprofilesctl &>/dev/null; then
    echo "Error: powerprofilesctl not found" >&2
    echo "Install: apt install power-profiles-daemon" >&2
    exit 1
fi

if ! powerprofilesctl list | grep -q 'performance:'; then
    echo "Warning: your CPU does not support the performance profile" >&2
    echo "Running without power profile optimization..." >&2
    exec "$@"
fi

exec systemd-inhibit --why "Soplos Gaming Mode active" powerprofilesctl launch \
    -p performance -r "Launched with Soplos Gaming Performance" -- "$@"
"#;

pub const PRIME_RUN_PATH: &str = "/usr/local/bin/prime-run";pub const PRIME_RUN_SCRIPT: &str =
    "#!/bin/sh\n# NVIDIA PRIME Render Offload Script\nexec env __NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia \"$@\"\n";

pub const AMD_ENV_CONF: &str =
    "RADV_PERFTEST=gpl\nAMD_VULKAN_ICD=RADV\nmesa_glthread=true\nMESA_LOADER_DRIVER_OVERRIDE=radeonsi\n";
pub const INTEL_ENV_CONF: &str = "mesa_glthread=true\nMESA_LOADER_DRIVER_OVERRIDE=iris\n";
pub const NVIDIA_ENV_CONF: &str =
    "__GL_SHADER_DISK_CACHE=1\n__GL_THREADED_OPTIMIZATION=1\n__GL_SYNC_TO_VBLANK=0\nPROTON_ENABLE_NVAPI=1\n";

/// Where Welcome leaves the RyzenAdj it compiles, so this program can say
/// whether the machine has it already. Building it is Welcome's business.
pub const RYZENADJ_PATH: &str = "/usr/local/bin/ryzenadj";

/// The chips that read temperatures, fans and voltages, and that some kernels
/// and some boards do not load on their own. Only these may be asked for.
pub const SENSOR_MODULES: [&str; 3] = ["coretemp", "k10temp", "nct6775"];

/// What Soplos Welcome calls an optimization. The window asks for one of these
/// by name; the paths live here and nowhere else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tweak {
    /// The sysctl profile of CachyOS.
    Sysctl,
    /// The variables of the graphic driver in /etc/environment.d.
    Gpu,
    /// The udev rules that pick an I/O scheduler per disk.
    Schedulers,
    /// The wrapper that asks for the performance power profile.
    Performance,
    /// The PRIME render offload script of a hybrid NVIDIA laptop.
    PrimeRun,
}

impl Tweak {
    pub fn all() -> [Tweak; 5] {
        [Tweak::Sysctl, Tweak::Gpu, Tweak::Schedulers, Tweak::Performance, Tweak::PrimeRun]
    }

    pub fn key(self) -> &'static str {
        match self {
            Tweak::Sysctl => "sysctl",
            Tweak::Gpu => "gpu",
            Tweak::Schedulers => "schedulers",
            Tweak::Performance => "performance",
            Tweak::PrimeRun => "prime_run",
        }
    }

    pub fn from_key(key: &str) -> Option<Tweak> {
        Tweak::all().into_iter().find(|tweak| tweak.key() == key)
    }

    /// Whether the file of the tweak is already in place.
    pub fn on(self) -> bool {
        match self {
            Tweak::Gpu => gpu_conf_path().is_some(),
            Tweak::Sysctl => Path::new(SYSCTL_PATH).exists(),
            Tweak::Schedulers => Path::new(SCHEDULERS_PATH).exists(),
            Tweak::Performance => Path::new(PERFORMANCE_PATH).exists(),
            Tweak::PrimeRun => Path::new(PRIME_RUN_PATH).exists(),
        }
    }
}

/// The vendor of a graphic card, as its PCI identifier says.
pub fn vendor_of(pci_id: u32) -> Option<&'static str> {
    match pci_id {
        0x10de => Some("nvidia"),
        0x1002 => Some("amd"),
        0x8086 => Some("intel"),
        _ => None,
    }
}

fn reads_id(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let digits = text.trim().trim_start_matches("0x");
    u32::from_str_radix(digits, 16).ok()
}

/// The vendor of the first card of the machine, or none if there is no card.
pub fn detect_vendor() -> Option<&'static str> {
    let entries = fs::read_dir("/sys/class/drm").ok()?;
    let mut found: Vec<&'static str> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("card"))
        .filter(|entry| !entry.file_name().to_string_lossy().contains('-'))
        .filter_map(|entry| reads_id(&entry.path().join("device/vendor")).and_then(vendor_of))
        .collect();
    found.sort_by_key(|vendor| match *vendor {
        "nvidia" => 0,
        "amd" => 1,
        _ => 2,
    });
    found.first().copied()
}

pub fn gpu_conf_for(vendor: &str) -> (&'static str, &'static str) {
    match vendor {
        "nvidia" => ("/etc/environment.d/50-soplos-nvidia-gaming.conf", NVIDIA_ENV_CONF),
        "amd" => ("/etc/environment.d/50-soplos-amd-gaming.conf", AMD_ENV_CONF),
        _ => ("/etc/environment.d/50-soplos-intel-gaming.conf", INTEL_ENV_CONF),
    }
}

/// The file of the graphic tweak that is already written, whatever the vendor.
pub fn gpu_conf_path() -> Option<&'static str> {
    ["nvidia", "amd", "intel"]
        .into_iter()
        .map(gpu_conf_for)
        .map(|(path, _)| path)
        .find(|path| Path::new(path).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tweak_has_a_key_and_comes_back_from_it() {
        for tweak in Tweak::all() {
            assert_eq!(Tweak::from_key(tweak.key()), Some(tweak));
        }
        assert_eq!(Tweak::from_key("nonsense"), None);
    }

    #[test]
    fn the_pci_identifier_names_the_vendor() {
        assert_eq!(vendor_of(0x10de), Some("nvidia"));
        assert_eq!(vendor_of(0x1002), Some("amd"));
        assert_eq!(vendor_of(0x8086), Some("intel"));
        assert_eq!(vendor_of(0x1234), None);
    }

    #[test]
    fn each_vendor_gets_its_own_file_and_variables() {
        assert!(gpu_conf_for("amd").0.ends_with("50-soplos-amd-gaming.conf"));
        assert!(gpu_conf_for("amd").1.contains("RADV_PERFTEST"));
        assert!(gpu_conf_for("nvidia").1.contains("PROTON_ENABLE_NVAPI"));
        assert!(gpu_conf_for("intel").1.contains("iris"));
    }

    #[test]
    fn the_sysctl_profile_is_all_assignments_of_numbers() {
        for line in SYSCTL_CONF.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').expect("every line is an assignment");
            assert!(!key.trim().is_empty());
            assert!(value.trim().parse::<u64>().is_ok());
        }
    }
}
