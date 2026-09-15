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
