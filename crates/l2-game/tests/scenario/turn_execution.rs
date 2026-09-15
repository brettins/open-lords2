#![allow(unused_imports)]
use super::*;
use super::england_fixture::*;
use super::save_loading::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

#[test]
fn the_imported_map_is_real_terrain_and_survives_a_played_turn() {
    let mut game = game!();

    let counted = |g: &l2_game::Game| {
        let m = &g.kingdom.campaign.map;
        let land = m.county.iter().filter(|&&c| c != 0).count();
        let roads = (0..l2_kingdom::MAP_TILES)
            .filter(|&i| m.flags[i] & l2_kingdom::map::flags::ROAD != 0)
            .count();
        (land, roads)
    };

    let (land, roads) = counted(&game);
    assert!(land > 1000, "{land} tiles belong to a county — the map is not blank");
    assert!(roads > 0, "{roads} road tiles — a merchant has somewhere to walk");

    for id in 1..=game.kingdom.county_count {
        let c = &game.kingdom.counties[id];
        assert_eq!(
            game.kingdom.campaign.map.county_at(c.anchor_x, c.anchor_y) as usize,
            id,
            "county {id}'s anchor is not in county {id}"
        );
    }

    l2_game::turn::end_turn(&mut game).expect("the turn comes round");

    let (land_after, roads_after) = counted(&game);
    assert_eq!(land_after, land, "the county plane survived the season");
    assert_eq!(roads_after, roads, "and so did the roads");
}

#[test]
fn a_turn_on_the_shipped_position_settles_every_phase() {
    let mut game = game!();
    let outcome = l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    assert!(outcome.ticks < l2_game::turn::MAX_TICKS, "{} ticks", outcome.ticks);
    assert_eq!(game.kingdom.turn.phase, l2_kingdom::Phase::NeutralCounties);
    assert!(outcome.pending_battles.is_empty(), "nobody is at war on turn one");
}

#[test]
fn the_six_shipped_merchants_take_the_next_county_on_their_own_route() {
    let mut game = game!();
    let routes = game.kingdom.campaign.routes.clone();
    let before: Vec<(usize, (u8, u8))> =
        game.kingdom.campaign.units.iter().map(|(id, u)| (id, u.tile())).collect();
    assert_eq!(before.len(), 6, "six merchants stand on the board before the turn");

    l2_game::turn::end_turn(&mut game).expect("the machine comes round");

    let mut walked = 0;
    for (slot, was) in before {
        let u = game.kingdom.campaign.units.get(slot).expect("merchants do not die");
        assert_eq!(u.name_index as usize, slot - 1, "merchant {slot} is on the wrong route");
        let expected = routes.row(slot - 1)[1];
        assert_eq!(
            u.dest_county, expected,
            "merchant {slot} was sent to county {} and its route says {expected}",
            u.dest_county
        );
        assert_eq!(u.move_allowance, 10, "Merchant_Tick writes the allowance every tick");
        if u.tile() != was {
            walked += 1;
        }
    }
    assert_eq!(walked, 6, "every merchant moved off the tile it started on");
}

#[test]
fn ten_turns_of_merchants_never_leave_their_own_routes() {
    let mut game = game!();
    let routes = game.kingdom.campaign.routes.clone();
    for turn in 1..=10 {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
        let merchants: Vec<usize> = game
            .kingdom
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
            .map(|(id, _)| id)
            .collect();
        assert_eq!(merchants.len(), 6, "turn {turn}: six merchants, still");
        for slot in merchants {
            let u = game.kingdom.campaign.units.get(slot).expect("just listed");
            assert!(slot <= 6, "a merchant is always one of slots 1..=6");
            let route = routes.row(slot - 1);
            assert!(
                route.contains(&u.dest_county),
                "turn {turn}: merchant {slot} is heading for county {}, and its route is {route:?}",
                u.dest_county
            );
            assert!((0..16).contains(&u.year_formed), "turn {turn}: merchant {slot} cursor");
        }
    }
    let visited: Vec<u8> = game
        .kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(_, u)| u.county)
        .collect();
    eprintln!("after ten turns the six merchants stand in counties {visited:?}");
}

#[test]
fn a_merchant_walks_its_route_over_the_frames_of_a_turn_rather_than_teleporting() {
    let mut stepped = game!();
    let mut at_once = game!();

    let watched: Vec<usize> = stepped.kingdom.campaign.units.iter().map(|(id, _)| id).collect();
    assert_eq!(watched.len(), 6, "six merchants to watch");

    let mut trail: Vec<Vec<(u8, u8)>> = vec![Vec::new(); watched.len()];
    let mut ticks = 0u32;
    let mut step = l2_game::turn::begin_turn(&mut stepped);
    loop {
        ticks += 1;
        for (i, &slot) in watched.iter().enumerate() {
            if let Some(u) = stepped.kingdom.campaign.units.get(slot) {
                trail[i].push(u.tile());
            }
        }
        match step {
            l2_game::turn::TurnStep::Done(_) => break,
            l2_game::turn::TurnStep::Running => {}
            other => panic!("nobody is at war on turn one: {other:?}"),
        }
        assert!(ticks < l2_game::turn::MAX_TICKS, "the machine never came round");
        step = l2_game::turn::tick_turn(&mut stepped);
    }

    assert!(ticks > 8, "a turn takes frames, not one call: {ticks}");

    for (i, &slot) in watched.iter().enumerate() {
        let path = &trail[i];
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let step = (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs());
            assert!(step <= 1, "merchant {slot} went from {a:?} to {b:?} in one tick");
        }
        let mut seen: Vec<(u8, u8)> = path.clone();
        seen.dedup();
        assert!(
            seen.len() > 2,
            "merchant {slot} stood on only {} tiles over {ticks} ticks: that is a teleport",
            seen.len(),
        );
    }

    let outcome = l2_game::turn::end_turn(&mut at_once).expect("the machine comes round");
    assert_eq!(outcome.ticks, ticks, "both ways take the same number of Turn_Ticks");
    for &slot in &watched {
        let a = stepped.kingdom.campaign.units.get(slot).map(|u| u.tile());
        let b = at_once.kingdom.campaign.units.get(slot).map(|u| u.tile());
        assert_eq!(a, b, "merchant {slot} ends the turn in the same place either way");
    }
    assert_eq!(stepped.gold(), at_once.gold());
    assert_eq!(stepped.kingdom.season, at_once.kingdom.season);
    assert_eq!(stepped.kingdom.turn_count, at_once.kingdom.turn_count);
    eprintln!("the turn took {ticks} frames and every merchant was watched across them");
}

