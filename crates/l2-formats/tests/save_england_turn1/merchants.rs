#![allow(unused_imports)]
use super::*;
use super::counties::*;
use super::globals::*;
use l2_formats::save::COUNTY_RECORDS;
use l2_testkit::{england, england_county_of_realm, ENGLAND_TURN1_COUNTIES};

/// **The falsifiable prediction in `docs/formats/plane4.md` §2.2, checked.**
///
/// That document simulated `Merchant_PickStartCounties` over `L2_maps.dat` and
/// predicted that England's six merchants start in counties **14, 5, 13, 11, 12
/// and 4** — marked `[I]`, "the specific county list, which was not recorded in
/// that run". It is in the save, and it is those six, in that order.
///
/// The routes come with it: row 1 is `14 → 4 → 7 → 8 → 2`, which is the scan
/// order the same document derives from castle geography
/// author wrote. Two files produced by different code — `L2_maps.dat` shipped
/// with the game, `england-turn1.sav` written by the running engine — agreeing
/// on 41 numbers.
#[test]
fn england_ships_six_merchants_on_the_six_routes_plane4_predicted() {
    let save = england!();
    assert_eq!(save.merchant_start_counties().unwrap(), [14, 5, 13, 11, 12, 4]);
    assert_eq!(save.globals().unwrap().merchant_count, 6);

    let rows = save.merchant_routes().unwrap();
    let live = |r: usize| -> Vec<u8> { rows[r].iter().copied().take_while(|&c| c != 0).collect() };
    assert_eq!(live(0), vec![14, 4, 7, 8, 2]);
    assert_eq!(live(1), vec![5, 12, 6, 3, 8, 2, 1]);
    assert_eq!(live(2), vec![14, 11, 13, 5, 10, 9, 7, 3, 1]);
    assert_eq!(live(3), vec![11, 12, 6, 10, 4, 9, 3, 1]);
    assert_eq!(live(4), vec![14, 11, 5, 13, 12, 10, 2]);
    assert_eq!(live(5), vec![13, 6, 4, 9, 7, 8]);

    // §1.1's invariant, on this map: every county is on at least one route.
    let mut seen = [false; 15];
    for r in 0..6 {
        for c in live(r) {
            seen[c as usize] = true;
        }
    }
    assert!(seen[1..=14].iter().all(|&s| s), "a county on no route");
}

/// The six units the England position holds are **six merchants and nothing
/// else** — no armies, no mobs, no transports — each owned by nobody, standing
/// in its own start county, with its route number and a route cursor of 1.
///
/// **The cursor is 1, not 0**,
/// the *second* county on its list. And every one of them carries
/// `moveAllowance = 0`: the field is written by `Merchant_Tick` and turn one has
/// not ticked them yet, so an importer that filled in 10 there would be
/// inventing a number the game had not reached.
#[test]
fn the_six_units_are_merchants_waiting_in_their_start_counties() {
    let save = england!();
    let start = save.merchant_start_counties().unwrap();
    let units: Vec<_> = save.units().unwrap().into_iter().filter(|u| u.is_live()).collect();
    assert_eq!(units.len(), 6, "the England position holds six units");

    for (n, u) in units.iter().enumerate() {
        assert_eq!(u.index, n + 1, "merchants take slots 1..6");
        assert_eq!(u.kind, 3, "unit {} is not a merchant", u.index);
        assert_eq!(u.owner, 6, "a merchant belongs to nobody");
        assert_eq!(u.county, start[n], "merchant {} is not in its start county", u.index);
        assert_eq!(u.role, start[n], "+0x167 is the start county");
        assert_eq!(u.name_index as usize, n, "the route number is the slot, zero-based");
        assert_eq!(u.route_cursor(), 1, "the cursor starts at 1");
        assert_eq!(u.morale, 100, "Merchant_SpawnAll writes 100 to +0x166");
        assert_eq!(u.move_allowance, 0, "tick-maintained, and turn one has not ticked");
        assert_eq!(u.men, 0, "a merchant is not troops");
        assert!(u.needs_destination, "and none of them has been given anywhere to go");
        assert_eq!(u.path_len, 0);
    }
}

