#![allow(unused_imports)]
use super::*;
use super::stalls::*;
use super::super::*;
use crate::field::terrain;
use crate::map::CampaignMap;

#[test]
fn there_are_five_allocators_and_the_two_passes_reach_different_ones() {
    assert_eq!(FarmStyle::ALL.len(), 5);
    let mut addrs: Vec<u32> = FarmStyle::ALL.iter().map(|s| s.address()).collect();
    addrs.sort_unstable();
    addrs.dedup();
    assert_eq!(addrs.len(), 5, "five distinct allocators");

    assert_eq!(FarmStyle::for_realm(0), Some(FarmStyle::RealmArable));
    assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
    assert_eq!(FarmStyle::for_realm(9), Some(FarmStyle::RealmMixed));
    assert_eq!(FarmStyle::for_realm(2), None, "no other byte dispatches");

    assert_eq!(FarmStyle::for_neutral(0), Some(FarmStyle::NeutralArable));
    assert_eq!(FarmStyle::for_neutral(1), Some(FarmStyle::NeutralGrazing));
    assert_eq!(
        FarmStyle::for_neutral(9),
        None,
        "an ex-Knight county is left alone by the neutral pass"
    );
}

/// `FUN_004A3C67` is the **neutral** pass's style 0 and no AI realm can
/// reach it. This is the briefing error, asserted.
#[test]
fn the_neutral_arable_style_is_unreachable_from_an_ai_realm() {
    assert_eq!(FarmStyle::NeutralArable.address(), 0x004A3C67);
    for byte in 0..=255u8 {
        assert_ne!(
            FarmStyle::for_realm(byte),
            Some(FarmStyle::NeutralArable),
            "byte {byte}"
        );
    }
}

#[test]
fn the_four_lords_between_them_use_every_realm_allocator() {
    let mut seen = Vec::new();
    for lord in 1..=crate::tables::AI_PERSONALITY_COUNT as u8 {
        let byte = T.ai_farm_style(lord).expect("lord {lord} has a record");
        let style = FarmStyle::for_realm(byte).expect("and a live allocator");
        seen.push(style);
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        vec![FarmStyle::RealmArable, FarmStyle::RealmGrazing, FarmStyle::RealmMixed]
    );
    assert_eq!(T.ai_farm_style(crate::realm::LORD_HUMAN), None);
}

#[test]
fn a_lot_that_arrives_stops_the_cascade_and_one_refused_does_not() {
    let (mut c, mut cmap) = county_with(4);
    c.grain = 0;
    let mut m = ledger(true);
    run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
    assert_eq!(m.asked, vec![(400, Good::Grain)], "the first lot satisfied the floor");
    assert_eq!(c.grain, 400);

    let (mut c, mut cmap) = county_with(4);
    c.grain = 0;
    let mut m = ledger(false);
    run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
    assert_eq!(
        m.asked,
        vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain), (50, Good::Grain)],
        "every lot is offered when the purse refuses"
    );
}

