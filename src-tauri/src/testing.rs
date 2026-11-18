// A folder for a test that deletes itself when the test ends, so running the
// suite leaves nothing behind in the temporary directory.

use std::ops::Deref;
use std::path::{Path, PathBuf};

pub struct Scratch(PathBuf);

/// A clean folder for one test. The prefix keeps the tests of each module
/// apart, and the process id keeps two runs apart.
pub fn scratch(prefix: &str, name: &str) -> Scratch {
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let _ = std::fs::create_dir_all(&path);
    Scratch(path)
}

impl Scratch {
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
