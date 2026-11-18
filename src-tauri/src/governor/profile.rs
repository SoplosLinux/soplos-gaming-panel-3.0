// The profiles the panel offers. A profile is not a different engine: it is
// the same levers with values chosen for a kind of machine, so what is applied
// can always be read, shown and put back.

use std::fmt;

pub const MAX: &str = "max";
pub const BALANCED: &str = "balanced";
pub const QUIET: &str = "quiet";

pub fn ids() -> [&'static str; 3] {
    [MAX, BALANCED, QUIET]
}

/// What a lever does under a profile. Leaving it alone is a real answer: the
/// balanced and quiet profiles do not touch the disk scheduler, for one.
pub enum Action {
    Set(String),
    Keep,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Set(value) => write!(f, "{value}"),
            Action::Keep => write!(f, "keep"),
        }
    }
}

/// The value for one lever, or None when the profile does not exist.
pub fn value(profile: &str, key: &str, strongest: &str) -> Option<Action> {
    if !ids().contains(&profile) {
        return None;
    }
    // The mode of amd_pstate is shown, never written: switching it while the
    // machine runs starts the cpufreq policy of every core again, which drops
    // the governor and the energy preference the profile has just written, and
    // a restore could not put them back in order. "active" is already the mode
    // the kernels of today start in.
    if key == "cpu.pstate" {
        return Some(Action::Keep);
    }
    let keep = |value: &str| Action::Set(value.to_string());
    let action = match profile {
        // The strongest profile is the value each lever already declares.
        MAX => keep(strongest),
        BALANCED => match key {
            "cpu.governor" => keep("schedutil"),
            "cpu.epp" => keep("balance_performance"),
            "cpu.pstate" => keep("active"),
            "gpu.perf_level" => keep("auto"),
            "kernel.thp" => keep("madvise"),
            "vm.swappiness" => keep("30"),
            "vm.dirty_bytes" => keep("134217728"),
            "kernel.nmi_watchdog" => keep("0"),
            "kernel.split_lock_mitigate" => keep("0"),
            "kernel.sched_autogroup_enabled" => keep("0"),
            "io.scheduler" => Action::Keep,
            _ => keep(strongest),
        },
        QUIET => match key {
            "cpu.governor" => keep("schedutil"),
            "cpu.epp" => keep("balance_power"),
            "cpu.pstate" => keep("active"),
            "gpu.perf_level" => keep("auto"),
            "kernel.thp" => keep("madvise"),
            "vm.swappiness" => keep("30"),
            "vm.dirty_bytes" => keep("67108864"),
            "kernel.nmi_watchdog" => keep("0"),
            "kernel.split_lock_mitigate" => keep("0"),
            "kernel.sched_autogroup_enabled" => keep("0"),
            "io.scheduler" => Action::Keep,
            _ => keep(strongest),
        },
        _ => return None,
    };
    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_profile_is_refused() {
        assert!(value("turbo", "cpu.governor", "performance").is_none());
    }

    #[test]
    fn the_strongest_profile_uses_what_each_lever_declares() {
        match value(MAX, "cpu.governor", "performance").unwrap() {
            Action::Set(value) => assert_eq!(value, "performance"),
            Action::Keep => panic!("the strongest profile always sets the value"),
        }
    }

    #[test]
    fn the_quiet_profile_does_not_touch_the_disk_scheduler() {
        assert!(matches!(value(QUIET, "io.scheduler", "none"), Some(Action::Keep)));
    }

    #[test]
    fn every_profile_leaves_the_memory_map_limit_ready_for_proton() {
        for profile in ids() {
            match value(profile, "vm.max_map_count", "1048576") {
                Some(Action::Set(value)) => assert_eq!(value, "1048576", "{profile}"),
                _ => panic!("{profile} must keep the map limit high"),
            }
        }
    }
}
