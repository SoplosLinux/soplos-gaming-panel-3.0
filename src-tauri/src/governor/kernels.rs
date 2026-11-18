// The kernels this machine has, read from the disk, and whether each one can
// run what the game mode needs. Nothing is copied or built here: the panel
// only looks at /boot and at the kernel that is running.
//
// Building and installing kernels belongs to Soplos Kernel Installer.

use serde::Serialize;
use std::fs;

/// A kernel that is on disk, with what matters for the game mode.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KernelInfo {
    /// What the file is called after `vmlinuz-`.
    pub version: String,
    /// The one that is running right now.
    pub active: bool,
    /// `CONFIG_SCHED_CLASS_EXT`: the extended scheduler of the kernel.
    pub sched_ext: Option<bool>,
    /// `DEBUG_INFO_BTF`: sched_ext does not work without it.
    pub btf: Option<bool>,
}

/// Whether a kernel configuration file turns an option on.
pub fn has_option(config: &str, name: &str) -> Option<bool> {
    let yes = format!("{name}=y");
    let no = format!("{name}=n");
    let mut found = None;
    for line in config.lines() {
        let line = line.trim();
        if line == yes {
            return Some(true);
        }
        if line == no {
            found = Some(false);
        }
    }
    found
}

/// The kernel of the running system, as the kernel itself says.
pub fn running() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_string())
}

fn config_of(version: &str) -> Option<String> {
    fs::read_to_string(format!("/boot/config-{version}")).ok()
}

/// Every kernel of `/boot`, the running one marked, and what each one can do.
pub fn installed() -> Vec<KernelInfo> {
    let current = running();
    let entries = match fs::read_dir("/boot") {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut kernels: Vec<KernelInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let version = name.strip_prefix("vmlinuz-")?.to_string();
            let config = config_of(&version);
            Some(KernelInfo {
                active: current.as_deref() == Some(version.as_str()),
                sched_ext: config.as_deref().and_then(|text| has_option(text, "CONFIG_SCHED_CLASS_EXT")),
                btf: config.as_deref().and_then(|text| has_option(text, "CONFIG_DEBUG_INFO_BTF")),
                version,
            })
        })
        .collect();

    kernels.sort_by(|one, other| other.version.cmp(&one.version));
    kernels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_option_is_read_from_the_configuration() {
        let config = "CONFIG_SCHED_CLASS_EXT=y\nDEBUG_INFO_BTF=n\n";
        assert_eq!(has_option(config, "CONFIG_SCHED_CLASS_EXT"), Some(true));
        assert_eq!(has_option(config, "DEBUG_INFO_BTF"), Some(false));
        assert_eq!(has_option(config, "CONFIG_CGROUP_DMEM"), None);
    }

    #[test]
    fn an_option_commented_out_is_not_turned_on() {
        let config = "# CONFIG_SCHED_CLASS_EXT is not set\n";
        assert_eq!(has_option(config, "CONFIG_SCHED_CLASS_EXT"), None);
    }

    #[test]
    fn this_machine_always_has_a_running_kernel() {
        let running = running().expect("the kernel of this machine");
        assert!(!running.is_empty());
    }
}
