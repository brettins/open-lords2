#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::england::*;
use std::collections::BTreeMap;
use l2_game::game::Game;
use l2_kingdom::report::Message;
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};

#[test]
fn a_hundred_turns_of_an_empire_taxed_at_thirty() {
    let Some(mut game) = england_with_an_empire() else {
        l2_testkit::skip!("no England fixture");
    };
    scoreline(&game.kingdom, "empire, turn 1");
    let census = play(&mut game, TURNS, "empire");
    scoreline(&game.kingdom, "empire, turn 100");
    census.print("empire, 100 turns");

    for what in [
        "a realm holding more than one county",
        "a realm holding four or more counties",
        "a tax rate the empire term can see (>= 20)",
        "TAX_HAPPINESS_OTHER non-zero",
        "Realm::tax_hap_empire non-zero",
    ] {
        assert!(census.fired(what), "after {TURNS} turns of a six-county empire, `{what}` never happened");
    }
}

/// **All 44 shipped maps, twenty turns each.** `docs/decisions.md` C26's
/// warning applied to the map
/// forty-four, and `tests/newgame.rs` only takes *one* turn in each.
#[test]
fn every_shipped_map_survives_twenty_turns() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2_maps.dat");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = l2_game::game::Assets::load(&platform.vfs).expect("assets load");
    let bytes = l2_testkit::read_install("L2_maps.dat").expect("the install has it");
    let set = l2_formats::maps::MapSet::parse(&bytes).expect("L2_maps.dat parses");

    let mut played = 0;
    let mut worst: Vec<String> = Vec::new();
    for slot in set.used_slots() {
        if slot >= 60 {
            continue;
        }
        let seats = set.slot(slot).unwrap().player_start_count();
        let lords = seats.min(5).max(1);
        let settings =
            l2_game::setup::SetupOptions::new().commit(1, l2_kingdom::Quirks::default());
        let settings = l2_game::setup::Settings { ai_lords: lords as i32 - 1, ..settings };
        let shield = (slot % 5 + 1) as u8;
        let mut game = l2_game::scenario::new_game(
            &assets,
            slot,
            &settings,
            1,
            shield,
            l2_game::scenario::SEED,
            Tables::DEFAULT,
        )
        .unwrap_or_else(|e| panic!("slot {slot}: {e}"));
        settings.apply_to(&mut game);
        game.kingdom.start_new_game();
        let census = play(&mut game, 20, &format!("map slot {slot}"));
        let alive = (1..l2_kingdom::MAX_REALMS)
            .filter(|&r| game.kingdom.realms[r].in_play)
            .count();
        worst.push(format!(
            "slot {slot:>2}: {:>3} counties, {lords} lords, {alive} alive after 20 turns, \
             max held {}",
            game.kingdom.county_count, census.max_counties
        ));
        played += 1;
    }
    for line in &worst {
        eprintln!("  {line}");
    }
    assert_eq!(played, 44, "every shipped map should have been played");
}

#[test]
fn a_hundred_turns_is_the_same_hundred_however_it_is_reached() {
    let save = l2_testkit::england!();
    let fresh =
        || l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let digest = |game: &Game| {
        let mut c = l2_net::Canonical::hashing();
        l2_net::Encode::encode(&game.kingdom, &mut c);
        c.finish()
    };

    let mut straight = fresh();
    for _ in 0..TURNS {
        l2_game::turn::end_turn(&mut straight).expect("the machine comes round");
    }

    let mut again = fresh();
    for _ in 0..TURNS {
        l2_game::turn::end_turn(&mut again).expect("the machine comes round");
    }
    assert_eq!(
        digest(&straight).hash,
        digest(&again).hash,
        "two identical hundred-turn games diverged"
    );

    let mut halved = fresh();
    for _ in 0..TURNS / 2 {
        l2_game::turn::end_turn(&mut halved).expect("the machine comes round");
    }
    let bytes = l2_game::save::encode(&halved);
    let mut reloaded = l2_game::save::decode(&bytes, Tables::DEFAULT).expect("our own save loads");
    for _ in 0..TURNS / 2 {
        l2_game::turn::end_turn(&mut reloaded).expect("the machine comes round");
    }
    assert_eq!(
        digest(&straight).hash,
        digest(&reloaded).hash,
        "a game saved at turn {} and reloaded reached a different turn {TURNS}",
        TURNS / 2
    );
}

