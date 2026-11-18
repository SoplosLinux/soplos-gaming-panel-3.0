// The schedulers of sched_ext are not a value to write in a file: they are
// programs that stay running, and the service `scx_loader` is the one that
// keeps them and swaps them. This module only asks that service what it has
// and what it is running; it never starts a scheduler on its own.

use serde::Serialize;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;

/// The name the service answers to on the system bus, and its one object.
const SERVICE: &str = "org.scx.Loader";
const OBJECT: &str = "/org/scx/Loader";
const INTERFACE: &str = "org.scx.Loader";

/// Where a kernel built with `sched_ext` says so.
const KERNEL_NODE: &str = "/sys/kernel/sched_ext";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Scx {
    /// The kernel was built with `sched_ext`.
    pub kernel: bool,
    /// The service answers on the system bus.
    pub service: bool,
    /// `unknown` while no scheduler is running.
    pub current: String,
    /// 0 auto, 1 gaming, 2 powersave, 3 lowlatency, 4 server.
    pub mode: u32,
    pub supported: Vec<String>,
}

impl Scx {
    /// Whether a scheduler can be chosen at all. Without either half the
    /// window says what is missing instead of offering a switch that would
    /// do nothing.
    pub fn available(&self) -> bool {
        self.kernel && self.service
    }
}

pub fn status() -> Scx {
    status_in(Path::new(KERNEL_NODE))
}

/// The same, looking for the node of the kernel in another folder, so a test
/// can answer both ways without touching `/sys`.
fn status_in(node: &Path) -> Scx {
    let kernel = kernel_has_sched_ext(node);
    let nothing = Scx {
        kernel,
        service: false,
        current: "unknown".to_string(),
        mode: 0,
        supported: Vec::new(),
    };
    if !kernel {
        return nothing;
    }
    match property("CurrentScheduler").and_then(|answer| text_of(&answer)) {
        Some(current) => Scx {
            kernel,
            service: true,
            current,
            mode: property("SchedulerMode")
                .and_then(|answer| number_of(&answer))
                .unwrap_or(0),
            supported: property("SupportedSchedulers")
                .map(|answer| strings_of(&answer))
                .unwrap_or_default(),
        },
        // The kernel can do it, but nothing is answering: the service of the
        // package is not installed or not running.
        None => nothing,
    }
}

/// Starts the scheduler, or swaps the one that is running. The service does
/// the work; this only names what is wanted.
pub fn switch(name: &str, mode: u32) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("Not a scheduler name: {name}"));
    }
    call(&["SwitchScheduler", "su", name, &mode.to_string()])
}

/// Lets go of the scheduler and leaves the kernel's own in charge.
pub fn stop() -> Result<(), String> {
    call(&["StopScheduler"])
}

fn kernel_has_sched_ext(node: &Path) -> bool {
    node.is_dir()
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn property(name: &str) -> Option<String> {
    let answer = Command::new("busctl")
        .args(["--system", "get-property", SERVICE, OBJECT, INTERFACE, name])
        .output()
        .ok()?;
    if !answer.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&answer.stdout).trim().to_string())
}

fn call(args: &[&str]) -> Result<(), String> {
    let answer = Command::new("busctl")
        .arg("--system")
        .arg("call")
        .args([SERVICE, OBJECT, INTERFACE])
        .args(args)
        .output()
        .map_err(|error| format!("busctl: {error}"))?;
    if answer.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&answer.stderr).trim().to_string();
    Err(if text.is_empty() { "The scheduler service refused the change".to_string() } else { text })
}

/// `s "scx_lavd"` as it comes from `busctl`.
fn text_of(answer: &str) -> Option<String> {
    let start = answer.find('"')? + 1;
    let end = answer.rfind('"')?;
    if end <= start {
        return None;
    }
    Some(answer[start..end].to_string())
}

/// `as 2 "scx_bpfland" "scx_lavd"` as it comes from `busctl`.
fn strings_of(answer: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = answer;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        names.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    names
}

/// `u 3` as it comes from `busctl`.
fn number_of(answer: &str) -> Option<u32> {
    answer.split_whitespace().last()?.parse().ok()
}

/// Where the node of the kernel would be inside a folder, for the tests that
/// build one.
#[cfg(test)]
pub fn kernel_node_in(root: &Path) -> PathBuf {
    root.join("sched_ext")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch;

    #[test]
    fn a_kernel_without_sched_ext_is_reported_as_without_it() {
        let folder = scratch("scx", "sin");
        assert!(!kernel_has_sched_ext(&kernel_node_in(&folder)));
        let state = status_in(&kernel_node_in(&folder));
        assert!(!state.kernel);
        assert!(!state.available());
        assert!(state.supported.is_empty());
    }

    #[test]
    fn a_string_property_is_read() {
        assert_eq!(text_of("s \"scx_lavd\"").as_deref(), Some("scx_lavd"));
        assert_eq!(text_of("s \"unknown\"").as_deref(), Some("unknown"));
        assert_eq!(text_of(""), None);
    }

    #[test]
    fn a_list_property_is_read() {
        assert_eq!(strings_of("as 2 \"scx_bpfland\" \"scx_lavd\""), vec!["scx_bpfland", "scx_lavd"]);
        assert!(strings_of("as 0").is_empty());
    }

    #[test]
    fn a_number_property_is_read() {
        assert_eq!(number_of("u 1"), Some(1));
        assert_eq!(number_of("u 4"), Some(4));
        assert_eq!(number_of("b false"), None);
    }

    #[test]
    fn only_a_plain_scheduler_name_is_accepted() {
        assert!(valid_name("scx_lavd"));
        assert!(valid_name("scx_bpfland"));
        assert!(!valid_name(""));
        assert!(!valid_name("scx_lavd; rm -rf /"));
        assert!(!valid_name("scx lavd"));
    }

    #[test]
    fn a_name_that_is_not_a_name_is_refused_before_asking() {
        let refused = switch("scx_lavd --performance", 1);
        assert!(refused.is_err());
    }
}
