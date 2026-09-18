#![allow(unused_imports)]
use super::*;
use super::turn_execution::*;
use super::save_loading::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

#[test]
fn the_england_fixture_holds_five_owned_counties_one_for_each_realm() {
    let game = game!();

    assert_eq!(game.kingdom.county_count, 14);
    let owners: Vec<(usize, u8)> = game
        .kingdom
        .county_ids()
        .map(|id| (id, game.kingdom.counties[id].owner))
        .filter(|(_, owner)| *owner != 0)
        .collect();
    assert_eq!(
        owners.iter().map(|&(id, _)| id).collect::<Vec<usize>>(),
        l2_testkit::ENGLAND_TURN1_COUNTIES
    );
    let mut realms: Vec<u8> = owners.iter().map(|&(_, r)| r).collect();
    realms.sort_unstable();
    assert_eq!(realms, [1, 2, 3, 4, 5], "one county each, in some order");

    assert_eq!(game.player, 1, "g_localPlayer");
    assert_eq!(game.owned_by(game.player), 1, "the human realm owns one county");
    assert_eq!(game.owned_by(0), 9, "and nine counties are unclaimed");
    let selected = game.selected as usize;
    assert_eq!(
        game.kingdom.counties[selected].owner, game.player as u8,
        "the game opens on the player's one county, wherever it is"
    );
    assert!(l2_testkit::ENGLAND_TURN1_COUNTIES.contains(&selected));
}

#[test]
fn the_clock_and_the_options_are_read_out_of_the_save() {
    let game = game!();
    let k = &game.kingdom;

    assert_eq!((k.season, k.season_next, k.year, k.turn_count), (4, 1, 1268, 1));
    assert_eq!(k.turn.phase, l2_kingdom::phase::Phase::NeutralCounties, "g_turnPhase = 1");
    assert_eq!(k.options.difficulty, 0);
    assert!(!k.options.advanced_farming);
    assert!(!k.options.armies_eat);
    assert_eq!(game.map_slot, 0, "g_scenarioIndex 0 is England, slot 0");

    for id in k.county_ids() {
        assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

#[test]
fn the_happiness_and_health_the_save_stores_are_the_ones_the_rules_predict() {
    let game = game!();
    let k = &game.kingdom;

    for id in k.county_ids() {
        let c = &k.counties[id];
        assert_eq!(c.happiness_last, 65, "county {id}");
        assert_eq!((c.shown_tax, c.shown_ration, c.shown_health), (5, 1, 1), "county {id}");
        assert_eq!((c.health_meter, c.health_band), (67, 3), "county {id}");
        if c.owner == 0 {
            assert_eq!((c.happiness, c.shown_events), (77, 5), "unowned county {id}");
            assert_eq!((c.population, c.births, c.deaths), (456, 84, 45), "county {id}");
        } else {
            assert_eq!((c.happiness, c.shown_events), (72, 0), "owned county {id}");
            assert_eq!((c.population, c.births, c.deaths), (435, 63, 45), "county {id}");
        }
        assert_eq!(c.pop_last, 417, "county {id}");
    }
}

#[test]
fn the_five_realms_are_in_play_with_a_thousand_crowns_each() {
    let game = game!();

    let lords: Vec<u8> = (1..=5).map(|r| game.kingdom.realms[r].lord).collect();
    assert_eq!(lords[0], 0, "row 0 of every lord-indexed table is the person's");
    let mut sorted = lords.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4], "five distinct lords, one apiece");
    for r in 1..=5usize {
        assert!(game.kingdom.realms[r].in_play, "realm {r}");
        assert_eq!(game.kingdom.realms[r].gold, 1000, "realm {r}");
        assert_eq!(game.kingdom.realms[r].county_count, 1, "realm {r}");
    }
    assert!(game.kingdom.realms[1].is_human);
    for r in 2..=5usize {
        assert!(!game.kingdom.realms[r].is_human, "realm {r} is an AI");
    }
}

/// The adjacency a game runs on is the list the save stores at county `+0x5C`.
#[test]
fn adjacency_derived_from_the_map_matches_the_list_stored_in_the_save() {
    let dir = install!();
    let assets = Assets::load(&platform(&dir).vfs).expect("assets load");
    let game = game!();

    let stored: Vec<Vec<u8>> = game
        .kingdom
        .county_ids()
        .map(|id| game.kingdom.counties[id].neighbours().to_vec())
        .collect();
    let counts: Vec<usize> = stored.iter().map(|n| n.len()).collect();
    assert_eq!(counts, vec![1, 3, 4, 3, 3, 6, 5, 3, 3, 7, 5, 4, 4, 3], "the save's own counts");

    let slot = assets.slot(game.map_slot).expect("slot 0");
    let derived = scenario::adjacency_from_map(&slot, game.kingdom.county_count);
    for id in game.kingdom.county_ids() {
        let mut mine = stored[id - 1].clone();
        mine.sort_unstable();
        assert_eq!(derived[id], mine, "county {id}");
    }
    assert_eq!(derived[2], vec![1, 3, 7], "and the lists are ids, not just counts");

    assert_eq!(stored[1], vec![3, 7, 1], "county 2's neighbours, in the file's own order");

    for id in game.kingdom.county_ids() {
        for &n in &stored[id - 1] {
            assert!(
                stored[n as usize - 1].contains(&(id as u8)),
                "county {id} lists {n} but not the other way round"
            );
        }
    }
}

/// **This test cannot, on its own, distinguish the two readings**, and saying
/// so is the point: the shipped index is 0, and `0 >> 2` is also 0. An earlier
/// revision shifted it right by two on the belief that the low bits named a
/// season variant of the tile set. What settles it is elsewhere —
/// `Map_LoadLattice(slot)` seeks `slot * 0x80C1`, the whole slot stride, and
/// `L2.eng` group 101's sixty strings name slots 0..=59 one for one
/// (`docs/screens.md` §3.1). The identity is asserted here so that a
/// re-introduced shift fails the moment anyone loads a save from any map but
/// the first.
#[test]
fn the_map_slot_the_save_names_is_the_fourteen_county_england() {
    let dir = install!();
    let assets = Assets::load(&platform(&dir).vfs).expect("assets load");
    let save = l2_testkit::england!();
    let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let index = save.globals().expect("the globals block").scenario_index;
    assert_eq!(index, 0, "the England fixture is slot 0");
    assert_eq!(game.map_slot as i32, index, "the slot is the index, not the index >> 2");

    let slot = assets.slot(game.map_slot).expect("slot 0");
    assert!(!slot.is_empty());
    assert_eq!(slot.county_count(), game.kingdom.county_count);
}

/// The realm colour byte at `+0x0A`, which picks a realm's banner in the menu
/// bar and its ramp on the minimap.
///
/// `FUN_004171EE` clamps it to 1..=5 before using it as a frame index, and the
/// clamp matters: `Misc_cty` frame `0x55 + 0` is 13 x 37 and would overflow the
/// 24-pixel menu bar, while 1..=5 land on the five 13 x 16 frames that fit.
#[test]
fn every_realm_in_the_save_flies_a_colour_the_banner_frames_have() {
    let game = game!();
    for id in 1..game.kingdom.realms.len() {
        let c = game.realm_colour[id];
        assert!((1..=5).contains(&c), "realm {id} flies colour {c}");
    }
    assert_eq!(game.realm_colour[0], 0);
    let mut seen = game.realm_colour[1..].to_vec();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 5, "the five realms fly five different colours");
}

