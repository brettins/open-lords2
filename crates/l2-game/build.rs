//! Stamps this build's identity into the binary, for `crate::build_id`.
//!
//! **Why there is a build script at all.** Three of the last five defects a
//! player reported were against a binary four merges old, and there was no way
//! for him — or for us — to tell. A version string is no help here: `0.1.0` has
//! been true of every build for two months, so it identifies nothing and is
//! worse than nothing, because it looks like an answer.
//!
//! What is emitted is the short commit, a `-DIRTY` marker when the tree had
//! uncommitted tracked changes, and the commit's date. The date is not
//! decoration: *"is this old?"* is the actual question, and a hash cannot answer
//! it without a lookup.
//!
//! **Two honest limits, stated here because the alternative is a stamp nobody
//! can trust.**
//!
//! 1. `cargo:rerun-if-changed` on the git directory's `HEAD` and `index` means
//!    this reruns on every commit, checkout, rebase and `git add` — which is
//!    exactly the staleness that bit the player. It does **not** rerun for an
//!    unstaged edit, so `-DIRTY` means *"the tree was dirty when this build's
//!    fingerprint was last taken"*, not *"…at this instant"*. Erring that way is
//!    deliberate: the marker can lag on, never off, so a stamp with no `-DIRTY`
//!    is trustworthy and one with it is a warning.
//! 2. Git may be missing, or the source may be a tarball. Then the stamp reads
//!    `NO GIT`, which is a fact rather than a fabricated version.
//!
//! Nothing here can fail the build. A build script that stops the game
//! compiling because `git` was not on the path would be a poor trade for a
//! caption.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-env=L2_BUILD_ID={}", stamp());
    // Only these two, and only when git could name them: emitting any
    // `rerun-if-changed` replaces cargo's default of "any file in the package",
    // so anything not listed here stops triggering a rerun.
    for name in ["HEAD", "index"] {
        if let Some(dir) = git_dir() {
            let p = dir.join(name);
            if p.exists() {
                println!("cargo:rerun-if-changed={}", p.display());
            }
        }
    }
}

fn stamp() -> String {
    let Some(commit) = git(&["rev-parse", "--short=9", "HEAD"]) else {
        return "NO GIT".into();
    };
    // `--untracked-files=no`: a stray scratch file in the tree is not a
    // different build of the game, and treating it as one would make the marker
    // permanent and therefore ignorable.
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|s| !s.trim().is_empty());
    let date = git(&["log", "-1", "--format=%cd", "--date=format:%Y-%m-%d"])
        .unwrap_or_else(|| "?".into());
    format!(
        "{}{} {}",
        commit.to_uppercase(),
        if dirty { "-DIRTY" } else { "" },
        date
    )
}

/// `--absolute-git-dir` rather than a hand-built `../../.git`, because agents
/// work in `git worktree`s, where `.git` is a *file* pointing at
/// `…/.git/worktrees/<name>` — and that directory has its own `HEAD` and
/// `index`, which are the two this build wants to watch.
fn git_dir() -> Option<PathBuf> {
    git(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}
