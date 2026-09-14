//! **The audio layer against a real install.** Headless: no device is opened
//! and no sound is made, so this runs on CI's runner —
//! by skipping, because CI has no install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_install
//! ```
//!
//! What these check is the pair of claims the rest of `src/audio` rests on:
//!
//! * **every name the recovered tables hold is a file that ships**, so a track
//!   or a bank slot cannot silently resolve to nothing;
//! * **every sound the game plays is PCM at 11,025 Hz, 8-bit**, which is what
//!   `wav.rs` was allowed to be as small as it is because of.
//!
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

/// The install is inconsistent about casing — `Scroll1.wav` on disk,
/// `scroll1.wav` in `Lords2.exe`'s tables — so every lookup here is
/// case-insensitive, the same rule the mod overlay applies.
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

