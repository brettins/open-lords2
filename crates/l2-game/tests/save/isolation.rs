#![allow(unused_imports)]
use super::*;
use super::roundtrip::*;
use super::ui::*;
use super::autosave::*;
use std::path::PathBuf;
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

// --- the files -------------------------------------------------------------

/// **A save directory belonging to one test**, and every save operation on the
/// test's thread pointed at it until this drops — the listing, the writes, and
/// the save screen, which reaches the directory through `saves::dir` like
/// everything else. The directory is deleted on drop.
///
/// # Why one each, and not one lock
///
/// These tests used to share one directory, named by `LORDS2_SAVES` for the
/// whole process, and use distinct file names. Distinct names are enough for
/// *"is my file there?"* and not for anything that depends on the listing's
/// **shape**: the listing is sorted
/// moves every row after it. The load-screen test opened the screen, then took
/// a *second* listing to work out which row to click, and when another test
/// wrote or removed a save between those two statements the click landed on
/// somebody else's game: 23 runs of this binary in 2,000, and 5 in 5 with
/// barriers forcing that order.
///
/// A mutex round the listing came first. It protected the two tests that took
/// it, and four tests wrote or deleted saves without it. A lock is a rule each
/// new test has to remember; a directory of one's own is a shape in which the
/// race cannot be written, because no test can see another test's files at
/// all. So the lock is gone, not kept alongside — there is nothing left for it
/// to guard.
///
/// And forgetting this is loud: see
/// [`an_unscoped_save_is_refused`].
pub(crate) struct Saves {
    pub(crate) path: PathBuf,
    _scope: saves::ScopedDir,
}

impl Saves {
    pub(super) fn new(tag: &str) -> Saves {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        an_unscoped_save_is_refused();
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("saves-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a directory of the test's own");
        let scope = saves::scoped_dir(&path);
        Saves { path, _scope: scope }
    }

    /// Every file in the directory, sorted — **read from the directory, not
    /// through `saves::list`**, which shows only `.l2sav` files and so cannot
    /// see a `.part` left behind or a file written under another name.
    pub(crate) fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.path)
            .expect("the test's own directory")
            .map(|e| e.expect("an entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Saves {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn file(name: &str) -> String {
    format!("{name}.{}", save::EXTENSION)
}

/// Where `LORDS2_SAVES` points for this whole process: **a path under a regular
/// file**, so no directory can ever be created there and every write to it
/// fails
/// own therefore fails on its first write
/// directory with every other forgetful test and failing one run in a hundred.
///
/// Set once and never changed, which is the only safe way to use a
/// process-global: no test ever needs a *different* value.
pub(super) fn an_unscoped_save_is_refused() -> PathBuf {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let plug = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("a-save-test-without-a-Saves-of-its-own-cannot-write");
    let dir = plug.join("saves");
    ONCE.call_once(|| {
        // Shared by every run of this binary and only ever written with the
        // same bytes, so two processes at once are harmless.
        let _ = std::fs::write(&plug, b"see crates/l2-game/tests/save/main.rs, `Saves`\n");
        assert!(plug.is_file(), "{} must be a file, so nothing can be saved under it", plug.display());
        std::env::set_var(saves::DIR_VAR, &dir);
    });
    dir
}

/// The safety net above, observed: `LORDS2_SAVES` decides when nothing is
/// scoped, a write there is refused and says where, a scoped directory outranks
/// it
#[test]
fn a_test_without_a_save_directory_of_its_own_cannot_write_a_save() {
    let refused = an_unscoped_save_is_refused();
    assert_eq!(saves::dir(), Some(refused.clone()), "LORDS2_SAVES decides when nothing is scoped");
    let err = saves::write("unscoped", &furnished(1)).expect_err("nowhere to write");
    assert!(err.to_string().contains("without-a-Saves-of-its-own"), "{err}");

    let own = Saves::new("outranks");
    assert_eq!(saves::dir().as_deref(), Some(own.path.as_path()), "a scoped directory outranks it");
    saves::write("scoped", &furnished(1)).expect("and a scoped write succeeds");
    drop(own);
    assert_eq!(saves::dir(), Some(refused), "the scope ends with its guard");
}

/// **No test can see another test's saves** — which is what makes it safe for
/// a test to depend on the listing's shape, and what the lock never gave.
///
/// The other thread writes a save that sorts first, and holds its directory
/// open while this thread lists and opens the load screen. Make the scope
/// process-global instead of per-thread and both of those see it.
#[test]
fn a_save_written_by_another_test_is_in_neither_this_listing_nor_this_load_screen() {
    let _own = Saves::new("isolation-here");
    std::thread::scope(|s| {
        let (written_tx, written) = std::sync::mpsc::channel::<()>();
        let (release, release_rx) = std::sync::mpsc::channel::<()>();
        s.spawn(move || {
            let _theirs = Saves::new("isolation-there");
            saves::write("aaa another test's save", &furnished(1)).expect("write");
            written_tx.send(()).expect("the listing thread is waiting");
            // Held until this thread has looked. A panic over there drops
// `release`, and this returns.
            let _ = release_rx.recv();
        });
        written.recv().expect("the other thread wrote its save");
        assert_eq!(saves::list(), Vec::new(), "another test's save is in this test's listing");
        let screen = SaveLoadScreen::new(Mode::Load);
        assert!(
            screen.entries().is_empty(),
            "another test's save is on this test's load screen: {:?}",
            screen.entries()
        );
        drop(release);
    });
}

#[test]
fn a_save_written_to_disk_reads_back_as_the_same_game() {
    let own = Saves::new("round-trip");
    assert_eq!(saves::dir().as_deref(), Some(own.path.as_path()), "the scoped directory is what decides");

    let game = played(2);
    let name = "round trip";
    let path = saves::write(name, &game).expect("write");
    assert_eq!(path.extension().unwrap(), save::EXTENSION, "ours, and never .sav");
    assert!(path.starts_with(&own.path), "a save must land in the save directory: {path:?}");

    let back = saves::read(name, Tables::DEFAULT).expect("read");
    assert_eq!(back, game);

    assert!(saves::list().iter().any(|e| e.name == name), "and it is in the listing");
    saves::remove(name).expect("remove");
    assert_eq!(own.files(), Vec::<String>::new(), "and removing it removes it");
}

/// **One capital letter, and it is load-bearing.** NTFS already returns names
/// in case-insensitive order, so three lower-case names come back sorted with
/// the sort deleted and this test would pass on the file system's say-so.
/// `M` sorts before `a` by byte and after it on NTFS, so here the two orders
/// disagree and only `list`'s own sort can produce the answer.
#[test]
fn the_listing_is_sorted_by_name_and_not_by_whatever_the_file_system_says() {
    let _own = Saves::new("sorted");
    let game = furnished(3);
    for n in ["zzz sorted", "aaa sorted", "Mmm sorted"] {
        saves::write(n, &game).expect("write");
    }
    let listed: Vec<String> = saves::list().into_iter().map(|e| e.name).collect();
    // The whole listing, not a filtered one: the directory is this test's alone.
    assert_eq!(listed, vec!["Mmm sorted", "aaa sorted", "zzz sorted"]);
}

#[test]
fn a_failed_write_cannot_destroy_the_save_it_was_replacing() {
    // The write goes to a `.part` file and is renamed over the target, so the
    // only way the target changes is a rename that succeeded. What is asserted
    // here is the visible consequence: after a successful overwrite
    // `.part` left behind
    //
    // **Against the directory, not the listing.** This used to ask `saves::list`
    // whether any name ended in `.part` — and `list` keeps only `.l2sav` files,
    // so `overwritten.l2sav.part` could never have been in it.
    let own = Saves::new("overwrite");
    let name = "overwritten";
    let first = furnished(1);
    let second = played(1);
    saves::write(name, &first).expect("write");
    saves::write(name, &second).expect("overwrite");
    let back = saves::read(name, Tables::DEFAULT).expect("read");
    assert_eq!(back, second);
    assert_eq!(own.files(), vec![file(name)], "a temporary file was left in the save directory");
}

