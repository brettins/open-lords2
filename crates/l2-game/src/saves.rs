//! **Where a saved game lives**, and the four operations on that directory.
//!
//! [`crate::save`] turns a [`Game`] into bytes and knows nothing about files.
//! This is the other half: one directory, a listing, a read and a write.
//!
//! # Where, and why not the two obvious places
//!
//! | candidate | why not |
//! |---|---|
//! | inside the game install | **`CLAUDE.md` rule 2: the install is read only.** It is also where the original keeps `lastturn.sav`, which we read as an oracle and whose fixture identity has already been destroyed once by a program writing into that directory (`docs/environment.md`). |
//! | inside the repository | `.gitignore` refuses `*.sav` and a test asserts none is in the tree. A save that lands beside the source is a save somebody commits. |
//! | beside the executable | works for a portable build and fails for an installed one, because `%PROGRAMFILES%` is not writable by the user who runs the game. |
//!
//! So: **under the user's own profile**, in a directory named after *this*
//! project
//! own format.
//!
//! | platform | directory |
//! |---|---|
//! | Windows | `%APPDATA%\open-lords2\saves` |
//! | anywhere else | `$XDG_DATA_HOME/open-lords2/saves`, else `$HOME/.local/share/open-lords2/saves` |
//!
//! [`DIR_VAR`] overrides all of it. That is not only for tests: a player who
//! keeps a game on a second drive, and a machine with a roaming profile it
//! would rather not fill, both want it, and the alternative is a settings file
//! whose own location has the same problem. [`scoped_dir`] overrides *that*,
//! for one thread, and **is** only for tests — it says why.
//!
//! The directory is created on the first *write* and never on a read or a
//! listing, so merely opening the load screen leaves the disk alone.
//!
//! # The extension is `.l2sav`, not `.sav`
//!
//! [`crate::save::EXTENSION`] says why: the original's `.sav` is a memory dump
//! we read as an oracle, ours is a versioned format of our own, and a directory
//! listing that cannot tell them apart is one somebody eventually confuses.

use std::path::{Path, PathBuf};

use l2_kingdom::tables::Tables;

use crate::game::Game;
use crate::save::{self, LoadError, EXTENSION};

/// The environment variable that overrides the directory below.
pub const DIR_VAR: &str = "LORDS2_SAVES";

/// The directory name under the user's data directory. Ours, deliberately not
/// the original's: `Lords of the Realm II` is their name for their files.
pub const APP_DIR: &str = "open-lords2";

/// How long a save name may be. The original's own file list holds 65-byte
/// records (`SaveLoad_DrawStatus` indexes `DAT_004e8790 + i * 0x41`), so 64
/// characters is the number it can display; anything longer would be a name the
/// list could not show.
pub const MAX_NAME: usize = 64;

#[derive(Debug)]
pub enum Error {
    /// Neither [`DIR_VAR`] nor a home directory could be resolved, so there is
    /// nowhere on this machine a save may go.
    NoDirectory,
    /// A name with a path separator, a drive letter, a `..`, or nothing in it.
    /// **Refused** — a name that is quietly changed is a
    /// save the player cannot find again.
    BadName(String),
    /// The file system said no.
    Io { path: PathBuf, detail: String },
    /// The bytes are not a saved game this build can read.
    Load { name: String, detail: LoadError },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoDirectory => write!(
                f,
                "no save directory: neither {DIR_VAR} nor a profile directory is set"
            ),
            Error::BadName(n) => write!(f, "{n:?} is not a save name"),
            Error::Io { path, detail } => write!(f, "{}: {detail}", path.display()),
            Error::Load { name, detail } => write!(f, "{name}: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// One entry of the load screen's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name without the extension — what the list shows and what
    /// [`write`] takes.
    pub name: String,
    pub path: PathBuf,
    /// The size on disk, for nothing but a diagnostic.
    pub bytes: u64,
}

/// The save directory, or `None` when this machine has nowhere to put one.
///
/// In order: this thread's [`scoped_dir`], then [`DIR_VAR`], then the profile.
///
/// Read on every call: a cached answer would be a second
/// source of truth for a value the user can change, and this is called once
/// per screen open.
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = SCOPED.with(|s| s.borrow().clone()) {
        return Some(d);
    }
    if let Ok(v) = std::env::var(DIR_VAR) {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    Some(data_home()?.join(APP_DIR).join("saves"))
}

thread_local! {
    static SCOPED: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// **Every save operation on this thread uses `path` until the guard drops** —
/// [`list`], [`write`], [`read`], [`remove`] and the save screen, which reaches
/// the directory through [`dir`] and nothing else.
///
/// Nothing in the game calls it. It exists for the same reason [`remove`] does:
/// the test harness runs every test as a **thread of one process**, and
/// [`DIR_VAR`] is process-global, so an environment variable can give a test
/// binary one directory and never one per test. With one directory shared,
/// `tests/save.rs` raced itself: its load-screen test opened the screen, took a
/// *second* listing to decide which row to click, and another test writing or
/// deleting a save between those two statements moved the row under it — 23
/// runs of the test binary in 2,000, measured. A lock round the
/// listing was tried first and it only protected the tests that remembered to
/// take it, and four did not.
///
/// A directory per thread is the shape in which that race cannot be written:
/// no test can see another's files, whatever it names them and whenever it
/// lists. The guard restores whatever was scoped before it, so scopes nest,
/// and it is not `Send`, so it cannot be dropped on a thread whose slot it
/// never set.
pub fn scoped_dir(path: impl Into<PathBuf>) -> ScopedDir {
    let previous = SCOPED.with(|s| s.borrow_mut().replace(path.into()));
    ScopedDir { previous, _not_send: core::marker::PhantomData }
}

/// The guard [`scoped_dir`] returns. See there.
#[must_use = "the directory is scoped only while the guard is alive"]
pub struct ScopedDir {
    previous: Option<PathBuf>,
    _not_send: core::marker::PhantomData<*const ()>,
}

impl Drop for ScopedDir {
    fn drop(&mut self) {
        let previous = self.previous.take();
        SCOPED.with(|s| *s.borrow_mut() = previous);
    }
}

/// `%APPDATA%` on Windows, `$XDG_DATA_HOME` or `$HOME/.local/share` elsewhere.
fn data_home() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Ok(v) = std::env::var("APPDATA") {
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join(".local").join("share"))
}

/// Whether a name may be written. **A whole-name test, not a scrub**: a
/// rejected name is reported, never silently repaired.
///
/// It refuses anything that is not a plain file name — separators of either
/// slash, a drive colon, a leading dot, `..`, a control byte, and the empty
/// string — because a save name reaches the file system and "the player typed
/// it" is not a reason to let it name a path.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !name.starts_with(' ')
        && !name.ends_with(' ')
        && !name.ends_with('.')
        && name.chars().all(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

/// The path a name maps to. `Err` for a name [`is_valid_name`] refuses.
pub fn path_for(name: &str) -> Result<PathBuf, Error> {
    if !is_valid_name(name) {
        return Err(Error::BadName(name.to_string()));
    }
    let dir = dir().ok_or(Error::NoDirectory)?;
    Ok(dir.join(format!("{name}.{EXTENSION}")))
}

/// Every save in the directory, **sorted by name**.
///
/// Sorted, not in directory order, and that is a correctness point
/// tidiness: `read_dir` returns whatever the file system happens to hand back,
/// which differs between machines and between file systems, and a list whose
/// order depends on that is a list where the same click means different things
/// on two computers. `docs/netcode.md` D-4 forbids exactly this shape of
/// iteration inside the simulation; the interface has no excuse for it either.
///
/// A missing directory is an **empty list**, not an error: a player who has
/// never saved has no directory, and that is not a fault to report.
pub fn list() -> Vec<Entry> {
    let Some(dir) = dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Entry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if !path.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION)) {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let bytes = e.metadata().ok().map_or(0, |m| m.len());
            Some(Entry { name, path, bytes })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Write a game out, creating the directory if it is not there.
///
/// The write is **atomic where the platform allows it**: the bytes go to a
/// neighbouring temporary file and are renamed over the target, so a crash or a
/// full disk halfway through leaves the previous save intact
/// truncated. Losing a saved game to a failed save of the same name is the one
/// failure a player never forgives.
pub fn write(name: &str, game: &Game) -> Result<PathBuf, Error> {
    let path = path_for(name)?;
    let dir = path.parent().expect("path_for joins a directory").to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;

    let bytes = save::encode(game);
    let temp = path.with_extension(format!("{EXTENSION}.part"));
    std::fs::write(&temp, &bytes).map_err(|e| io(&temp, e))?;
    // `rename` over an existing file is atomic on POSIX and, on Windows,
    // `std::fs::rename` maps to `MoveFileEx` with `REPLACE_EXISTING`.
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        io(&path, e)
    })?;
    Ok(path)
}

/// Read a game back on a supplied ruleset.
pub fn read(name: &str, tables: Tables) -> Result<Game, Error> {
    let path = path_for(name)?;
    read_path(&path, tables)
}

/// The same, from a path a listing already produced.
pub fn read_path(path: &Path, tables: Tables) -> Result<Game, Error> {
    let bytes = std::fs::read(path).map_err(|e| io(path, e))?;
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
    save::decode(&bytes, tables).map_err(|detail| Error::Load { name, detail })
}

/// **`Save_RotateAndWrite`'s three names** (`0x0049A453`), newest first.
///
/// The original's are `lastturn.sav`, `old_turn.sav` and `safeturn.sav` — three
/// 13-byte literals at `0x004DC2F0`, `0x004DC300` and `0x004DC310`, `[V]` read
/// out of `.rdata`. The stems are kept and the extension is ours, for
/// [`crate::save::EXTENSION`]'s reason: these are our files in our own format,
/// and a directory listing that cannot tell them from the original's memory
/// dumps is one somebody eventually confuses.
///
/// A multiplayer game writes a different triple — `.sva` for the master,
/// `.svb` for a client — and we have no network game to write one for.
pub const AUTOSAVES: [&str; 3] = ["lastturn", "old_turn", "safeturn"];

/// **`Save_RotateAndWrite` (`0x0049A453`)** — shift the window down one and
/// write the newest.
///
/// ```c
/// if (DAT_00553260 < 1) {
///     remove(safeturn);  rename(old_turn, safeturn);  rename(lastturn, old_turn);
/// }
/// Save_Write(lastturn);
/// ```
///
/// `[V]`, decompiled. So it keeps **three** turns, not a numbered series, and
/// the newest name is rewritten every time: `lastturn` is the turn that just
/// began, `old_turn` the one before it, `safeturn` the one before that. That is
/// the shape the player asked for — *"there doesn't seem to be a last-turn
/// autosave either so I can't easily repro that for you"* — and three deep is
/// what makes the turn *before* the one that went wrong reachable too.
///
/// **Every rotation step's failure is ignored, as the original ignores it.** A
/// game's first autosave has no `old_turn` to rename and no `safeturn` to
/// remove, and `remove`/`rename` simply return non-zero there; the write is the
/// only step whose failure is worth a word.
///
/// `DAT_00553260` is not built. It is set to 2 by `FUN_0049B973`, the
/// multiplayer com-link-error path that reloads `lastturn.sva`/`.svb`, and
/// decremented per turn — *don't shift the snapshot we just resynced from out
/// of the window*. `FUN_0043F01D`, the network bring-up, zeroes it. In a single
/// player game it is zero and the rotation always runs. `[V]` on both writers.
pub fn rotate_and_write(game: &Game) -> Result<PathBuf, Error> {
    let [newest, middle, oldest] = AUTOSAVES;
    let (newest, middle, oldest) = (path_for(newest)?, path_for(middle)?, path_for(oldest)?);
    let _ = std::fs::remove_file(&oldest);
    let _ = std::fs::rename(&middle, &oldest);
    let _ = std::fs::rename(&newest, &middle);
    write(AUTOSAVES[0], game)
}

/// **`FUN_0049A3E6`'s last statement, pumped** — the machine's standing autosave
/// request, carried out.
///
/// The request is raised by a screen and drained by [`Machine`]
/// ([`crate::screen::Screen::take_autosave`]); this is the one place it becomes
/// a file. It lives in the library
/// [`crate::audio::Director::listen`]'s reason, which is the whole argument for
/// where the seam goes: **a binary's code cannot be called by a test**, so an
/// application that performed the write itself could only be checked by a test
/// that re-typed the same lines beside its own assertions.
///
/// It is also why nothing in the simulation writes a file. Seventeen test files
/// end a turn through [`crate::screens::map::MapScreen`]; not one of them calls
/// this, so not one of them can rotate the person's own autosaves out from under
/// them. That failure is the exact defect being fixed here, and
/// `docs/environment.md` has already watched a program destroy a save
/// directory's identity once.
pub fn run_pending(
    machine: &mut crate::screen::Machine,
    game: &Game,
) -> Option<Result<PathBuf, Error>> {
    machine.take_autosave().then(|| rotate_and_write(game))
}

/// Delete one. The save screen does not offer it; this exists so that a test
/// can clean up after itself without reaching for `std::fs` and a path it
/// assembled by hand.
pub fn remove(name: &str) -> Result<(), Error> {
    let path = path_for(name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(&path, e)),
    }
}

fn io(path: &Path, e: std::io::Error) -> Error {
    Error::Io { path: path.to_path_buf(), detail: e.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_could_name_a_path_is_refused_rather_than_scrubbed() {
        assert!(is_valid_name("Winter 1268"));
        assert!(is_valid_name("autosave"));
        assert!(is_valid_name("a"));

        for bad in [
            "",
            ".",
            "..",
            ".hidden",
            "../../etc/passwd",
            "sub/dir",
            r"sub\dir",
            "C:evil",
            "trailing ",
            " leading",
            "trailing.",
            "with\nnewline",
            "wild*card",
        ] {
            assert!(!is_valid_name(bad), "{bad:?} was accepted as a save name");
        }
        assert!(!is_valid_name(&"x".repeat(MAX_NAME + 1)), "longer than the list can show");
        assert!(is_valid_name(&"x".repeat(MAX_NAME)));
    }

    #[test]
    fn a_refused_name_never_reaches_the_file_system() {
        // `path_for` is the only route from a name to a path, and it refuses
        // before it so much as resolves the directory.
        assert!(matches!(path_for("../escape"), Err(Error::BadName(_))));
        assert!(matches!(path_for(""), Err(Error::BadName(_))));
    }

    #[test]
    fn the_directory_is_under_the_users_profile_and_never_inside_an_install() {
        let Some(d) = dir() else { return };
        let shown = d.display().to_string().to_ascii_lowercase();
        assert!(
            !shown.contains("lords of the realm"),
            "a save must never land inside the game install: {shown}"
        );
        assert!(
            shown.contains(APP_DIR) || std::env::var(DIR_VAR).is_ok(),
            "the default directory is named after this project: {shown}"
        );
    }
}
