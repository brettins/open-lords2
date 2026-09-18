//! Both are corpus checks over all 771 files
//! happened to work — `docs/decisions.md` C1.

mod track_tests;
pub use track_tests::*;
mod voice_tests;
pub use voice_tests::*;
mod format_tests;
pub use format_tests::*;

use std::path::{Path, PathBuf};

use l2_game::audio::{names, track, wav};

pub(crate) fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).find_map(|e| {
        let p = e.path();
        let n = p.file_name()?.to_str()?.to_ascii_lowercase();
        (n == want).then_some(p)
    })
}

fn every_wav(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
        })
        .collect();
    v.sort();
    v
}

