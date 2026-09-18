//! **The two destination helpers `BattleUnit_Order` (`0x00479E90`) runs before
//! it commits a missile unit's move**: `Order_StopShortOfTarget`
//! (`0x00497437`) and `Dest_FindReachableNear` (`0x0048A7D9`). C231 left both
//! aside with the fire arrow; ledger row `siege-leftovers`.

#![allow(unused_imports)]
use super::*;
use l2_sim::runner::stop_short_of_target;
use l2_sim::terrain::DIM;

/// `Order_StopShortOfTarget` (`0x00497437`) on its own, against the arithmetic
/// of the 36 lines: `range - 3` kept on each axis, independently, and **an axis
/// already inside range keeps the unit's own coordinate** because the function
/// seeds its answer from `(x, y)` and not from the cell clicked.
#[test]
fn a_bowman_halts_twelve_cells_short_on_each_axis_by_itself() {
    assert_eq!(
        stop_short_of_target((10, 10), (10, 40), 15),
        (10, 28),
        "target below"
    );
    assert_eq!(
        stop_short_of_target((10, 40), (10, 10), 15),
        (10, 22),
        "target above"
    );
    assert_eq!(
        stop_short_of_target((10, 10), (40, 10), 15),
        (28, 10),
        "target right"
    );
    assert_eq!(
        stop_short_of_target((10, 10), (30, 14), 15),
        (18, 10),
        "one axis only"
    );
    assert_eq!(
        stop_short_of_target((10, 10), (10, 40), 8),
        (10, 35),
        "crossbow"
    );
    assert_eq!(
        stop_short_of_target((10, 10), (10, 40), 20),
        (10, 23),
        "catapult"
    );
    assert_eq!(
        stop_short_of_target((10, 10), (14, 16), 15),
        (10, 10),
        "no move"
    );
}

#[test]
fn the_ladder_pulls_an_ordered_missile_unit_back_to_its_range() {
    let mut r = siege_battle(0, 0x5E1_6Eu64);
    let unit = r
        .units
        .live()
        .find(|&u| {
            let u = r.units.get(u);
            u.side == SIDE_A && u.category == 1
        })
        .expect("the besieger's archers are a live category-1 unit of side 0");
    let (ux, uy) = {
        let u = r.units.get(unit);
        (u.x as i32, u.y as i32)
    };
    let click = (
        if ux > 40 { 5u8 } else { 74 },
        if uy > 40 { 5u8 } else { 74 },
    );
    r.order_full(
        unit,
        click.0,
        click.1,
        None,
        true,
        l2_sim::runner::Formation::Keep,
    );

    let range = 15; // archers, `g_missileStats[1][0] >> 3`
    let want = stop_short_of_target((ux, uy), (click.0 as i32, click.1 as i32), range);
    let got = {
        let u = r.units.get(unit);
        (u.target_x as i32, u.target_y as i32)
    };
    assert_eq!(got, want, "unit at ({ux}, {uy}) clicked at {click:?}");
    assert_ne!(
        got,
        (click.0 as i32, click.1 as i32),
        "the click itself is out of range"
    );
    assert_eq!(
        r.units.get(unit).target_cell,
        l2_sim::fire::cell_byte_offset(click.0 as i32, click.1 as i32),
        "`targetCell` is the cell clicked, not the cell walked to"
    );
}

/// `Dest_FindReachableNear` (`0x0048A7D9`) — squares of radius 0…19 around the
/// destination for an interior cell `Formation_SlotIsUsable` (`0x0048A672`)
/// accepts against the **source** cell's surface and elevation.
#[test]
fn the_wall_walk_reaches_only_the_wall_walk() {
    let r = siege_battle(4, 0x5E1_6Fu64);
    let walk = (0..DIM * DIM)
        .find(|&c| r.field.cells[c].surface == SURFACE_RAMPART_WALK)
        .expect("a level-4 castle has a rampart walk");
    let walk = ((walk % DIM) as i32, (walk / DIM) as i32);
    let unit = r.units.live().next().expect("a live unit");

    assert!(
        r.dest_find_reachable_near(walk, walk, unit),
        "the walk is its own ground"
    );

    let far = (0..DIM * DIM)
        .map(|c| ((c % DIM) as i32, (c / DIM) as i32))
        .find(|&(x, y)| {
            (1..DIM as i32 - 1).contains(&x)
                && (1..DIM as i32 - 1).contains(&y)
                && !(0..DIM * DIM).any(|c| {
                    r.field.cells[c].surface == SURFACE_RAMPART_WALK
                        && ((c % DIM) as i32 - x).abs() <= 19
                        && ((c / DIM) as i32 - y).abs() <= 19
                })
        })
        .expect("the castle does not fill the field");
    assert!(
        !r.dest_find_reachable_near(walk, far, unit),
        "no surface 4 within 19 of {far:?}"
    );
}

/// `Formation_SlotIsUsable` (`0x0048A672`) first branch: `uVar2 == 0 && side ==
/// 0 && g_battleIsSiege != 0 && surface[cell] < 4`. Ablate that `if` and the
/// garrison line below turns green with the besieger's answer.
#[test]
fn the_garrison_is_refused_an_empty_cell_of_surface_under_four() {
    let r = siege_battle(4, 0x5E1_6Fu64);
    let taken: Vec<(u8, u8)> = r.fighters.iter().map(|f| (f.x, f.y)).collect();
    let (x, y) = (0..DIM * DIM)
        .map(|c| ((c % DIM) as i32, (c / DIM) as i32))
        .find(|&(x, y)| {
            (1..DIM as i32 - 1).contains(&x)
                && (1..DIM as i32 - 1).contains(&y)
                && r.field.at(x as usize, y as usize).surface == 1
                && !r.field.at(x as usize, y as usize).impassable()
                && !taken.contains(&(x as u8, y as u8))
        })
        .expect("a besieged field has empty open ground");
    let elevation = r.field.at(x as usize, y as usize).elevation;
    let side_of = |s| r.units.live().find(|&u| r.units.get(u).side == s).expect("a live unit");

    assert!(
        !r.slot_is_usable(side_of(l2_sim::SIDE_A), x, y, 1, elevation),
        "the garrison is side 0 and the cell is surface 1"
    );
    assert!(
        r.slot_is_usable(side_of(l2_sim::SIDE_B), x, y, 1, elevation),
        "the besieger is side 4, so the branch does not fire"
    );
}

/// `Formation_FindNearbySlot` (`0x0048A38E`, `00480000.c:4077-4081`): a
/// destination of surface 2 is handed straight back, ahead of the catapult arm
/// at `:4082` and ahead of the ring search. Correction `C236`, against
/// C81's "a dry slot beside it".
///
/// Ablation: delete the `dest.surface == SURFACE_WATER` return in
/// `find_nearby_slot` and the answer is a dry cell — `Formation_SlotIsUsable`
/// rejects the moat, which is empty and impassable — so `got != moat` here.
#[test]
fn a_unit_ordered_onto_the_moat_is_given_the_moat_cell_itself() {
    let r = siege_battle(4, 0x5E1_70u64);
    let moat = (0..DIM * DIM)
        .find(|&c| r.field.cells[c].surface == l2_sim::siege::SURFACE_WATER)
        .expect("a level-4 castle has a moat");
    let moat = ((moat % DIM) as i16, (moat / DIM) as i16);
    assert!(
        r.field.at(moat.0 as usize, moat.1 as usize).impassable(),
        "the cell the search would have refused"
    );
    let unit = r
        .units
        .live()
        .find(|&u| r.units.get(u).side == SIDE_B)
        .expect("a besieging unit");

    assert_eq!(
        r.find_nearby_slot(unit, moat, &[]),
        Some((moat.0 as i32, moat.1 as i32)),
        "surface 2 short-circuits"
    );
    // `Formation_AssignSearchedSlots` (`0x00489798`, `:3758`) never reaches the
    // claim list on this path, so the second figure is given the same cell.
    assert_eq!(
        r.find_nearby_slot(unit, moat, &[(moat.0 as i32, moat.1 as i32)]),
        Some((moat.0 as i32, moat.1 as i32)),
        "the claim list does not divert it"
    );
}

/// `Formation_FindNearbySlot` (`0x0048A38E`, `00480000.c:4112-4113`): the ring
/// search's own guard, `0 < x < 0x4f && 0 < y < 0x4f`, ahead of the
/// `Formation_SlotIsUsable` call in the same `if`.
///
/// Ablation: delete the `(1..0x4f)` guard in `find_nearby_slot`'s inner loop and
/// the corner target answers `(0, 0)`, which is on the border ring.
#[test]
fn the_ring_search_never_answers_with_a_border_cell() {
    let r = siege_battle(4, 0x5E1_71u64);
    let unit = r
        .units
        .live()
        .find(|&u| r.units.get(u).side == SIDE_B)
        .expect("a besieging unit");
    for corner in [(0i16, 0i16), (79, 0), (0, 79), (79, 79)] {
        let got = r.find_nearby_slot(unit, corner, &[]).expect("open ground near every corner");
        assert!(
            (1..0x4f).contains(&got.0) && (1..0x4f).contains(&got.1),
            "target {corner:?} answered {got:?}"
        );
    }
}
