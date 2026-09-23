#![allow(unused_imports)]
use super::*;
use super::thinking::*;
use super::marching::*;
use super::reform::*;
use super::super::*;
use crate::figure::{SIDE_A, SIDE_B};
use crate::unit::CATEGORY_OF_TROOP;

#[test]
fn the_search_radius_is_the_whole_field_for_missiles_and_a_few_cells_for_melee() {
    let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
    let world = World {
        units: &mut fx.units,
        figures: &mut fx.figures,
        positions: &fx.positions,
        field: &fx.field,
        ai: &mut fx.ai,
    };
    assert_eq!(world.nearest_enemy_unit(1, 80, 1), 2, "twenty cells is within eighty");
    assert_eq!(world.nearest_enemy_unit(1, 9, 3), 0, "and outside nine");
    assert_eq!(world.nearest_enemy_unit(1, 20, 1), 2, "exactly twenty is inside");
    assert_eq!(world.nearest_enemy_unit(1, 19, 1), 0);
}

#[test]
fn halfway_to_unit_does_nothing_below_eight_cells_of_separation() {
    let close = |gap: u8| {
        let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 1);
        fx.positions[0] = (40, 50);
        fx.positions[1] = (40, 50 - gap);
        fx.units.rebuild_from_figures(&mut fx.figures);
        fx.units.recentre(1, &fx.figures, &fx.positions);
        fx.units.recentre(2, &fx.figures, &fx.positions);
        fx.units.get_mut(1).target_x = 40;
        fx.units.get_mut(1).target_y = 50;
        let mut world = World {
            units: &mut fx.units,
            figures: &mut fx.figures,
            positions: &fx.positions,
            field: &fx.field,
            ai: &mut fx.ai,
        };
        world.halfway_to_unit(1, 2);
        (fx.units.get(1).target_x, fx.units.get(1).target_y)
    };
    assert_eq!(close(7), (40, 50), "seven cells: nothing happens at all");
    assert_eq!(close(8), (40, 46), "eight: halve it");
    assert_eq!(close(20), (40, 40));
}


/// `UnitOrder_SiegeAttKnight` is the only writer of `g_battleWithdrawal`
/// (`0x0056D5C8`) in the whole binary, and the clause was missing here —
/// which made [`crate::End::Withdrawal`] unreachable in a played game and
/// with it the whole withdrawal half of the campaign seam, including
/// `Army_WithdrawCasualties`, which `l2-kingdom` had never implemented
/// because nothing could reach it. `docs/decisions.md` C71.
#[test]
fn an_all_knight_besieger_at_an_unbreached_wall_leaves_the_field() {
    let raised = |breach: i32, dismount: bool| {
        let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
        fx.ai.is_siege = true;
        if dismount {
            fx.figures[0].troop = Troop::Peasants;
        }
        fx.ai.count_men(&fx.figures);
        assert!(fx.ai.men_total > 0, "the census sees the AI's own men");
        fx.ai.breach_score = breach;
        assert_eq!(fx.ai.withdrawal, None, "nothing before the unit has thought");
        fx.think_at(0);
        fx.ai.withdrawal
    };
    assert_eq!(raised(0, false), Some(SIDE_B), "all knights, nothing breached");
    assert_eq!(raised(4, false), None, "a breach is a reason to stay");
    assert_eq!(raised(0, true), None, "and so is one man who can climb");
}

#[test]
fn a_garrison_has_no_handler_for_the_three_troops_it_never_raises() {
    for category in [5usize, 6, 7] {
        assert_eq!(
            TABLE_SIEGE_DEF[category].name, "UnitOrder_None",
            "defender category {category} should be the empty handler"
        );
    }
    assert_eq!(TABLE_SIEGE_ATT[8].name, "UnitOrder_None");

    let all = || {
        TABLE_FIELD
            .iter()
            .chain(TABLE_SIEGE_ATT.iter())
            .chain(TABLE_SIEGE_DEF.iter())
    };
    assert_eq!(all().count(), 25, "twenty-five slots");
    assert_eq!(
        all().filter(|s| s.name == "UnitOrder_None").count(),
        7,
        "UnitOrder_None fills seven of them"
    );
    let mut distinct: Vec<u32> = all().map(|s| s.addr).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 18, "eighteen distinct functions");
    assert_eq!(distinct.len() - 1, 17, "seventeen of which do something");
    // The last real handler is UnitOrder_SiegeDefOil, not the helper at
    // 0x0048ECA9 that `battle.md` §11's range overshoots to.
    assert_eq!(*distinct.last().unwrap(), 0x0048_E8B8);
}

#[test]
fn a_garrison_only_sorties_when_it_believes_it_is_three_and_a_half_times_stronger() {
    let sortied = |advantage: i32| {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let def = fx.units.create(3, false, SIDE_A, 4).unwrap(); // knight
        let mut f = Figure::new(Troop::Knights, SIDE_A, 4);
        f.unit = def as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((40, 40));
        fx.think_at(advantage);
        (fx.ai.last_action[def], fx.ai.drawbridge_down)
    };
    let (held, bridge) = sortied(SORTIE_THRESHOLD);
    assert_ne!(held, Action::Charge, "260 is not above 260");
    assert!(!bridge, "and the drawbridge stayed up");
    assert_eq!(sortied(SORTIE_THRESHOLD + 1), (Action::Charge, true));
}

#[test]
fn the_first_wall_missile_unit_holds_its_slot_for_the_entire_battle() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
    fx.ai.is_siege = true;
    fx.ai.multiplayer = true;
    let u = fx.units.create(3, false, SIDE_A, 9).unwrap();
    let mut f = Figure::new(Troop::Archers, SIDE_A, 4);
    f.unit = u as u16;
    f.owner = 3;
    fx.figures.push(f);
    fx.positions.push((30, 44));
    fx.frames(2_000);
    assert_eq!(fx.units.get(u).orders, 10, "it thought ten times");
    assert_eq!(fx.ai.last_action[u], Action::DoNothing, "and never ordered anything");
    assert_eq!(
        (fx.units.get(u).target_x, fx.units.get(u).target_y),
        (30, 44),
        "its destination is still where it deployed"
    );
}

#[test]
fn a_catapult_in_a_field_battle_is_never_given_an_order() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
    fx.ai.multiplayer = true;
    let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
    let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
    f.unit = u as u16;
    f.owner = 3;
    fx.figures.push(f);
    fx.positions.push((30, 55));
    fx.frames(2_000);
    assert_eq!(fx.units.get(u).orders, 0, "it never even thinks");
    assert_eq!(fx.ai.last_action[u], Action::NoThink);
    fx.ai.is_siege = true;
    fx.frames(200);
    assert!(fx.units.get(u).orders > 0);
}

#[test]
fn the_catapult_looks_for_a_wall_within_exactly_its_firing_range() {
    let found = |wall_distance: i32| {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.field.surface = vec![0u8; CELLS];
        let (cx, cy) = (30usize, 55usize);
        fx.field.surface[(cy as i32 - wall_distance) as usize * DIM + cx] = 4;
        let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
        let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((cx as u8, cy as u8));
        for _ in 0..32 {
            fx.frames(200);
        }
        fx.ai.last_action[u]
    };
    assert_eq!(found(20), Action::ToCell, "twenty cells is in range");
    assert_eq!(found(21), Action::ToCastleApproach(5), "twenty-one is not");
}

#[test]
fn a_catapult_that_has_found_its_wall_stops_advancing_its_script() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
    fx.ai.is_siege = true;
    fx.ai.multiplayer = true;
    fx.field.surface = vec![0u8; CELLS];
    fx.field.surface[50 * DIM + 30] = 4;
    let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
    let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
    f.unit = u as u16;
    f.owner = 3;
    fx.figures.push(f);
    fx.positions.push((30, 55));
    for _ in 0..32 {
        fx.frames(200);
    }
    let pinned = fx.units.get(u).orders;
    assert_eq!(fx.ai.last_action[u], Action::ToCell);
    for _ in 0..10 {
        fx.frames(200);
    }
    assert_eq!(fx.units.get(u).orders, pinned, "the script counter is stuck");
}

