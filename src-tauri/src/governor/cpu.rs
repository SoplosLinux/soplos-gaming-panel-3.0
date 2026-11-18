// CPU frequency governor, the first lever of the profile.
//
// Each core, or each group of cores, has a cpufreq policy of its own, so the
// governor is read from cpu0 and written to every one of them. If the core has
// no cpufreq the lever is not built at all: a virtual machine without cpufreq,
// for one.

use super::lever::{self, Lever};
use std::path::{Path, PathBuf};

pub struct CpuGovernor {
    governor: PathBuf,
    available: PathBuf,
}

impl CpuGovernor {
    /// The lever as this machine has it, or nothing when there is no cpufreq.
    pub fn new() -> Option<Self> {
        let root = std::env::var("SOPLOS_CPUFREQ_ROOT").unwrap_or_else(|_| "/sys/devices/system/cpu".to_string());
        Self::at(Path::new(&root).join("cpu0/cpufreq"))
    }

    /// The same, on any directory shaped like cpu0/cpufreq. Tests point it at a
    /// temporary one so nothing touches the real machine.
    pub fn at(cpufreq: PathBuf) -> Option<Self> {
        let governor = cpufreq.join("scaling_governor");
        let available = cpufreq.join("scaling_available_governors");
        governor.exists().then_some(Self { governor, available })
    }
}

/// One file of cpufreq for every core that has it, each policy once: cores
/// that share a policy point to the same folder.
pub fn every_core_in(root: &Path, file: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.strip_prefix("cpu").is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|entry| entry.path().join("cpufreq").join(file))
        .filter(|path| path.exists())
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    found.sort();
    found.dedup();
    found
}

impl Lever for CpuGovernor {
    fn key(&self) -> &'static str {
        "cpu.governor"
    }

    fn group(&self) -> &'static str {
        "cpu"
    }

    fn path(&self) -> &Path {
        &self.governor
    }

    fn paths(&self) -> Vec<PathBuf> {
        // The folder of the cores is the one above cpu0/cpufreq.
        let all = self
            .governor
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|root| every_core_in(root, "scaling_governor"))
            .unwrap_or_default();
        if all.is_empty() {
            vec![self.governor.clone()]
        } else {
            all
        }
    }

    fn game_value(&self) -> String {
        "performance".to_string()
    }

    fn allowed(&self) -> Option<Vec<String>> {
        lever::read(&self.available).ok().map(|value| lever::list(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch;
    use std::fs;

    fn fake(with_governor: bool, name: &str) -> crate::testing::Scratch {
        let dir = scratch("sgp-cpu", name);
        if with_governor {
            fs::write(dir.join("scaling_governor"), "schedutil\n").unwrap();
            fs::write(dir.join("scaling_available_governors"), "schedutil performance powersave\n").unwrap();
        }
        dir
    }

    #[test]
    fn without_cpufreq_there_is_no_lever() {
        let dir = fake(false, "none");
        assert!(CpuGovernor::at(dir.to_path_buf()).is_none());
    }

    #[test]
    fn reads_the_governor_and_what_it_accepts() {
        let dir = fake(true, "read");
        let lever = CpuGovernor::at(dir.to_path_buf()).unwrap();
        assert_eq!(lever.key(), "cpu.governor");
        assert_eq!(lever::read(lever.path()).unwrap(), "schedutil");
        assert_eq!(lever.game_value(), "performance");
        assert_eq!(
            lever.allowed().unwrap(),
            vec!["schedutil".to_string(), "performance".to_string(), "powersave".to_string()]
        );
    }

    #[test]
    fn the_governor_is_written_to_every_core() {
        let root = scratch("sgp-cpu", "cores");
        for core in ["cpu0", "cpu1", "cpu2", "cpufreq", "cpuidle"] {
            fs::create_dir_all(root.join(core).join("cpufreq")).unwrap();
        }
        for core in ["cpu0", "cpu1", "cpu2"] {
            fs::write(root.join(core).join("cpufreq/scaling_governor"), "schedutil\n").unwrap();
        }
        let lever = CpuGovernor::at(root.join("cpu0/cpufreq")).unwrap();
        let paths = lever.paths();
        assert_eq!(paths.len(), 3, "cpu0, cpu1 and cpu2, not cpufreq nor cpuidle");
        assert!(paths.iter().all(|path| path.ends_with("cpufreq/scaling_governor")));
    }

    #[test]
    fn writing_and_restoring_leaves_the_node_as_it_was() {
        let dir = fake(true, "write");
        let lever = CpuGovernor::at(dir.to_path_buf()).unwrap();
        let original = lever::read(lever.path()).unwrap();
        lever::write(lever.path(), &lever.game_value()).unwrap();
        assert_eq!(lever::read(lever.path()).unwrap(), "performance");
        lever::write(lever.path(), &original).unwrap();
        assert_eq!(lever::read(lever.path()).unwrap(), "schedutil");
    }
}
