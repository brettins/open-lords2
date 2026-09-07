//! A throwaway directory, so the filesystem tests need no dependency and no
//! game install.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("l2mods-{tag}-{}-{nanos}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write a file, creating parent directories. `rel` uses forward slashes.
    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let full = self.path.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&full, contents).expect("write file");
        full
    }

    #[allow(dead_code)] // used by some test files, not all
    pub fn dir(&self, rel: &str) -> PathBuf {
        let full = self.path.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        std::fs::create_dir_all(&full).expect("create dir");
        full
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
