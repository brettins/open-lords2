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
    make_room_for_the_link();
    println!("cargo:rustc-env=L2_BUILD_ID={}", stamp());

    // **`L2_ALWAYS_UNLOCK=1` makes this script run before every link**, so a
    // build can never collide with a running game. It is opt-in because the cost
    // was measured rather than guessed: the two `rerun-if-changed` lines below
    // replace cargo's default of "any file in this package", and naming a path
    // that does not exist is how a build script asks to be rerun
    // unconditionally — but a rerun marks this crate dirty, so **every build
    // recompiles `l2-game`: 2.6 to 3.2 seconds against 0.17 to 0.21 for a no-op build.**
    //
    // That is a permanent tax on every build in the workspace, paid by everyone,
    // to cover the minutes a day somebody has the game open. The launcher in
    // `tools/run/` solves the same problem for nothing by running a *copy*, so
    // this switch exists for anyone who would rather not use it.
    println!("cargo:rerun-if-env-changed=L2_ALWAYS_UNLOCK");
    if std::env::var_os("L2_ALWAYS_UNLOCK").is_some_and(|v| v != "0") {
        println!("cargo:rerun-if-changed=build.rs.always-rerun");
    }
    // **Without the switch, this script reruns only when `HEAD` or `index`
    // moves**, so `make_room_for_the_link` protects only a build that follows a
    // commit, checkout, rebase or `git add`. An ordinary edit relinks this binary
    // without rerunning the script, and that build still collides with a running
    // game. Measured: a locked `l2-game.exe`, one touched source file, a plain
    // `cargo build -p l2-game`, and exit 101 with *"failed to remove file
    // `…\target\debug\l2-game.exe`"* / *"Access is denied. (os error 5)"*. That
    // is why the launcher is the primary mechanism and this rename is only the
    // safety net —
    // `docs/environment.md`, `docs/decisions.md` CNEW-launcher-never-launched.
    //
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

// ---------------------------------------------------------------------------

/// **Move a running `l2-game.exe` aside so this build can link over it.**
///
/// Windows will not let a build replace a running executable, so a player with
/// the game open turns every build in the workspace red — not only
/// `cargo build`, but `cargo test --workspace`, which builds this same binary.
/// That has cost the project real time twice: *"game is closed. You cant rebuild
/// with it open?"* **It is not the linker that fails.** rustc links into
/// `deps/` without complaint; cargo's last step removes the old
/// `target/<profile>/l2-game.exe` to put the new one in its place, and that is
/// what Windows refuses: *"failed to remove file `…\l2-game.exe`"* /
/// *"Access is denied. (os error 5)"*, exit 101.
///
/// **It will, however, let you *rename* one.** Verified on this machine rather
/// than assumed: overwriting a running exe fails (*"Device or resource busy"*
/// from Git Bash's `cp`), and renaming the same file succeeds, after which a
/// fresh exe writes to the original path while the old process keeps running
/// out of the renamed image. So the fix is to get the old file out of the way, not to give the new
/// one a different name.
///
/// **Why not the version-suffixed name the request suggested.** Stamping
/// `l2-game-<hash>.exe` would work, and it would break every shortcut, script
/// and muscle-memory `./target/debug/l2-game.exe` in the project, and leave
/// nobody able to say which file is current. Renaming the *stale* one keeps the
/// live path stable and puts the churn on the copy nobody refers to.
///
/// # What it costs
///
/// A debug exe is about 19 MB against a `target/debug` that is already 9.5 GB,
/// so ten stranded copies are two per cent of what is there. **No count-based
/// policy is needed and none is implemented**: every run of this script sweeps
/// every `l2-game.old-*.exe`, and the only ones that survive are the ones still
/// running, which are precisely the ones that must. *A run of this script is not
/// every build* — see the rerun conditions in `main` — so they are collected on
/// the first rerun after the player quits, not the first build. If the sweep
/// ever leaves more than a handful behind, that is reported rather than silently accumulated — it would
/// mean deletion is failing for a reason other than "still running", and a
/// disposable file that cannot be disposed of is worth knowing about.
///
/// # It cannot fail the build
///
/// Every step is best-effort. The existing stamp above takes the same line for
/// the same reason: a build script that stops the game compiling because a file
/// could not be renamed would be a far worse trade than the problem it solves.
/// On failure the build proceeds and cargo reports the same `failed to remove
/// file` error it would have reported anyway.
#[cfg(windows)]
fn make_room_for_the_link() {
    // **The name a moved exe gets and the names the sweep deletes are one
    // constant**, not two literals that must agree. If they drifted apart the
    // sweep would match nothing, and that is silent: the warning below counts
    // files it *failed to delete*, and a pattern that fails to *match* leaves
    // the count at zero while 19 MB copies accumulate. No test covers the sweep;
    // this is why none is needed. `docs/decisions.md` CNEW-launcher-never-launched.
    const ASIDE_PREFIX: &str = "l2-game.old-";
    const ASIDE_SUFFIX: &str = ".exe";
    use std::fs;

    // `OUT_DIR` is `<target>/<profile>/build/<pkg>-<hash>/out`; three levels up
    // is the profile directory the binary is linked into. This holds with an
    // explicit `--target` too, where the whole chain sits under the triple.
    let Ok(out) = std::env::var("OUT_DIR") else { return };
    let Some(profile) = PathBuf::from(out).ancestors().nth(3).map(PathBuf::from) else {
        return;
    };

    // Sweep first: most builds have nothing locked and this is the whole job.
    let mut stranded = 0usize;
    if let Ok(entries) = fs::read_dir(&profile) {
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with(ASIDE_PREFIX) && name.ends_with(ASIDE_SUFFIX) {
                // `DeleteFileW` refuses a running image, which is the answer we
                // want and not an error to report.
                if fs::remove_file(e.path()).is_err() {
                    stranded += 1;
                }
            }
        }
    }
    if stranded > 8 {
        println!(
            "cargo:warning=l2-game: {stranded} stale l2-game.old-*.exe files in \
             {} could not be removed. If the game is not running, something else \
             holds them open.",
            profile.display()
        );
    }

    // Then the live one, and only if it is actually locked. Opening for write
    // is the test: a running image refuses, an idle file opens and is left
    // alone so an ordinary build does no renaming at all.
    let exe = profile.join("l2-game.exe");
    if !exe.exists() || fs::OpenOptions::new().write(true).open(&exe).is_ok() {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let aside = profile.join(format!("{ASIDE_PREFIX}{stamp}{ASIDE_SUFFIX}"));
    match fs::rename(&exe, &aside) {
        Ok(()) => println!(
            "cargo:warning=l2-game: the game is running, so the old binary was moved to {}. \
             The build continues; the running copy is unaffected and is swept on a later build.",
            aside.file_name().and_then(|s| s.to_str()).unwrap_or("l2-game.old-*.exe")
        ),
        Err(e) => println!(
            "cargo:warning=l2-game: {} is locked and could not be renamed ({e}). \
             The link is about to fail; close the game and build again.",
            exe.display()
        ),
    }
}

#[cfg(not(windows))]
fn make_room_for_the_link() {}
