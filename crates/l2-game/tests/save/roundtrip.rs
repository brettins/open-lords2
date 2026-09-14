#![allow(unused_imports)]
use super::*;
use super::isolation::*;
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

/// A game with something in every field the format writes: two realms in play,
/// a mix of owned and unowned counties, stock, anchors, colours, a selection
/// and turns behind it.
///
/// Deliberately not `Game::new`: a save format tested only on zeros is a save
/// format tested only on zeros.
///
/// **Also where the save-directory safety net goes up.** Nothing can be saved
/// without a `Game`
/// forgot its [`Saves`] meets [`an_unscoped_save_is_refused`] before it can
/// write — whatever order the harness happens to run the tests in.
pub(crate) fn furnished(seed: u64) -> Game {
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
        // Not the default
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

/// The lockstep digest of a kingdom — the number a peer would exchange
/// one this file compares two timelines with.
pub(crate) fn digest(k: &Kingdom) -> u64 {
    l2_kingdom::save::checksum(k)
}

/// A game with `n` turns played through the phase machine.
pub(crate) fn played(n: usize) -> Game {
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

