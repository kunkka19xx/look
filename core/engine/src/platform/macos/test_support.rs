//! Shared fixtures for the macOS platform tests. Both the app walk and the
//! bundle-metadata reads need throwaway directories of `.app`/`.appex` bundles.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A directory under the system temp dir, removed when it goes out of scope.
/// The name carries the pid and a nanosecond stamp so concurrent test binaries
/// never collide.
pub(super) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub(super) fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "look-macos-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
