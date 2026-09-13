//! **Saving a game and loading it back**, and the one test that can fail for a
//! real reason.
//!
//! ```text
//! cargo test -p l2-game --test save                       # all of it but one
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test save
//! ```
//!
//! # Why comparing the two structs is not enough
//!
//! `Game` derives `PartialEq`, so `assert_eq!(saved, loaded)` is one line and
//! proves something — but it is the *weak* form of the claim, because it only
//! ever compares the loader against the writer. A field that both of them
//! ignore round-trips perfectly and is gone.
//!
//! So the test that carries the weight is [`ten_seasons_from_a_reloaded_game_are_the_same_ten`]:
//! it takes one game, saves it, loads the save, and then **plays both forward
//! ten seasons**, requiring the `l2_net::Canonical` digest of the two kingdoms
//! to match after every single one. That digest is the same number a lockstep
//! peer exchanges every tick (`docs/netcode.md` §5), so a save that lost the
//! generator's state, or the history ring's head, or one county's dryness, does
//! not survive the first season that reads it — the digests part company and
//! the test names the season they parted on.
//!
//! A save format is only worth having if resuming is indistinguishable from
//! never having stopped. That is exactly what this measures.
//!
//! # And the files
//!
//! **Every test that touches a save has a directory of its own**, and [`Saves`]
//! is the only way this file gets one. It is never the default directory — a
//! test that wrote into `%APPDATA%` would destroy the player's own saves on the
//! machine it ran on — and it is never *shared*, because a shared one is what
//! made `the_save_screen_writes_a_file_and_the_load_screen_reads_it_back` fail
//! about one run in a hundred. See [`Saves`].
//!
//! Nothing here writes a `.sav` anywhere near the repository, and the extension
//! is `.l2sav` in any case — `l2_game::save::EXTENSION` says why.

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

/// A game with something in every field the format writes: two realms in play,
/// a mix of owned and unowned counties, stock, anchors, colours, a selection
/// and turns behind it.
///
/// Deliberately not `Game::new`: a save format tested only on zeros is a save
/// format tested only on zeros.
///
/// **Also where the save-directory safety net goes up.** Nothing can be saved
/// without a `Game`, and every game in this file starts here, so a test that
/// forgot its [`Saves`] meets [`an_unscoped_save_is_refused`] before it can
/// write — whatever order the harness happens to run the tests in.
fn furnished(seed: u64) -> Game {
    an_unscoped_save_is_refused();
    let mut game = Game::new(seed);
    let k = &mut game.kingdom;
    k.options = Options {
        difficulty: 2,
        advanced_farming: true,
        armies_eat: true,
        fight_humans_only_byte: 0,
        exploration: true,
        time_limit: 240,
        // Not the default, so a round trip that dropped it would come back
        // FAITHFUL and the test would pass anyway.
        quirks: l2_kingdom::Quirks::FIXED,
    };
    assert!(k.set_county_count(14));
    k.season = 1;
    k.season_next = 2;
    k.year = 1268;
    k.year_next = 1268;

    for id in 1..=5usize {
        let r = &mut k.realms[id];
        r.in_play = true;
        r.gold = 1000 + id as i32 * 37;
        r.iron = 40 + id as i32;
        r.stone = 50 + id as i32;
        r.wood = 60 + id as i32;
        r.weapons = [1, 2, 3, 4, 5, 6].map(|w| w * id as i32);
    }
    k.realms[1].is_human = true;

    for id in 1..=14usize {
        let c = &mut k.counties[id];
        c.owner = match id {
            1 | 4 | 8 => 1,
            11 | 13 => 2,
            _ => 0,
        };
        c.population = 400 + id as i32 * 11;
        c.happiness = 50 + id as i32;
        c.health_meter = 60 + id as i32;
        c.herd = 60 + id as i32 * 3;
        c.grain = 100 + id as i32 * 5;
        c.crop = [id as i32, id as i32 * 2, id as i32 * 3];
        c.fields_fallow = 3;
        c.fields_grain = 6;
        c.fields_cattle = 4;
        c.ration_wanted = 3;
        c.ration_split = (id * 7 % 101) as i32;
        c.tax_rate = (id % 13) as i32;
        c.dryness = 30 + id as i32;
        c.weather = Weather::ALL[id % 6];
    }

    // The interface's own ten fields, all of them different from each other so
    // that a swap between two of them would show.
    game.player = 1;
    game.map_slot = 42;
    game.realm_colour = [0, 5, 4, 3, 2, 1];
    game.selected = 8;
    for id in 0..game.anchor_x.len() {
        game.anchor_x[id] = (id * 3 % 64) as u8;
        game.anchor_y[id] = (id * 5 % 64) as u8;
    }
    game.gold_last = [0, 900, 1100, 1200, 1300, 1400];
    game.turns_played = 3;
    game
}

/// The lockstep digest of a kingdom — the number a peer would exchange, and the
/// one this file compares two timelines with.
fn digest(k: &Kingdom) -> u64 {
    l2_kingdom::save::checksum(k)
}

/// A game with `n` turns played through the phase machine.
fn played(n: usize) -> Game {
    let mut game = furnished(0x51A_7E5);
    for i in 0..n {
        assert!(turn::end_turn(&mut game).is_some(), "turn {i} did not come round");
    }
    game
}

// --- the round trip --------------------------------------------------------

#[test]
fn a_furnished_game_round_trips_field_for_field() {
    let game = furnished(99);
    let back = save::decode(&save::encode(&game), Tables::DEFAULT).expect("our own bytes");
    assert_eq!(back, game);
}

#[test]
fn a_game_with_turns_behind_it_round_trips_including_its_last_report() {
    for turns in [1usize, 2, 5] {
        let game = played(turns);
        assert_eq!(game.turns_played, 3 + turns as u32, "the interface's counter moved");
        assert!(game.last_report.is_some(), "a played turn leaves a report to redraw");
        let back = save::decode(&save::encode(&game), Tables::DEFAULT).expect("our own bytes");
        assert_eq!(back, game, "after {turns} turns");
    }
}

/// **The test that can fail for a real reason.**
///
/// Two timelines from the same save, ten seasons each, compared on the digest a
/// lockstep peer would exchange.
#[test]
fn ten_seasons_from_a_reloaded_game_are_the_same_ten() {
    let mut original = played(4);
    let mut resumed =
        save::decode(&save::encode(&original), Tables::DEFAULT).expect("our own bytes");
    assert_eq!(
        digest(&original.kingdom),
        digest(&resumed.kingdom),
        "the two kingdoms differ before a single season has run"
    );

    for season in 1..=10 {
        let a = turn::end_turn(&mut original).expect("the machine comes round");
        let b = turn::end_turn(&mut resumed).expect("the machine comes round");
        assert_eq!(
            digest(&original.kingdom),
            digest(&resumed.kingdom),
            "the saved game and the reloaded one diverged at season {season}"
        );
        assert_eq!(a.report, b.report, "the reports diverged at season {season}");
        assert_eq!(a.ticks, b.ticks, "the phase machine took a different route at season {season}");
        assert_eq!(original, resumed, "and every interface field too, at season {season}");
    }
}

/// The same shape, from the **England turn-one position**
/// game this file made up. The digest is a different one every time the fixture
/// is regenerated, so nothing here asserts its value — only that the two
/// timelines agree.
#[test]
fn ten_seasons_from_a_reloaded_england_are_the_same_ten() {
    let save = l2_testkit::england!();
    let mut original =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    assert!(original.kingdom.county_count >= 5, "the England map has counties in it");

    for _ in 0..3 {
        turn::end_turn(&mut original).expect("the machine comes round");
    }
    let bytes = save::encode(&original);
    let mut resumed = save::decode(&bytes, Tables::DEFAULT).expect("our own bytes");
    assert_eq!(resumed, original, "the England position reloads field for field");

    for season in 1..=6 {
        turn::end_turn(&mut original).expect("comes round");
        turn::end_turn(&mut resumed).expect("comes round");
        assert_eq!(
            digest(&original.kingdom),
            digest(&resumed.kingdom),
            "England diverged at season {season}"
        );
    }
    eprintln!(
        "England resumed: {} bytes, {} seasons matched",
        bytes.len(),
        original.kingdom.turn_count
    );
}

// --- refusals --------------------------------------------------------------

#[test]
fn a_version_this_build_does_not_understand_is_named_rather_than_half_loaded() {
    let mut bytes = save::encode(&furnished(1));
    bytes[8..12].copy_from_slice(&(save::VERSION + 9).to_le_bytes());
    let err = save::decode(&bytes, Tables::DEFAULT).expect_err("a future save must be refused");
    assert_eq!(
        err,
        LoadError::UnsupportedVersion { found: save::VERSION + 9, supported: save::VERSION }
    );
    let text = err.to_string();
    assert!(text.contains(&(save::VERSION + 9).to_string()), "{text}");
    assert!(text.contains("Refusing rather than guessing"), "{text}");
}

#[test]
fn the_originals_own_save_is_not_mistaken_for_ours() {
    // `lastturn.sav` is a memory dump with no header at all; whatever its first
    // eight bytes are, they are not `L2GSAVE\x01`.
    let mut theirs = vec![0xABu8; 4096];
    theirs[..8].copy_from_slice(b"L2KSAVE\x01");
    assert_eq!(save::decode(&theirs, Tables::DEFAULT), Err(LoadError::NotASave));
}

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
/// **shape**: the listing is sorted, so a file another test writes or deletes
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
struct Saves {
    path: PathBuf,
    _scope: saves::ScopedDir,
}

impl Saves {
    fn new(tag: &str) -> Saves {
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
    fn files(&self) -> Vec<String> {
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
/// fails, naming the path. A test that touches saves without a [`Saves`] of its
/// own therefore fails on its first write, every time, instead of sharing a
/// directory with every other forgetful test and failing one run in a hundred.
///
/// Set once and never changed, which is the only safe way to use a
/// process-global: no test ever needs a *different* value.
fn an_unscoped_save_is_refused() -> PathBuf {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let plug = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("a-save-test-without-a-Saves-of-its-own-cannot-write");
    let dir = plug.join("saves");
    ONCE.call_once(|| {
        // Shared by every run of this binary and only ever written with the
        // same bytes, so two processes at once are harmless.
        let _ = std::fs::write(&plug, b"see crates/l2-game/tests/save.rs, `Saves`\n");
        assert!(plug.is_file(), "{} must be a file, so nothing can be saved under it", plug.display());
        std::env::set_var(saves::DIR_VAR, &dir);
    });
    dir
}

/// The safety net above, observed: `LORDS2_SAVES` decides when nothing is
/// scoped, a write there is refused and says where, a scoped directory outranks
/// it, and the scope ends with its guard.
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
    // here is the visible consequence: after a successful overwrite there is no
    // `.part` left behind, and the file is the *new* game.
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

// --- the two screens -------------------------------------------------------

fn bare() -> (Game, Assets) {
    (played(1), Assets::placeholder())
}

/// Deliver events to a screen through the machine, so that the transitions are
/// the ones the application would apply.
fn drive(m: &mut Machine, game: &mut Game, assets: &Assets, events: &[Event]) {
    for e in events {
        let mut ctx = Ctx { game, assets };
        m.handle(*e, &mut ctx);
    }
}

fn click_widget(w: (i32, i32, usize, i32)) -> Event {
    Event::Click { x: w.0 + w.3 / 2, y: w.1 + w.3 / 2 }
}

fn release_widget(w: (i32, i32, usize, i32)) -> Event {
    Event::Release { x: w.0 + w.3 / 2, y: w.1 + w.3 / 2 }
}

/// **`SaveLoad_Tick`'s 150 frames**, run through the machine until the screen
/// has closed or the count is spent.
fn settle(m: &mut Machine, game: &mut Game, assets: &Assets) {
    for _ in 0..=l2_game::screens::saveload::WORK_FRAMES {
        if m.depth() == 0 || m.should_quit() {
            return;
        }
        let mut ctx = Ctx { game, assets };
        m.update(&mut ctx);
    }
}

/// **The thumb up clicks on the press, writes nothing on it, and the save runs
/// 150 frames later.**
///
/// A player, twice: *"no sound on clicking the yes/no on save, and it is still
/// on mousedown instead of mouseup"*. Both halves are the binary's:
/// `g_saveLoadWidgets`' records are `Widget_Test` **kind 4**, which plays
/// `Sound_RestartSlot(1)` and calls the handler on the press, and the handler,
/// `FUN_004342F3`, is only `DAT_005CD41C = 100` — `SaveLoad_Tick` sets
/// `DAT_0057D3C4 = 0x96` and writes when that runs out. Ours answered a raw
/// click and wrote on the spot.
///
/// **Ablations, run:** make `SaveLoadScreen::begin` arm a one-frame wait and
/// *"written on frame 1, before SaveLoad_Tick's 150"* goes red; delete
/// `self.click()` from `Press::press` and the first click count reads 0.
#[test]
fn the_save_thumb_clicks_on_the_press_and_writes_a_hundred_and_fifty_frames_later() {
    use l2_game::screens::saveload::{CONFIRM, WORK_FRAMES};

    let own = Saves::new("latch");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "LATCH".chars().map(Event::Text).collect();
    // Pressed and let go. **A thumb held down never saves**: it is kind 4, its
    // repeat runs `FUN_004342F3` again, and each run restarts the 150 frames.
    events.push(click_widget(CONFIRM));
    events.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &events);

    assert_eq!(m.clicks(), 1, "Widget_Test kind 4 plays Sound_RestartSlot(1) on the press");
    assert!(own.files().is_empty(), "nothing is written on the press");
    assert_eq!(m.depth(), 1, "and the box is still up");
    for t in 1..WORK_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        assert!(own.files().is_empty(), "written on frame {t}, before SaveLoad_Tick's {WORK_FRAMES}");
    }
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(own.files(), vec![file("latch")], "written on frame {WORK_FRAMES}");
    assert!(m.should_quit() || m.depth() == 0, "and the box closed");
    assert_eq!(m.clicks(), 1, "the save itself is silent");
}

#[test]
fn the_save_screen_writes_a_file_and_the_load_screen_reads_it_back() {
    use l2_game::screens::saveload::{CONFIRM, LIST};

    let own = Saves::new("screens");
    // Four saves that sort before the one this test makes, all of a different
    // game, so a click on any row but the right one loads the wrong world and
    // the digest says so.
    let decoy = furnished(0xDEC0);
    let decoys = ["a decoy", "b decoy", "c decoy", "d decoy"];
    for n in decoys {
        saves::write(n, &decoy).expect("write");
    }

    // **What is typed and what is saved are different strings now**, and the
    // difference is the point. The save box is `Edit_Begin(&DAT_004EA130, 8,
    // 0xA0, 1)` — **kind 1**, a DOS file name — so `Edit_TypeChar` runs
    // `A`–`Z` through `0x004011B0` and lower-cases them. Typing `SCREENTEST`
    // gives `screentest`, which is what the original's own file box does and
    // what its file list shows.
    let typed_name = "SCREEN TEST";
    let name = "screen test";

    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let before = digest(&game.kingdom);
    assert_ne!(digest(&decoy.kingdom), before);

    // Save: type a name, then press the tick. Nothing here reaches into the
    // screen's fields; it is clicks and keys.
    //
    // **`Event::Text`, not `Event::KeyDown`.** They are `WM_CHAR` and
    // `WM_KEYDOWN` and the field reads the first, exactly as the original's
    // does — `Key::Char` is folded to upper case for the hotkey matchers, so a
    // field fed from it could never produce a lower-case letter at all. This
    // test drove the old hand-rolled field through the hotkey message and is
// the reason that distinction is now enforced. The
    // space in the middle is deliberate: a space is a character here, not a
    // confirm, and driving it proves the field takes it.
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut typed: Vec<Event> = typed_name.chars().map(Event::Text).collect();
    typed.push(click_widget(CONFIRM));
    typed.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &typed);
    settle(&mut m, &mut game, &assets);
    assert!(m.should_quit() || m.depth() == 0, "the save screen closes when it has saved");
    let mut expected: Vec<String> = decoys.iter().map(|n| file(n)).collect();
    expected.push(file(name));
    assert_eq!(own.files(), expected, "the file is on disk, and nothing else was written");

    // Now change the world, and load it back.
    let mut game2 = furnished(0xDEAD);
    game2.prefs.tip_screens = false;
    assert_ne!(digest(&game2.kingdom), before);
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Load));
    // **Row 4, and it is a literal.** The directory holds exactly the five
    // files asserted above, the list is sorted, and the four decoys sort first,
    // so the list this screen opened on has `screen test` fifth — the middle
    // column of the second line, which exercises both halves of the geometry.
    //
    // It used to be found by taking a *second* `saves::list()` after the
    // screen had opened. That answered for the directory as it stood at that
    // statement, not as the screen had read it, and in a directory shared with
    // other tests those were sometimes different lists.
    const ROW: usize = 4;
    let r = SaveLoadScreen::row_rect(ROW);
    drive(
        &mut m,
        &mut game2,
        &assets,
        &[Event::Click { x: r.x + 2, y: r.y + 2 }, click_widget(CONFIRM), release_widget(CONFIRM)],
    );
    settle(&mut m, &mut game2, &assets);
    assert_eq!(digest(&game2.kingdom), before, "the loaded game is the saved one");
    assert_eq!(LIST.1, r.y - (ROW / 3) as i32 * 16, "the row geometry is the painter's");
}

/// **A click means the row that was drawn, whatever the directory holds now.**
///
/// The screen reads the directory once, when it opens, and both what it paints
/// and what a click resolves against are that one read. So a save that appears
/// while the screen is up — a second copy of the game saving, a file copied in
/// by hand — cannot move a row out from under the pointer. The test above used
/// to get this wrong in its own code, by resolving a click against a second
/// read; this is the property that kept the screen itself from ever doing so.
///
/// Make the click arm re-read the directory and this loads the save that
/// arrived.
#[test]
fn a_click_on_the_load_screen_means_the_row_it_drew_even_if_a_save_arrived_since() {
    let _own = Saves::new("arrived");
    let (mut game, assets) = bare();
    let shown = furnished(0x5409);
    saves::write("b on screen", &shown).expect("write");
    let mut screen = SaveLoadScreen::new(Mode::Load);

    // One that sorts first, written after the screen opened.
    let arrived = furnished(0xA441);
    saves::write("a arrived later", &arrived).expect("write");
    assert_ne!(digest(&arrived.kingdom), digest(&shown.kingdom));
    assert_ne!(digest(&game.kingdom), digest(&shown.kingdom));

    // Row 0: the screen opened on a directory with one save in it.
    let r = SaveLoadScreen::row_rect(0);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    // Enter is `Edit_Confirm`, the same latch as the thumb up: the load runs when
    // `SaveLoad_Tick`'s count has run out.
    l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    let mut t = l2_game::Transition::Stay;
    for _ in 0..=l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
        if t != l2_game::Transition::Stay {
            break;
        }
    }
    assert_eq!(t, l2_game::Transition::Pop, "the load went through");
    assert_eq!(
        digest(&game.kingdom),
        digest(&shown.kingdom),
        "the click loaded a save that was not the row it landed on"
    );
}

/// **The words on these two screens are the game's**, and this is what makes
/// that claim checkable: `Mode::heading_index` returns the
/// painter's own `saving` argument, and `Eng_DrawString(40, saving, …)` uses it
/// as a string index, so index 0 and index 1 have to be those two sentences in
/// the user's own `L2.eng`.
#[test]
fn the_headings_and_the_status_line_are_l2_engs_own_words() {
    use l2_game::screens::saveload::{ERROR_INDEX, GROUP};
    use l2_game::shell::Eng;

    let Some(dir) = l2_testkit::install_dir() else {
        eprintln!("SKIP the_headings_and_the_status_line_are_l2_engs_own_words: no game install");
        return;
    };
    let e = Eng::parse(std::fs::read(dir.join("L2.eng")).expect("L2.eng")).expect("parses");

    assert_eq!(e.get(GROUP, Mode::Load.heading_index()), Some("Loading a conquest."));
    assert_eq!(e.get(GROUP, Mode::Save.heading_index()), Some("Saving a conquest."));
    assert_eq!(e.get(GROUP, Mode::Load.working_index()), Some("Loading game. Please wait."));
    assert_eq!(e.get(GROUP, Mode::Save.working_index()), Some("Saving game. Please wait."));
    assert_eq!(e.get(GROUP, ERROR_INDEX), Some("File error. Operation canceled."));
}

/// It paints inside the window the painter opens and **nowhere else**, on a
/// machine with no artwork at all — which is what proves the layout does not
/// depend on the install being present.
///
/// **With a save directory too long to fit**, on every machine. The screen
/// writes its directory along the bottom of the box, and this test used to run
/// with a short temporary one — so it passed while a long directory ran out of
/// the side, and went red only when its own directory happened to get longer.
#[test]
fn the_screen_paints_inside_its_own_window_and_nothing_outside_it() {
    use l2_game::screens::saveload::{BOX_COLS, BOX_ROWS, BOX_X, BOX_Y};
    use l2_view::Canvas;

    let _own = Saves::new(&"a-save-directory-far-too-long-to-fit-in-the-box".repeat(2));
    assert!(saves::dir().unwrap().display().to_string().len() > 100, "long enough to overflow");
    let (mut game, assets) = bare();
    let mut screen = SaveLoadScreen::new(Mode::Save);
    let mut canvas = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        l2_game::Screen::draw(&mut screen, &ctx, &mut canvas);
    }
    let mut outside = 0usize;
    for y in 0..480i32 {
        for x in 0..640i32 {
            let inside = x >= BOX_X
                && x < BOX_X + BOX_COLS * 16
                && y >= BOX_Y
                && y < BOX_Y + BOX_ROWS * 16;
            if !inside && canvas.at(x as usize, y as usize) != 0 {
                outside += 1;
            }
        }
    }
    assert_eq!(outside, 0, "an overlay must not paint outside its own box");
    assert!(canvas.count(0) < 640 * 480, "and it must paint something inside it");
}

#[test]
fn the_cancel_cross_closes_without_writing_anything() {
    use l2_game::screens::saveload::CANCEL;

    let own = Saves::new("cancel");
    let (mut game, assets) = bare();
    // **A name that would be written if the cross confirmed**, typed the way
    // the field reads it. This used to type through `Event::KeyDown`, which the
    // field ignores, and then look for `CANCELLED` — a name the file-name field
    // would have lower-cased — so a cross that saved could not have turned it
    // red twice over. An empty directory of its own is the whole assertion now.
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "CANCELLED".chars().map(Event::Text).collect();
    events.push(click_widget(CANCEL));
    drive(&mut m, &mut game, &assets, &events);
    assert!(m.should_quit() || m.depth() == 0, "the cross closes the screen");
    assert_eq!(own.files(), Vec::<String>::new(), "cancelling must not write a file");
}

#[test]
fn a_save_name_the_file_system_would_choke_on_is_reported_and_not_written() {
    let own = Saves::new("refused-name");
    let (mut game, assets) = bare();
    let mut screen = SaveLoadScreen::new(Mode::Save);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    // The name field cannot hold a separator — `Key::Char` never carries one —
    // so the refusal is reached the way a player would reach it: an empty name.
    // Enter arms `SaveLoad_Tick`'s latch, and the refusal comes when it runs out.
    let mut t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
    }
    assert_eq!(t, l2_game::Transition::Stay, "a refused save leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)), "and it says so");
    assert_eq!(own.files(), Vec::<String>::new(), "and nothing was written");
}

#[test]
fn the_load_screen_refuses_a_file_it_cannot_read_and_stays_open() {
    let own = Saves::new("corrupted");
    let name = "CORRUPTED";
    let (mut game, assets) = bare();

    // A file with our magic and a version from the future.
    let mut bytes = save::encode(&game);
    bytes[8..12].copy_from_slice(&(save::VERSION + 1).to_le_bytes());
    std::fs::write(own.path.join(file(name)), &bytes).expect("write");

    let before = digest(&game.kingdom);
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let row = screen.entries().iter().position(|e| e.name == name).expect("listed");
    assert!(row < 30, "the test's own save must be on the first page, not row {row}");
    let r = SaveLoadScreen::row_rect(row);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    // Enter arms `SaveLoad_Tick`'s latch; the load is attempted when it runs out.
    let mut t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
    }

    assert_eq!(t, l2_game::Transition::Stay, "a refused load leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)));
    assert_eq!(digest(&game.kingdom), before, "and the world it refused to replace is untouched");
}

// --- the rolling autosave --------------------------------------------------

/// **`Save_RotateAndWrite` (`0x0049A453`) keeps three turns, not one.**
///
/// ```c
/// if (DAT_00553260 < 1) {
///     remove(safeturn); rename(old_turn, safeturn); rename(lastturn, old_turn);
/// }
/// Save_Write(lastturn);
/// ```
///
/// So after three writes the three files hold three *different* games, newest
/// first, and a fourth pushes the oldest off the end
/// series. Asserted on the kingdom digest and not on the file names, because
/// names would pass with the same bytes written three times.
///
/// Ablation: delete either `rename` in [`saves::rotate_and_write`] and two of
/// the three digests become equal.
#[test]
fn the_autosave_keeps_the_last_three_turns_newest_first() {
    let own = Saves::new("autosave-rotation");
    let mut game = played(1);
    let mut want = Vec::new();
    for _ in 0..3 {
        want.insert(0, digest(&game.kingdom));
        saves::rotate_and_write(&game).expect("the autosave is written");
        assert!(turn::end_turn(&mut game).is_some(), "a turn between two autosaves");
    }
    assert_eq!(
        own.files(),
        vec![file("lastturn"), file("old_turn"), file("safeturn")],
        "the original's three names, and nothing else"
    );
    let read = |n: &str| digest(&saves::read(n, Tables::DEFAULT).expect("readable").kingdom);
    let got: Vec<u64> = saves::AUTOSAVES.iter().map(|n| read(n)).collect();
    assert_eq!(got, want, "lastturn is the newest and safeturn the oldest");

    want.insert(0, digest(&game.kingdom));
    want.pop();
    saves::rotate_and_write(&game).expect("written");
    let got: Vec<u64> = saves::AUTOSAVES.iter().map(|n| read(n)).collect();
    assert_eq!(got, want, "three deep, always");
    assert_eq!(own.files().len(), 3, "no fourth file");
}

/// **The first autosave of a game has nothing to rotate**, and the original
/// does not care: its `remove` and both `rename`s fail and it writes
/// anyway. Ours must not turn that into a failure a player is told about.
#[test]
fn the_first_autosave_of_a_game_writes_one_file_and_reports_no_failure() {
    let own = Saves::new("autosave-first");
    let game = played(1);
    saves::rotate_and_write(&game).expect("nothing to rotate is not a failure");
    assert_eq!(own.files(), vec![file("lastturn")]);
}

/// **The autosave is our own directory's, never the install's** — `CLAUDE.md`
/// rule 2, and the one thing about this feature that must never regress. The
/// original keeps its rotation *inside the game directory*, which is exactly
/// where `docs/environment.md` has already watched a program destroy a
/// fixture's identity.
#[test]
fn the_autosave_lands_where_every_other_save_of_ours_lands() {
    let own = Saves::new("autosave-where");
    let game = played(1);
    let path = saves::rotate_and_write(&game).expect("written");
    assert!(path.starts_with(&own.path), "{} is not under the save directory", path.display());
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some(save::EXTENSION));
}

/// **The end of a turn asks for one, and `Game_NewGame` asks for one** —
/// `Save_RotateAndWrite`'s only two call sites, driven through the machine.
///
/// **The whole chain, pumped through [`saves::run_pending`]** — the fade raises
/// it, the machine drains it, and that one function is the only thing in the
/// workspace that turns it into a file. `main.rs` is these lines and a line that
/// prints a failure, so they are here and not there.
///
/// **It must be raised in the dark, not on the button and not on the far side
/// of the light.** `FUN_0049A3E6` runs on `g_screenId == 0x24`, after the fade
/// has bottomed out and the seasonal art has been reloaded, so what the file
/// holds is the *opening* of the turn that just began — which is what the last
/// assertion reads back.
///
/// Ablations: delete the `self.autosave = true` in `tick_fade` and the turn
/// never asks; move it out of the `is_darkest` guard and it asks many times;
/// drop the `take_autosave` drain from `Machine::update` and no file appears.
#[test]
fn ending_a_turn_asks_for_exactly_one_autosave_and_asks_in_the_dark() {
    let own = Saves::new("autosave-turn");
    let (mut game, assets) = bare();
    // **Tip screens: No.** A new game's tips hold the campaign map's input on
    // screen `0x27`, which is `tests/tips.rs`'s subject and not this one's.
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    let end = l2_game::screens::map::END_TURN_BUTTON;
    drive(&mut m, &mut game, &assets, &[Event::Click { x: end.centre_x(), y: end.y + 4 }]);
    assert!(
        saves::run_pending(&mut m, &game).is_none() && own.files().is_empty(),
        "the button itself does not autosave"
    );

    let before = game.kingdom.turn_count;
    let mut asked = Vec::new();
    let mut came_round = None;
    for t in 0..l2_game::turn::MAX_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        // `main.rs`'s tick, after `Machine::update` and before the audio.
        if let Some(r) = saves::run_pending(&mut m, &game) {
            r.expect("the autosave is written");
            asked.push(t);
        }
        if came_round.is_none() && game.kingdom.turn_count != before {
            came_round = Some(t);
        }
        // Long enough after the turn came round for the whole fade to run.
        if came_round.is_some_and(|r| t > r + 4 * l2_view::fade::PHASES as u32) {
            break;
        }
    }
    let round = came_round
        .unwrap_or_else(|| panic!("the End Turn button ends a turn; stack {:?}", m.ids()));
    assert_eq!(asked.len(), 1, "one turn, one autosave: asked on ticks {asked:?}");
    assert!(asked[0] > round, "the autosave is written after the turn, not before it");
    assert_eq!(own.files(), vec![file("lastturn")], "one turn, one file");
    // **The opening of the turn that just began**, not the end of the one that
    // finished: `Season_FinishFade` writes after `Season_Advance`.
    let back = saves::read("lastturn", Tables::DEFAULT).expect("readable");
    assert_eq!(digest(&back.kingdom), digest(&game.kingdom));
    assert_ne!(back.kingdom.turn_count, before, "the turn on disk is the new one");
}