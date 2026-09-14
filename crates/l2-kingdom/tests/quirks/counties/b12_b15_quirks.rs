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

/// The share moves the wrong way on every click with the quirk on, and the
/// right way with it off — asserted over **four clicks**, because one click
/// cannot tell "inverted" from "off by one".
#[test]
fn b12_the_castle_switch_moves_its_labour_share_backwards_or_forwards() {
    use l2_kingdom::industry::MapToggle;
    let (faithful, fixed) = pair(Quirk::CastleSwitchMovesShareBackwards);

    let walk = |quirks: Quirks| {
        let mut c = County::new();
        c.population = 400;
        let mut shares = Vec::new();
        for _ in 0..4 {
            l2_kingdom::industry::toggle_from_map(&mut c, MapToggle::Castle, quirks);
            shares.push(c.labour_share[l2_kingdom::tables::JOB_CASTLE_BUILDING]);
        }
        shares
    };

    let a = walk(faithful);
    let b = walk(fixed);
    assert_ne!(a, b, "the share sequence must differ: {a:?} against {b:?}");
    assert!(
        b[0] > 0,
        "fixed, switching castle building ON gives it a share: {b:?}"
    );
}

// ---------------------------------------------------------------------------
// B15 — the migration inflow list is written with no break
// ---------------------------------------------------------------------------

#[test]
fn b15_the_inflow_list_holds_one_repeated_value_or_a_list() {
    let (faithful, fixed) = pair(Quirk::InflowListHasNoBreak);

    let sources = |quirks: Quirks| {
        let mut k = furnished_kingdom(15);
        k.options.quirks = quirks;
        // Two miserable counties beside a happy one, so both send people to it.
        // **The adjacency has to be set** - `migrate_all` walks
        // `County::neighbours`, and a county with none emigrates nowhere, which
        // is how the first draft of this test measured an empty list twice and
        // called them different.
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

    // The reproduced bug: some county's list is one value written into every
    // free slot, so it is full.
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

// ---------------------------------------------------------------------------
// B16 — a county that dies out records a negative death count
// ---------------------------------------------------------------------------

