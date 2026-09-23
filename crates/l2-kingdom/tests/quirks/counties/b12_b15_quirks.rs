#![allow(unused_imports)]
use super::*;
use super::b11a_trade::*;
use super::b16_b17_quirks::*;
use super::*;
use super::economy::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

/// B12 retracted: `Industry_ToggleFromMap` `0x0043D309` calls the guarded share
/// setter with the old switch, flips `+0x1B0`, then calls it with the new one.
#[test]
fn b12_retracted_the_castle_share_follows_the_switch() {
    use l2_kingdom::industry::MapToggle;
    use l2_kingdom::tables::JOB_CASTLE_BUILDING as CASTLE;
    let mut c = County::new();
    c.population = 400;
    let mut shares = Vec::new();
    for _ in 0..4 {
        let on = l2_kingdom::industry::toggle_from_map(&mut c, MapToggle::Castle);
        shares.push((on, c.labour_share[CASTLE] > 0));
    }
    // Ablate: drop the second setter call and ON leaves the share at 0.
    assert_eq!(shares, [(true, true), (false, false), (true, true), (false, false)]);

    // Out of step (switch off, share held): the first call clears the stale
    // share, the second deals a fresh one. Ablate the first and it stays 77.
    let mut c = County::new();
    c.population = 400;
    c.labour_share = [33, 50, 17, 77, 0, 0, 23, 0];
    let mut fresh = County::new();
    fresh.population = 400;
    fresh.labour_share = [33, 50, 17, 0, 0, 0, 100, 0];
    l2_kingdom::industry::toggle_from_map(&mut c, MapToggle::Castle);
    l2_kingdom::industry::toggle_from_map(&mut fresh, MapToggle::Castle);
    assert_eq!(c.labour_share[CASTLE], fresh.labour_share[CASTLE]);
    assert_ne!(c.labour_share[CASTLE], 77);
}

#[test]
fn b15_the_inflow_list_holds_one_repeated_value_or_a_list() {
    let (faithful, fixed) = pair(Quirk::InflowListHasNoBreak);

    let sources = |quirks: Quirks| {
        let mut k = furnished_kingdom(15);
        k.options.quirks = quirks;
        k.counties[1].happiness = 100;
        for id in 2..=3 {
            k.counties[id].happiness = 0;
            k.counties[id].population = 500;
            k.counties[id].add_neighbour(1);
            k.counties[1].add_neighbour(id as u8);
        }
        l2_kingdom::population::migrate_all(&mut k.counties, k.county_count, quirks);
        (1..=k.county_count)
            .map(|id| k.counties[id].inflow_sources)
            .collect::<Vec<_>>()
    };

    let a = sources(faithful);
    let b = sources(fixed);
    let filled = |list: &[u8; l2_kingdom::county::MAX_INFLOW_SOURCES]| {
        list.iter().filter(|v| **v != 0).count()
    };

    assert!(
        a.iter().any(|l| filled(l) > 1 && l.iter().filter(|v| **v != 0).all(|v| *v == l[0])),
        "reproduced: a destination's sixteen bytes hold one repeated source"
    );
    assert!(
        b.iter().all(|l| filled(l) <= 2),
        "fixed: one slot per arriving county, not sixteen"
    );
    assert_ne!(a, b);
}


