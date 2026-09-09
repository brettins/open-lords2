//! **A realm that is cut in half loses the far half.**
//! `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) through the season pipeline —
//! `docs/kingdom.md` §6.1 and [`l2_kingdom::territory`].
//!
//! The block partition itself is unit-tested inside `territory.rs`. What is
//! here is the pass: that it runs in the season, in the right place, that the
//! counties it takes end up genuinely neutral, and that the human is treated no
//! differently from an AI.
//!
//! # The anchor, said once more
//!
//! This mechanic has **no data-side oracle**. Every realm in every fixture in
//! `E:\dev\lords2-fixtures` owns exactly one county, so the invariant the pass
//! maintains holds trivially in all six saves. What holds it up is the code,
//! the two `L2.eng` strings that name it, and a **player's recollection that
//! cut-off counties do secede in play**. Corroboration, not reproduction.

use l2_kingdom::phase::Pass;
use l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
use l2_kingdom::{territory, Kingdom, Message};

/// `n` counties in a line, `1 - 2 - 3 - …`, all held by realm 1, each with
/// people in it.
fn chain(n: usize) -> Kingdom {
    let mut k = Kingdom::new(0x5ECE_5510);
    assert!(k.set_county_count(n));
    k.realms[1].strength = 3;
    k.realms[1].is_human = true;
    k.realms[2].strength = 3;
    for id in 1..=n {
        let c = &mut k.counties[id];
        c.owner = 1;
        c.population = 100 * id as i32;
        c.pop_band = c.compute_pop_band();
        c.happiness = 60;
        c.health_meter = 60;
        if id > 1 {
            c.add_neighbour(id as u8 - 1);
        }
        if id < n {
            k.counties[id].add_neighbour(id as u8 + 1);
        }
    }
    k
}

#[test]
fn the_pass_sits_between_the_unrest_counter_and_the_field_recount() {
    assert!(Pass::UnrestUpdate.order() < Pass::SecedeIsolatedCounties.order());
    assert!(Pass::SecedeIsolatedCounties.order() < Pass::CountyRecountFields.order());
    assert!(
        l2_kingdom::phase::is_in_season_advance(Pass::SecedeIsolatedCounties),
        "it really is one of Season_Advance's own calls"
    );
}

/// A realm whose counties are all joined up loses nothing, season after season.
#[test]
fn a_connected_realm_keeps_everything_it_holds() {
    let mut k = chain(5);
    for _ in 0..4 {
        let report = k.advance_season();
        assert!(
            !report.messages.iter().any(|m| matches!(
                m,
                Message::CountySeceded { .. } | Message::LandsDivide { .. }
            )),
            "nothing should secede from a realm in one piece"
        );
    }
    assert!((1..=5).all(|id| k.counties[id].owner == 1));
}

/// **The rule that changes what conquest is worth.** Take the county that
/// bridges an enemy's territory and the far half goes free — for nothing.
#[test]
fn taking_the_bridge_county_costs_the_enemy_the_far_half() {
    let mut k = chain(5);
    // Populations 100, 200, 300, 400, 500. Realm 2 takes county 3, splitting
    // realm 1 into {1, 2} worth 300 and {4, 5} worth 900.
    k.counties[3].owner = 2;

    let report = k.advance_season();
    assert_eq!(k.counties[4].owner, 1, "the more populous block is kept");
    assert_eq!(k.counties[5].owner, 1);
    assert_eq!(k.counties[1].owner, 0, "and the far half declared independence");
    assert_eq!(k.counties[2].owner, 0);

    let divide: Vec<&Message> = report
        .messages
        .iter()
        .filter(|m| matches!(m, Message::LandsDivide { .. }))
        .collect();
    assert_eq!(divide, vec![&Message::LandsDivide { realm: 1, counties: 2 }]);
}

/// One county alone raises the *other* message, and that one names it.
#[test]
fn a_single_cut_off_county_raises_the_message_that_names_it() {
    let mut k = chain(3);
    // 100, 200, 300. Realm 2 takes the middle; realm 1 keeps county 3.
    k.counties[2].owner = 2;
    let report = k.advance_season();
    assert_eq!(k.counties[1].owner, 0);
    assert_eq!(k.counties[3].owner, 1);
    assert!(report.messages.contains(&Message::CountySeceded { realm: 1, county: 1 }));
    assert_eq!(
        Message::CountySeceded { realm: 1, county: 1 }.original_id(),
        Some(0x7F),
        "L2.eng group 127"
    );
    assert_eq!(Message::LandsDivide { realm: 1, counties: 2 }.original_id(), Some(0x80));
}

/// **The human is not treated differently.** Only the *message* is filtered by
/// `g_localPlayer`; the secession itself runs over realms 1 … 5 alike.
#[test]
fn an_ai_realm_is_split_on_exactly_the_same_rule_as_the_player() {
    let split = |human: bool| {
        let mut k = chain(3);
        k.realms[1].is_human = human;
        k.counties[2].owner = 2;
        k.advance_season();
        (1..=3).map(|id| k.counties[id].owner).collect::<Vec<u8>>()
    };
    assert_eq!(split(true), split(false));
    assert_eq!(split(false), vec![0, 2, 1]);
}

/// A realm out of play — strength 0 — is skipped entirely, and unowned
/// counties were never in a block to begin with.
#[test]
fn an_eliminated_realm_loses_nothing_because_it_is_not_looked_at() {
    let mut k = chain(3);
    k.counties[2].owner = 2;
    k.realms[1].strength = 0;
    k.advance_season();
    assert_eq!((1..=3).map(|id| k.counties[id].owner).collect::<Vec<u8>>(), vec![1, 2, 1]);
}

/// `County_MakeIndependent` leaves the county **consistent**, not merely
/// unowned: all four industries switched off, the peasants reallocated, and the
/// tax preview rewritten. Switching the industries off is the mechanism — it is
/// what turns the four industry ceilings to zero and moves those people into
/// *Idle townsfolk*.
#[test]
fn a_seceded_county_is_left_a_consistent_neutral_one() {
    let mut k = chain(3);
    for c in k.counties.iter_mut() {
        for industry in c.industry.iter_mut() {
            industry.enabled = true;
        }
    }
    k.counties[2].owner = 2;
    k.advance_season();

    let c = &k.counties[1];
    assert_eq!(c.owner, 0);
    assert!(c.industry.iter().all(|i| !i.enabled), "every industry switched off");
    assert!(!c.castle_switch);
    assert_eq!(
        c.labour.iter().sum::<i32>(),
        c.population,
        "and the peasants were reallocated: {:?}",
        c.labour
    );
    assert!(c.labour[JOB_IDLE_TOWNSFOLK] > 0, "with nothing left to do: {:?}", c.labour);
    assert_eq!(k.realms[1].county_count, 1, "and the realm's own count was rebuilt");
}

/// The partition is over the county neighbour lists, so a realm whose counties
/// simply have no adjacency at all is *every county its own block* — and keeps
/// only the most populous.
#[test]
fn a_realm_with_no_adjacency_at_all_keeps_only_its_biggest_county() {
    let mut k = Kingdom::new(7);
    assert!(k.set_county_count(3));
    k.realms[1].strength = 3;
    for (id, pop) in [(1usize, 100), (2, 900), (3, 200)] {
        let c = &mut k.counties[id];
        c.owner = 1;
        c.population = pop;
        c.pop_band = c.compute_pop_band();
    }
    let blocks = territory::build_blocks(&k.counties, 3);
    assert_eq!(blocks.count(), 3, "no neighbour entries, no contiguity");
    k.advance_season();
    assert_eq!((1..=3).map(|id| k.counties[id].owner).collect::<Vec<u8>>(), vec![0, 1, 0]);
}
