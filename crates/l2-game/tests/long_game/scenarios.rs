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
    // A load leaves g_moveCost empty (B108); so does this.
    halved.kingdom.campaign.move_cost = l2_kingdom::map::CostMap::uniform(0);
    for _ in 0..TURNS / 2 {
        l2_game::turn::end_turn(&mut reloaded).expect("the machine comes round");
        l2_game::turn::end_turn(&mut halved).expect("the machine comes round");
    }
    assert_eq!(
        digest(&halved).hash,
        digest(&reloaded).hash,
        "a game saved at turn {} and reloaded reached a different turn {TURNS}",
        TURNS / 2
    );
}


/// B108: Save_Read 0x004ADFE6 leaves g_moveCost 0x004F4080 as it was.
#[test]
fn a_loaded_game_paths_on_an_empty_cost_map_until_phase_seven() {
    let empty = l2_kingdom::map::CostMap::uniform(0);
    let mut game = l2_game::scenario::from_save(&l2_testkit::england!(), Tables::DEFAULT).unwrap();
    assert_eq!(game.kingdom.campaign.move_cost, empty, "from_save runs no Move_BuildCostMap");
    let ai_armies = |g: &Game| {
        g.kingdom.campaign.units.iter()
            .filter(|(_, u)| u.kind == UnitKind::Army && u.owner != g.player)
            .map(|(id, u)| (id, u.tile())).collect::<Vec<_>>()
    };
    let mut built = game.clone();
    built.kingdom.campaign.rebuild_move_cost();
    let mut loaded = game.clone();
    l2_game::turn::end_turn_to_autosave(&mut built).unwrap();
    l2_game::turn::end_turn_to_autosave(&mut loaded).unwrap();
    assert_ne!(ai_armies(&built), ai_armies(&loaded), "Ai_AdvanceArmies 0x004A5667 paths on the stored map");

    let (army, owner) = built.kingdom.campaign.units.iter()
        .find(|(_, u)| u.kind == UnitKind::Army).map(|(id, u)| (id, u.owner)).unwrap();
    let mut ordered = built.clone();
    ordered.kingdom.campaign.move_cost = empty.clone();
    ordered.player = owner;
    let (x, y) = ordered.kingdom.campaign.units.get(army).unwrap().tile();
    ordered.order_unit_move(army, (x + 1, y));
    assert_ne!(ordered.kingdom.campaign.move_cost, empty, "Unit_OrderMove 0x004A98FE rebuilds it");

    l2_game::turn::end_turn_to_autosave(&mut game).unwrap();
    assert_eq!(game.kingdom.campaign.move_cost, game.kingdom.campaign.map.cost_map(), "Turn_Tick phase 7");
}
