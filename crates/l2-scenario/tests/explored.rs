//! **The fog of war against the bits the original wrote.**
//!
//! `l2_kingdom::explore` reads the seen bit's writers out of the decompilation.
//! That is a claim about code; this file holds it against *data*. A saved game
//! carries bank bit `0x20` in its `g_tiles` block exactly where the original's
//! writers put it, so the rules, run over the same position, have to land on
//! the same bits.
//!
//! **What the corpus can and cannot settle, stated.** The England turn-one
//! fixture settles the county rule *exactly* — at turn one the local player has
//! a county and no army, so every seen bit in it came from one call. The battle
//! and turn saves carry armies that have walked, and nothing in a save records
//! the path, so for those the rules give a **lower bound**: every tile they
//! must have set is set. That checks the army square's radius from below and
//! cannot check it from above; `crates/l2-kingdom/tests/explore.rs` holds the
//! 6 to the decompiled literal instead.

use l2_formats::maps::MapSet;
use l2_formats::save::Save;
use l2_kingdom::explore::{Explored, ARMY_SIGHT};
use l2_kingdom::map::{coords, MAP_TILES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_kingdom::UnitKind;
use l2_scenario::newgame::NewGame;
use l2_scenario::Scenario;

/// `g_tiles` (`0x00522F90`), eight bytes a tile; the bank byte is `+2`.
const TILES: u32 = 0x0052_2F90;

fn file_bits(save: &Save) -> Vec<bool> {
    (0..MAP_TILES)
        .map(|t| save.u8_at(TILES + t as u32 * 8 + 2).expect("g_tiles is block 0") & 0x20 != 0)
        .collect()
}

/// What the rules say the local player must have seen of a saved position:
/// every county he holds with its border (`FUN_0046DFD5`), and thirteen by
/// thirteen round every army of his where it now stands (`Unit_Step`'s reveal
/// at the tile centre it stopped on, or `Army_Create`'s if it never moved).
/// Returns the plane and how many armies went into it.
fn predicted(s: &Scenario) -> (Explored, usize) {
    let local = s.local_player;
    let mut e = Explored::new();
    for id in s.county_ids() {
        if s.counties[id].as_ref().is_some_and(|c| c.owner == local) {
            e.reveal_county(local, &s.map, id as u8);
        }
    }
    let mut armies = 0;
    for (_, u) in &s.units {
        if u.kind == UnitKind::Army && u.owner == local {
            e.reveal_square(local, u.x as i32, u.y as i32, ARMY_SIGHT);
            armies += 1;
        }
    }
    (e, armies)
}

/// **At turn one the rules reproduce the file bit for bit.**
///
/// Only one writer can have run for the local player: `Game_SetupRealmsAndCounties`'
/// last statement, `FUN_0046DFD5(startCounty)` — the county and a one-tile
/// border. The fixture's local player has no army (its *Army size* was none), so
/// the prediction is that one call and the file must be exactly it.
///
/// **And the option was off.** The fixture's `g_optExploration` is 0 and the
/// bits are there anyway: none of the writers tests the option. That is what
/// lets a player turn the fog on in the middle of a game and see the ground his
/// armies have already walked.
///
/// Ablation: `COUNTY_BORDER` set to 0 in `l2_kingdom::explore` — the prediction
/// loses the ring and this names every tile of it.
#[test]
fn the_england_fixtures_seen_bits_are_exactly_the_players_county_and_its_border() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let local = s.local_player;
    assert!(!s.options.exploration, "the fixture was saved with Exploration off");

    let (want, armies) = predicted(&s);
    assert_eq!(armies, 0, "turn one, no garrison: one writer ran");
    let file = file_bits(&save);
    let only_in_file: Vec<(u8, u8)> =
        (0..MAP_TILES).filter(|&t| file[t] && !want.is_seen(local, t)).map(coords).collect();
    let only_predicted: Vec<(u8, u8)> =
        (0..MAP_TILES).filter(|&t| !file[t] && want.is_seen(local, t)).map(coords).collect();
    assert!(
        only_in_file.is_empty() && only_predicted.is_empty(),
        "in the file and not predicted: {only_in_file:?}\npredicted and not in the file: {only_predicted:?}"
    );
    let seen = file.iter().filter(|&&b| b).count();
    let county_tiles = (0..MAP_TILES)
        .filter(|&t| {
            s.counties.get(s.map.county[t] as usize).and_then(|c| c.as_ref()).is_some_and(|c| c.owner == local)
        })
        .count();
    assert!(seen > county_tiles, "the border is in the file too: {seen} seen, {county_tiles} in the county");
}

/// **Every saved game has seen at least what the rules say it must.**
///
/// Four saves from later in a game, with the local player's armies out on the
/// map. Each army's square and each county's bordered block must be in the file.
/// Whatever else is set is the history of where the armies walked, which a save
/// does not record, so this is a lower bound and says so.
#[test]
fn every_saved_game_has_seen_its_counties_and_round_its_armies() {
    let mut armies_checked = 0;
    for name in ["battle-before.sav", "battle-during.sav", "old_turn.sav", "safeturn.sav"] {
        let save: Save = l2_testkit::fixture!(name);
        let s = Scenario::from_save(&save).unwrap();
        let (want, armies) = predicted(&s);
        armies_checked += armies;
        let file = file_bits(&save);
        let missing: Vec<(u8, u8)> = (0..MAP_TILES)
            .filter(|&t| want.is_seen(s.local_player, t) && !file[t])
            .map(coords)
            .collect();
        assert!(missing.is_empty(), "{name}: the rules require these and the file lacks them: {missing:?}");
        let seen = file.iter().filter(|&&b| b).count();
        assert!(seen < MAP_TILES, "{name}: a file with every bit set would pass anything");
    }
    assert!(armies_checked > 0, "no save had an army of the local player's to check a square round");
}

/// **The import carries the file's bits to the local player, and to nobody
/// else** — a `.sav` cannot say what anybody else has seen, because every writer
/// is guarded on `g_localPlayer`. And the kingdom built from the scenario holds
/// the same plane, which is what the painters read.
#[test]
fn the_import_gives_the_files_bits_to_the_local_player_alone() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let file = file_bits(&save);

    for t in 0..MAP_TILES {
        assert_eq!(s.explored.is_seen(s.local_player, t), file[t], "tile {:?}", coords(t));
    }
    for realm in 0..MAX_REALMS as u8 {
        if realm != s.local_player {
            assert_eq!(s.explored.count(realm), 0, "realm {realm} has no bits a save can carry");
        }
    }
    let k = s.kingdom_with_tables(1, Tables::DEFAULT);
    assert_eq!(k.campaign.explored, s.explored);
}

/// **A new game shows each realm its start county and the border round it, and
/// nothing else** — `Map_InitScenario` cleared the plane and
/// `Game_SetupRealmsAndCounties` revealed one county. The England fixture above
/// is the same call made by the original, and it came out exactly this shape.
///
/// Ablation: the reveal loop in `Scenario::from_map_world` — every realm's
/// count is 0.
#[test]
fn a_new_game_shows_each_realm_its_start_county_with_its_border_and_nothing_else() {
    let Some(bytes) = l2_testkit::read_install("L2_maps.dat") else {
        l2_testkit::skip!("no L2_maps.dat in the install");
    };
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(0).expect("England is slot 0");
    let setup = NewGame { slot: 0, lords: 5, local_player: 1, ..NewGame::default() };
    let s = Scenario::from_map(&slot, &setup).expect("England builds");

    for realm in 1..=5u8 {
        let starts: Vec<u8> = s
            .county_ids()
            .filter(|&id| s.counties[id].as_ref().is_some_and(|c| c.owner == realm))
            .map(|id| id as u8)
            .collect();
        assert_eq!(starts.len(), 1, "realm {realm} starts with one county");
        let mut want = Explored::new();
        want.reveal_county(realm, &s.map, starts[0]);
        assert!(want.count(realm) > 0);
        for t in 0..MAP_TILES {
            assert_eq!(
                s.explored.is_seen(realm, t),
                want.is_seen(realm, t),
                "realm {realm}, tile {:?}",
                coords(t)
            );
        }
    }
    assert!(s.explored.count(1) < MAP_TILES / 2, "the rest of England is dark");
}
