
use std::path::PathBuf;
use std::process::Command;

fn main() {
    make_room_for_the_link();
    println!("cargo:rustc-env=L2_BUILD_ID={}", stamp());

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
    // `docs/environment.md`, `docs/decisions.md` C154.
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
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|s| !s.trim().is_empty());
    let date = git(&["log", "-1", "--format=%cd", "--date=format:%Y-%m-%d"])
        .unwrap_or_else(|| "?".into());
    let time = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Date -Format HH:mm"])
        .output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|t| !t.is_empty()).unwrap_or_else(|| "??:??".into());
    format!(
        "{}{} {} {}",
        commit.to_uppercase(),
        if dirty { "-DIRTY" } else { "" },
        date,
        time
    )
}

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


#[cfg(windows)]
fn make_room_for_the_link() {
    // **The name a moved exe gets and the names the sweep deletes are one
    // constant**, not two literals that must agree. If they drifted apart the
    // sweep would match nothing, and that is silent: the warning below counts
    // files it *failed to delete*, and a pattern that fails to *match* leaves
    // the count at zero while 19 MB copies accumulate. No test covers the sweep;
    // this is why none is needed. `docs/decisions.md` C154.
    const ASIDE_PREFIX: &str = "l2-game.old-";
    const ASIDE_SUFFIX: &str = ".exe";
    use std::fs;

    let Ok(out) = std::env::var("OUT_DIR") else { return };
    let Some(profile) = PathBuf::from(out).ancestors().nth(3).map(PathBuf::from) else {
        return;
    };

    let mut stranded = 0usize;
    if let Ok(entries) = fs::read_dir(&profile) {
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with(ASIDE_PREFIX) && name.ends_with(ASIDE_SUFFIX) {
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
