// The levers this kernel and this machine actually have, in the order the
// profile applies them, and the ones it lacks with the reason and the way out.
//
// A lever missing from the first list is missing in the kernel or in the
// hardware: the window shows it as unavailable instead of offering a switch
// that would do nothing.

pub mod cpu;
pub mod games;
pub mod kernels;
pub mod lever;
pub mod node;
pub mod profile;
pub mod scx;

use lever::{Fix, Lever, Missing};
use std::path::PathBuf;

pub fn levers() -> Vec<Box<dyn Lever>> {
    catalogue().0
}

pub fn catalogue() -> (Vec<Box<dyn Lever>>, Vec<Missing>) {
    let mut levers: Vec<Box<dyn Lever>> = Vec::new();
    let mut missing: Vec<Missing> = Vec::new();

    match cpu::CpuGovernor::new() {
        Some(governor) => levers.push(Box::new(governor)),
        None => missing.push(Missing {
            key: "cpu.governor",
            group: "cpu",
            reason: "no_cpufreq",
            fix: Fix::Hardware,
        }),
    }

    let cpu = cpufreq_root();
    // Every core has its own energy preference, as it has its own governor.
    let epp = cpu.join("energy_performance_preference");
    if node::exists(&epp) {
        levers.push(Box::new(node::Node::per_core("cpu.epp", "cpu", epp, "energy_performance_preference", "performance")));
    } else {
        missing.push(Missing { key: "cpu.epp", group: "cpu", reason: "no_epp", fix: Fix::Hardware });
    }
    add(
        &mut levers,
        &mut missing,
        spec(
            "cpu.pstate",
            "cpu",
            "/sys/devices/system/cpu/amd_pstate/status",
            "active",
            "no_pstate",
            Fix::Kernel,
        ),
    );
    match x3d_node() {
        Some(path) => levers.push(Box::new(node::Node::new("cpu.x3d", "cpu", path, "cache"))),
        None => missing.push(Missing {
            key: "cpu.x3d",
            group: "cpu",
            reason: "no_x3d",
            fix: Fix::Hardware,
        }),
    }
    add(
        &mut levers,
        &mut missing,
        spec(
            "kernel.thp",
            "memory",
            "/sys/kernel/mm/transparent_hugepage/enabled",
            "always",
            "no_thp",
            Fix::Kernel,
        ),
    );
    add(
        &mut levers,
        &mut missing,
        spec("vm.swappiness", "memory", "/proc/sys/vm/swappiness", "10", "no_sysctl", Fix::Kernel),
    );
    add(
        &mut levers,
        &mut missing,
        spec(
            "vm.vfs_cache_pressure",
            "memory",
            "/proc/sys/vm/vfs_cache_pressure",
            "50",
            "no_sysctl",
            Fix::Kernel,
        ),
    );
    // 256 MB of dirty pages before the kernel starts writing them back.
    add(
        &mut levers,
        &mut missing,
        spec("vm.dirty_bytes", "memory", "/proc/sys/vm/dirty_bytes", "268435456", "no_sysctl", Fix::Kernel),
    );
    add(
        &mut levers,
        &mut missing,
        spec("kernel.nmi_watchdog", "kernel", "/proc/sys/kernel/nmi_watchdog", "0", "no_sysctl", Fix::Kernel),
    );
    add(
        &mut levers,
        &mut missing,
        spec(
            "kernel.split_lock_mitigate",
            "kernel",
            "/proc/sys/kernel/split_lock_mitigate",
            "0",
            "no_split_lock",
            Fix::Hardware,
        ),
    );
    // The limit that Proton games hit first when it is low, the classic cause
    // of a game that will not start. Welcome uses the same figure.
    add(
        &mut levers,
        &mut missing,
        spec("vm.max_map_count", "memory", "/proc/sys/vm/max_map_count", "2147483642", "no_sysctl", Fix::Kernel),
    );
    add(&mut levers, &mut missing, spec("vm.page_cluster", "memory", "/proc/sys/vm/page-cluster", "0", "no_sysctl", Fix::Kernel));
    add(
        &mut levers,
        &mut missing,
        spec("vm.dirty_background_bytes", "memory", "/proc/sys/vm/dirty_background_bytes", "134217728", "no_sysctl", Fix::Kernel),
    );
    add(
        &mut levers,
        &mut missing,
        spec("vm.dirty_writeback_centisecs", "memory", "/proc/sys/vm/dirty_writeback_centisecs", "1500", "no_sysctl", Fix::Kernel),
    );
    add(&mut levers, &mut missing, spec("fs.file_max", "kernel", "/proc/sys/fs/file-max", "2097152", "no_sysctl", Fix::Kernel));
    add(&mut levers, &mut missing, spec("net.tcp_fastopen", "net", "/proc/sys/net/ipv4/tcp_fastopen", "3", "no_sysctl", Fix::Kernel));
    add(
        &mut levers,
        &mut missing,
        spec("net.netdev_max_backlog", "net", "/proc/sys/net/core/netdev_max_backlog", "16384", "no_sysctl", Fix::Kernel),
    );
    add(&mut levers, &mut missing, spec("net.rmem_max", "net", "/proc/sys/net/core/rmem_max", "16777216", "no_sysctl", Fix::Kernel));
    add(&mut levers, &mut missing, spec("net.wmem_max", "net", "/proc/sys/net/core/wmem_max", "16777216", "no_sysctl", Fix::Kernel));
    add(&mut levers, &mut missing, spec("net.rmem_default", "net", "/proc/sys/net/core/rmem_default", "1048576", "no_sysctl", Fix::Kernel));
    add(&mut levers, &mut missing, spec("net.wmem_default", "net", "/proc/sys/net/core/wmem_default", "1048576", "no_sysctl", Fix::Kernel));
    add(
        &mut levers,
        &mut missing,
        spec(
            "kernel.sched_autogroup_enabled",
            "kernel",
            "/proc/sys/kernel/sched_autogroup_enabled",
            "0",
            "no_sysctl",
            Fix::Kernel,
        ),
    );

    match gpu_level() {
        Some(path) => levers.push(Box::new(node::Node::new("gpu.perf_level", "gpu", path, "high"))),
        None => missing.push(Missing {
            key: "gpu.perf_level",
            group: "gpu",
            reason: "no_gpu_level",
            fix: Fix::Hardware,
        }),
    }

    match io_scheduler() {
        Some(path) => levers.push(Box::new(node::Node::new("io.scheduler", "disk", path, "none"))),
        None => missing.push(Missing {
            key: "io.scheduler",
            group: "disk",
            reason: "no_disk",
            fix: Fix::Hardware,
        }),
    }

    (levers, missing)
}

/// The levers of this machine as JSON, for the window to draw. Reading is
/// allowed without privileges, so the panel can show them even when the
/// service is stopped; applying them still needs the daemon.
pub fn catalogue_json() -> serde_json::Value {
    use serde_json::json;
    let (available, missing) = catalogue();
    let levers: Vec<serde_json::Value> = available
        .iter()
        .map(|lever| {
            json!({
                "key": lever.key(),
                "group": lever.group(),
                "path": lever.path().display().to_string(),
                "value": lever::read(lever.path()).map(|value| lever::current(&value)).unwrap_or_default(),
                "gameValue": lever.game_value(),
                "available": true,
            })
        })
        .collect();
    json!({ "levers": levers, "missing": missing })
}

fn cpufreq_root() -> PathBuf {
    let root = std::env::var("SOPLOS_CPUFREQ_ROOT").unwrap_or_else(|_| "/sys/devices/system/cpu".to_string());
    PathBuf::from(root).join("cpu0/cpufreq")
}

/// The mode of the 3D V-Cache optimizer of AMD: which cores are preferred,
/// the ones with the extra cache or the ones with the higher frequency. From
/// kernel 6.13 the driver keeps one folder per device, each with the file.
fn x3d_node_in(root: &std::path::Path) -> Option<PathBuf> {
    let driver = root.join("bus/platform/drivers/amd_x3d_vcache");
    let entries = std::fs::read_dir(driver).ok()?;
    entries
        .flatten()
        .map(|entry| entry.path().join("amd_x3d_mode"))
        .find(|path| node::exists(path))
}

fn x3d_node() -> Option<PathBuf> {
    x3d_node_in(std::path::Path::new("/sys"))
}

/// Everything the catalogue needs to know about one node before deciding
/// whether this machine has it.
struct Spec {
    key: &'static str,
    group: &'static str,
    path: PathBuf,
    game_value: &'static str,
    /// Name of the translated string used when the node is not there.
    reason: &'static str,
    fix: Fix,
}

fn spec(
    key: &'static str,
    group: &'static str,
    path: impl Into<PathBuf>,
    game_value: &'static str,
    reason: &'static str,
    fix: Fix,
) -> Spec {
    Spec { key, group, path: path.into(), game_value, reason, fix }
}

fn add(levers: &mut Vec<Box<dyn Lever>>, missing: &mut Vec<Missing>, one: Spec) {
    if node::exists(&one.path) {
        levers.push(Box::new(node::Node::new(one.key, one.group, one.path, one.game_value)));
    } else {
        missing.push(Missing { key: one.key, group: one.group, reason: one.reason, fix: one.fix });
    }
}

/// The level node of the first graphics card that lets it be forced. AMD
/// exposes it as power_dpm_force_performance_level; the closed driver of NVIDIA
/// does not, and its clocks are set with nvidia-smi, which is a separate lever.
fn gpu_level() -> Option<PathBuf> {
    let mut cards: Vec<PathBuf> = std::fs::read_dir("/sys/class/drm")
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().starts_with("card"))
                .unwrap_or(false)
        })
        .collect();
    cards.sort();
    for card in cards {
        let path = card.join("device/power_dpm_force_performance_level");
        if node::exists(&path) {
            return Some(path);
        }
    }
    None
}

/// The scheduler node of the first disk that lets it be chosen. Virtual and
/// loop devices are skipped: they either have no queue or a single option.
/// The game value is "none", which only suits a disk without moving parts:
/// a hard disk wants a scheduler that orders its reads. A disk that can be
/// pulled out (a USB stick, a card) is not where the games live either. The
/// disks are looked at in order, so the same machine always gets the same one.
fn io_scheduler() -> Option<PathBuf> {
    let mut disks: Vec<PathBuf> = std::fs::read_dir("/sys/block").ok()?.flatten().map(|entry| entry.path()).collect();
    disks.sort();
    for disk in disks {
        let name = disk.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") || name.starts_with("sr") {
            continue;
        }
        let flag = |file: &str| lever::read(&disk.join(file)).map(|v| v == "1").unwrap_or(false);
        if flag("removable") || flag("queue/rotational") {
            continue;
        }
        let path = disk.join("queue/scheduler");
        if let Ok(value) = lever::read(&path) {
            if value.contains('[') && lever::list(&value.replace(['[', ']'], "")).iter().any(|one| one == "none") {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lever_is_never_both_here_and_missing() {
        let (levers, missing) = catalogue();
        let mut keys: Vec<&str> = levers.iter().map(|lever| lever.key()).collect();
        keys.extend(missing.iter().map(|one| one.key));
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total);
        assert!(missing.iter().all(|one| !one.reason.is_empty() && !one.group.is_empty()));
    }

    #[test]
    fn every_lever_of_the_profile_can_be_read() {
        for lever in levers() {
            assert!(lever::read(lever.path()).is_ok(), "{} cannot be read", lever.key());
        }
    }

    #[test]
    fn the_cache_of_amd_is_found_where_the_driver_leaves_it() {
        let root = crate::testing::scratch("sgp-x3d", "con");
        let device = root.join("bus/platform/drivers/amd_x3d_vcache/AMDI0101:00");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(device.join("amd_x3d_mode"), "frequency\n").unwrap();
        assert_eq!(x3d_node_in(&root), Some(device.join("amd_x3d_mode")));
    }

    #[test]
    fn a_machine_without_the_driver_has_no_cache_to_choose() {
        let root = crate::testing::scratch("sgp-x3d", "sin");
        std::fs::create_dir_all(root.join("bus/platform/drivers")).unwrap();
        assert_eq!(x3d_node_in(&root), None);
    }
}

