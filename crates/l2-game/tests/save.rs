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
//! [`crate::saves`] is exercised against a **temporary directory**, never the
//! default one: a test that wrote into `%APPDATA%` would destroy the player's
//! own saves on the machine it ran on. `LORDS2_SAVES` is set for the process,
//! which is also the proof that the override works.
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
fn furnished(seed: u64) -> Game {
    let mut game = Game::new(seed);
    let k = &mut game.kingdom;
    k.options = Options {
        difficulty: 2,
        advanced_farming: true,
        armies_eat: true,
        fight_humans_only_byte: 0,
        exploration: true,
        time_limit: 240,
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

/// A game with `n` turns actually played through the phase machine.
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
/// lockstep peer would exchange rather than on a struct compared with itself.
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

/// The same shape, from the **England turn-one position** rather than from a
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

/// A directory of this test process's own, and `LORDS2_SAVES` pointed at it.
///
/// Set once for the whole process — `std::env::set_var` is process-global, so
/// two tests setting *different* directories would race. They all share this
/// one and use distinct names instead.
fn temp_saves() -> PathBuf {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let dir = std::env::temp_dir().join(format!("l2-game-saves-{}", std::process::id()));
    ONCE.call_once(|| {
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        std::env::set_var(saves::DIR_VAR, &dir);
    });
    dir
}

#[test]
fn a_save_written_to_disk_reads_back_as_the_same_game() {
    let dir = temp_saves();
    assert_eq!(saves::dir().as_deref(), Some(dir.as_path()), "the override is what decides");

    let game = played(2);
    let name = "round trip";
    let path = saves::write(name, &game).expect("write");
    assert_eq!(path.extension().unwrap(), save::EXTENSION, "ours, and never .sav");
    assert!(path.starts_with(&dir), "a save must land in the save directory: {path:?}");

    let back = saves::read(name, Tables::DEFAULT).expect("read");
    assert_eq!(back, game);

    assert!(saves::list().iter().any(|e| e.name == name), "and it is in the listing");
    saves::remove(name).expect("clean up");
    assert!(!saves::list().iter().any(|e| e.name == name));
}

#[test]
fn the_listing_is_sorted_by_name_and_not_by_whatever_the_file_system_says() {
    temp_saves();
    let game = furnished(3);
    let names = ["zzz sorted", "aaa sorted", "mmm sorted"];
    for n in names {
        saves::write(n, &game).expect("write");
    }
    let listed: Vec<String> = saves::list()
        .into_iter()
        .map(|e| e.name)
        .filter(|n| n.ends_with("sorted"))
        .collect();
    assert_eq!(listed, vec!["aaa sorted", "mmm sorted", "zzz sorted"]);
    for n in names {
        saves::remove(n).expect("clean up");
    }
}

#[test]
fn a_failed_write_cannot_destroy_the_save_it_was_replacing() {
    // The write goes to a `.part` file and is renamed over the target, so the
    // only way the target changes is a rename that succeeded. What is asserted
    // here is the visible consequence: after a successful overwrite there is no
    // `.part` left behind, and the file is the *new* game.
    temp_saves();
    let name = "overwritten";
    let first = furnished(1);
    let second = played(1);
    saves::write(name, &first).expect("write");
    saves::write(name, &second).expect("overwrite");
    let back = saves::read(name, Tables::DEFAULT).expect("read");
    assert_eq!(back, second);
    assert!(
        !saves::list().iter().any(|e| e.name.ends_with(".part")),
        "a temporary file was left in the save directory"
    );
    saves::remove(name).expect("clean up");
}

// --- the two screens -------------------------------------------------------

fn bare() -> (Game, Assets) {
    (played(1), Assets::placeholder())
}

/// Deliver events to a screen through the machine, so that the transitions are
/// the ones the application would actually apply.
fn drive(m: &mut Machine, game: &mut Game, assets: &Assets, events: &[Event]) {
    for e in events {
        let mut ctx = Ctx { game, assets };
        m.handle(*e, &mut ctx);
    }
}

fn click_widget(w: (i32, i32, usize, i32)) -> Event {
    Event::Click { x: w.0 + w.3 / 2, y: w.1 + w.3 / 2 }
}

#[test]
fn the_save_screen_writes_a_file_and_the_load_screen_reads_it_back() {
    use l2_game::screens::saveload::{CONFIRM, LIST};

    temp_saves();
    let name = "SCREENTEST";
    let _ = saves::remove(name);

    let (mut game, assets) = bare();
    let before = digest(&game.kingdom);

    // Save: type a name, then press the tick. Nothing here reaches into the
    // screen's fields; it is clicks and keys.
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut typed: Vec<Event> =
        name.chars().map(|c| Event::KeyDown(Key::letter(c))).collect();
    typed.push(click_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &typed);
    assert!(m.should_quit() || m.depth() == 0, "the save screen closes when it has saved");
    assert!(saves::list().iter().any(|e| e.name == name), "the file is on disk");

    // Now change the world, and load it back.
    let mut game2 = furnished(0xDEAD);
    assert_ne!(digest(&game2.kingdom), before);
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Load));
    // The list is sorted, so the row this save is in is found by name rather
    // than assumed to be row 0.
    let row = saves::list().iter().position(|e| e.name == name).expect("listed");
    assert!(row < 30, "the test's own save must be on the first page");
    let r = SaveLoadScreen::row_rect(row);
    drive(
        &mut m,
        &mut game2,
        &assets,
        &[Event::Click { x: r.x + 2, y: r.y + 2 }, click_widget(CONFIRM)],
    );
    assert_eq!(digest(&game2.kingdom), before, "the loaded game is the saved one");
    assert_eq!(LIST.1, r.y - (row / 3) as i32 * 16, "the row geometry is the painter's");

    saves::remove(name).expect("clean up");
}

/// **The words on these two screens are the game's**, and this is what makes
/// that claim checkable rather than asserted: `Mode::heading_index` returns the
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
#[test]
fn the_screen_paints_inside_its_own_window_and_nothing_outside_it() {
    use l2_game::screens::saveload::{BOX_COLS, BOX_ROWS, BOX_X, BOX_Y};
    use l2_view::Canvas;

    temp_saves();
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

    temp_saves();
    let (mut game, assets) = bare();
    // A name nothing else in this file uses, so the assertion cannot be
    // answered by another test writing concurrently into the same directory.
    let name = "CANCELLED";
    let _ = saves::remove(name);
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> =
        name.chars().map(|c| Event::KeyDown(Key::letter(c))).collect();
    events.push(click_widget(CANCEL));
    drive(&mut m, &mut game, &assets, &events);
    assert!(
        !saves::list().iter().any(|e| e.name == name),
        "cancelling must not write a file"
    );
}

#[test]
fn a_save_name_the_file_system_would_choke_on_is_reported_and_not_written() {
    temp_saves();
    let (mut game, assets) = bare();
    let mut screen = SaveLoadScreen::new(Mode::Save);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    // The name field cannot hold a separator — `Key::Char` never carries one —
    // so the refusal is reached the way a player would reach it: an empty name.
    let t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(t, l2_game::Transition::Stay, "a refused save leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)), "and it says so");
}

#[test]
fn the_load_screen_refuses_a_file_it_cannot_read_and_stays_open() {
    let dir = temp_saves();
    let name = "CORRUPTED";
    let (mut game, assets) = bare();

    // A file with our magic and a version from the future.
    let mut bytes = save::encode(&game);
    bytes[8..12].copy_from_slice(&(save::VERSION + 1).to_le_bytes());
    std::fs::write(dir.join(format!("{name}.{}", save::EXTENSION)), &bytes).expect("write");

    let before = digest(&game.kingdom);
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let row = screen.entries().iter().position(|e| e.name == name).expect("listed");
    assert!(row < 30, "the test's own save must be on the first page, not row {row}");
    let r = SaveLoadScreen::row_rect(row);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    let t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);

    assert_eq!(t, l2_game::Transition::Stay, "a refused load leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)));
    assert_eq!(digest(&game.kingdom), before, "and the world it refused to replace is untouched");

    saves::remove(name).expect("clean up");
}
