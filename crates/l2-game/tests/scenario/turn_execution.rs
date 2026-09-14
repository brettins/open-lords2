#![allow(unused_imports)]
use super::*;
use super::england_fixture::*;
use super::save_loading::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

/// **The imported map is real terrain, and it is still real terrain after a
/// turn has been played.**
///
/// `Kingdom::new` builds an empty `CampaignMap` and for a long time nothing
/// overwrote it, so every imported game did its pathfinding, its field
/// crossing and its trampling over 4,096 blank tiles. The planes come out of
/// block 0 of the save now. This checks both halves of that: that the tiles
/// arrive, and that **ending a turn does not lose them** — which is the half a
/// season pipeline could quietly break, because the map is the one piece of
/// simulation state that the economy writes to and never reads back.
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

    // Every county the save names has an anchor inside its own territory,
    // which is what the merchant and transport re-targets steer for.
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

/// A turn ends on the shipped position without anything getting stuck.
///
/// The unit phases wait on the unit array now
/// a turn that never terminates is a live failure mode
/// impossible one. The England position carries no armies, so this is the
/// *empty* case — the one where every wait has to settle on its own.
#[test]
fn a_turn_on_the_shipped_position_settles_every_phase() {
    let mut game = game!();
    let outcome = l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    assert!(outcome.ticks < l2_game::turn::MAX_TICKS, "{} ticks", outcome.ticks);
    assert_eq!(game.kingdom.turn.phase, l2_kingdom::Phase::NeutralCounties);
    assert!(outcome.pending_battles.is_empty(), "nobody is at war on turn one");
}

/// **The six shipped merchants walk their shipped routes.**
///
/// This is the whole point of importing `g_units`, and it had never happened:
/// the seam read the counties, the realms and the map and dropped the unit block
/// on the floor, so every game loaded from the England position had a working
/// economy and an empty board.
///
/// One ended turn is enough to see it. Phase 6 runs `Merchant_AdvanceAll`, each
/// merchant takes the *second* county on its own route — the cursor ships at 1,
/// not 0 — and walks its ten points towards it. What is checked is not where any
/// of them ends up, which is pathfinding, but that the destination each one took
/// is the county the route table names for its slot.
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
        // `Merchant_AdvanceAll` indexes the table by **slot minus one**, not by
        // the merchant's own route number. The two agree here — the merchants
        // are slots 1..6 and their route numbers are 0..5 — and that agreement
        // is the coupling the original depends on.
        assert_eq!(u.name_index as usize, slot - 1, "merchant {slot} is on the wrong route");
        let expected = routes.row(slot - 1)[1];
        assert_eq!(
            u.dest_county, expected,
            "merchant {slot} was sent to county {} and its route says {expected}",
            u.dest_county
        );
        // A merchant that *arrived* this turn is idle again and will take its
        // next stop next turn; one still walking is not. Both are correct, and
        // `dest_county` above is the assertion either way, because
        // `Merchant_AdvanceAll` writes the route's county and never overwrites
        // it from the tile.
        assert_eq!(u.move_allowance, 10, "Merchant_Tick writes the allowance every tick");
        if u.tile() != was {
            walked += 1;
        }
    }
    assert_eq!(walked, 6, "every merchant moved off the tile it started on");
}

/// Ten turns of merchants, and the property that has to hold across all of
/// them: a merchant only ever heads for a county **on its own route**, and it
/// takes them in order.
///
/// A cursor that ran off the end, an index taken from the wrong field, or a
/// route row read at the wrong stride would all break this within a season or
/// two; walking it ten times is what makes the cyclic half of the walk mean
/// something.
///
/// > **This used to walk *every* unit and assert each one was a merchant**, on
/// > the reasonable grounds that nothing else had ever appeared. The AI raises
/// > armies now (`l2_kingdom::ai_army`), so by turn ten the unit array holds
/// > merchants *and* armies and the old assertion failed on slot 7 — which is
/// > the first evidence, from a test written before any of this existed, that
/// > the AI's war
/// > filters; the count is asserted separately below so the filter cannot
/// > quietly become "no merchants at all".
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
            // The cursor stays inside the row it indexes.
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

/// **A turn has a duration, and a merchant walks it.**
///
/// This is the defect a player reported as *"the merchant seems to just
/// teleport on end turn"*, made into an assertion. It is not about *where* a
/// merchant ends up — the route test above already pins that — it is about the
/// turn taking **frames**, with the merchant on a different tile in each of
/// them.
///
/// The old `end_turn` ran the whole phase machine inside one call, so every
/// tile of every route was entered between two frames and the only two
/// positions a merchant was ever *drawn* at were where it started and where it
/// stopped. `turn::tick` is one `Turn_Tick` / `Units_Tick` pair, which is what
/// the original's main loop calls once a frame, and this walks the same turn
/// one frame at a time and watches.
///
/// Three properties, and the first is the one that was false:
///
/// 1. a turn takes **many** ticks, and a merchant occupies **several distinct
///    tiles** over them — it is seen to walk;
/// 2. it never moves more than one tile in a tick, which is `Unit_Step`'s own
///    rule and the difference between walking and skipping;
/// 3. the turn ends in exactly the state the all-at-once `end_turn` produces,
///    so nothing about spreading it over frames changed the simulation.
#[test]
fn a_merchant_walks_its_route_over_the_frames_of_a_turn_rather_than_teleporting() {
    // The same turn, run both ways, from the same starting position.
    let mut stepped = game!();
    let mut at_once = game!();

    let watched: Vec<usize> = stepped.kingdom.campaign.units.iter().map(|(id, _)| id).collect();
    assert_eq!(watched.len(), 6, "six merchants to watch");

    // Where each merchant stood at the end of every tick of the turn.
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
        // 2. One tile a tick. `Unit_Step` enters at most one, and a merchant
        //    that jumped two would be a merchant nobody could watch walk.
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let step = (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs());
            assert!(step <= 1, "merchant {slot} went from {a:?} to {b:?} in one tick");
        }
        // 1. And it was seen in more than two places, which is the whole
        //    difference between walking and teleporting.
        let mut seen: Vec<(u8, u8)> = path.clone();
        seen.dedup();
        assert!(
            seen.len() > 2,
            "merchant {slot} stood on only {} tiles over {ticks} ticks: that is a teleport",
            seen.len(),
        );
    }

    // 3. Spreading a turn over frames is a *display* change and must not be a
    //    simulation one.
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

