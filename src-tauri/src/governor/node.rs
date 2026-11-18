// A lever that is nothing more than a node with one value: the kernel tunables
// of /proc/sys and the transparent huge pages setting. Writing is all they take,
// so one type covers them instead of a module each.

use super::lever::{self, Lever};
use std::path::{Path, PathBuf};

pub struct Node {
    key: &'static str,
    group: &'static str,
    path: PathBuf,
    game_value: &'static str,
    /// The file of cpufreq this node is, when every core has its own copy.
    per_core: Option<&'static str>,
}

impl Node {
    pub fn new(key: &'static str, group: &'static str, path: PathBuf, game_value: &'static str) -> Self {
        Self { key, group, path, game_value, per_core: None }
    }

    /// A file of cpufreq read from cpu0 and written to every core.
    pub fn per_core(key: &'static str, group: &'static str, path: PathBuf, file: &'static str, game_value: &'static str) -> Self {
        Self { key, group, path, game_value, per_core: Some(file) }
    }
}

impl Lever for Node {
    fn key(&self) -> &'static str {
        self.key
    }

    fn group(&self) -> &'static str {
        self.group
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn paths(&self) -> Vec<PathBuf> {
        let Some(file) = self.per_core else { return vec![self.path.clone()] };
        // The folder of the cores is the one above cpu0/cpufreq.
        let all = self
            .path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|root| super::cpu::every_core_in(root, file))
            .unwrap_or_default();
        if all.is_empty() {
            vec![self.path.clone()]
        } else {
            all
        }
    }

    fn game_value(&self) -> String {
        self.game_value.to_string()
    }

    fn allowed(&self) -> Option<Vec<String>> {
        // Without a list of accepted values the check is left to the node: a
        // refused write comes back as an error and the profile is undone.
        None
    }
}

/// Reads a node, telling apart the failure that means "not here".
pub fn exists(path: &Path) -> bool {
    path.exists() && lever::read(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn writes_the_value_it_was_given() {
        let dir = crate::testing::scratch("sgp-node", "swappiness");
        let path = dir.join("swappiness");
        fs::write(&path, "60\n").unwrap();

        let node = Node::new("vm.swappiness", "memory", path.clone(), "10");
        assert_eq!(lever::read(node.path()).unwrap(), "60");
        lever::write(node.path(), &node.game_value()).unwrap();
        assert_eq!(lever::read(&path).unwrap(), "10");
    }

    #[test]
    fn a_node_that_is_not_there_is_not_a_lever() {
        assert!(!exists(Path::new("/proc/sys/soplos-does-not-exist")));
    }
}
