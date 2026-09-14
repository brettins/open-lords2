#![allow(unused_imports)]
use super::*;
use super::starvation::*;
use super::mines::*;
use super::conquest_part::*;
use super::persistence::*;
use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};

/// Phase 7 gives every unit its moves back, and it does so *after* the
/// mercenaries have walked.
#[test]
fn a_season_returns_every_units_movement_and_walks_the_bands() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 1, 300, 5, 5);
    k.campaign.units.get_mut(id).unwrap().moves_used = 15;
    let before: Vec<u8> = k.campaign.mercenaries.iter().map(|(_, b)| b.next_county).collect();

    let report = k.advance_season();
    assert!(report.passes.contains(&Pass::MercenaryAdvance));
    assert!(report.passes.contains(&Pass::UnitsResetMoves));
    assert_eq!(k.campaign.units.get(id).unwrap().moves_used, 0);
    assert_eq!(k.campaign.units.get(id).unwrap().moves_left(), 15);

    let after: Vec<u8> = k.campaign.mercenaries.iter().map(|(_, b)| b.next_county).collect();
    assert_ne!(before, after, "the bands moved");
}

/// A year of the mercenary walk on a two-county map: the bands stay inside the
/// map and the county offers only ever name a band in play.
#[test]
fn a_year_of_the_mercenary_walk_stays_inside_the_map() {
    let mut k = kingdom();
    assert_eq!(k.campaign.mercenaries.in_play(), 2, "two counties, two bands");
    for _ in 0..12 {
        k.advance_season();
        for (id, band) in k.campaign.mercenaries.iter() {
            assert!(band.next_county <= 3, "band {id} walked to {}", band.next_county);
            assert!(band.offered_in <= 2);
        }
        for c in 1..=2 {
            let offer = k.counties[c].mercenary_offer;
            assert!(offer as usize <= k.campaign.mercenaries.in_play(), "county {c} offers {offer}");
        }
    }
}

/// A band on offer can be hired into a new army, and the men land in `men` but
/// not in the troop counts.
#[test]
fn a_band_standing_in_a_county_can_be_hired_when_an_army_is_raised_there() {
    let mut k = kingdom();
    // Walk until something is on offer in county 1.
    let mut seasons = 0;
    while k.counties[1].mercenary_offer == 0 {
        k.advance_season();
        seasons += 1;
        assert!(seasons < 30, "no band ever reached county 1");
    }
    let band = k.counties[1].mercenary_offer;
    let rules = *k.campaign.mercenaries.rules(band).expect("a band in play");

    let id = army(&mut k, 1, 1, 100, 5, 5);
    let gold = k.realms[1].gold;
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    assert!(campaign.mercenaries.hire(&mut campaign.units, counties, realms, id, band));

    let u = k.campaign.units.get(id).unwrap();
    assert_eq!(u.men, 100 + rules.men);
    assert_eq!(u.troops.iter().sum::<i32>(), 100, "the band is not in the troop counts");
    assert_eq!(u.mercenary_men(), rules.men);
    assert_eq!(k.realms[1].gold, gold - rules.price);
    assert_eq!(k.counties[1].mercenary_offer, 0);
}

/// Bankruptcy walks the mercenaries out first, and it is the season pass that
/// does it.
#[test]
fn a_realm_that_cannot_pay_loses_its_mercenaries_before_it_loses_men() {
    let mut k = kingdom();
    k.realms[1].gold = 0;
    let id = army(&mut k, 1, 1, 400, 5, 5);
    k.campaign.mercenaries.set_band_raw(
        1,
        l2_kingdom::mercenary::Band { hired_by: id as u16, ..Default::default() },
    );
    k.campaign.units.get_mut(id).unwrap().mercenaries = Some(l2_kingdom::Mercenaries {
        band: 1,
        troop: TroopType::Pikeman,
        men: 100,
    });
    k.campaign.units.get_mut(id).unwrap().men += 100;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.realms[1].bankrupt_stage, 1);
    assert!(k.campaign.units.get(id).unwrap().mercenaries.is_none(), "they walked");
    assert_eq!(k.campaign.units.get(id).unwrap().men, 400, "and took their men with them");
    assert_eq!(k.campaign.units.get(id).unwrap().troops.iter().sum::<i32>(), 400);

    // The next unpaid season takes a tenth off the levy itself.
    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.realms[1].bankrupt_stage, 2);
    assert_eq!(k.campaign.units.get(id).unwrap().men, 360);
}

/// **The whole six-season ladder, and its last two rungs.** `Wages_PayAll`
/// (`0x004ACBD4`): stage 0 releases mercenaries, stages 1..=4 are
/// `Realm_DesertArmies` (`0x004AD0E8`) — **four** desertions, not three — and
/// stage 5 is `Realm_DestroyArmies` (`0x004AD316`), which disbands every army
/// the realm holds and wraps the counter to 0.
///
/// Ablation: key the handler on `realm.bankrupt_stage` instead of the action
/// and the last two seasons do nothing at all — the counter is the *next*
/// stage, and the mutiny has already wrapped it to 0.
#[test]
fn six_unpaid_seasons_desert_four_times_and_then_disband_every_army() {
    let mut k = kingdom();
    k.realms[1].gold = 0;
    let a = army(&mut k, 1, 1, 400, 5, 5);
    let b = army(&mut k, 1, 1, 200, 7, 5);

    let mut men = Vec::new();
    let mut ladder = Vec::new();
    for _ in 0..6 {
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::WagesPay, &mut report);
        ladder.extend(report.messages.iter().filter_map(|m| match m {
            Message::Bankrupt { action, .. } => Some(*action),
            _ => None,
        }));
        men.push(k.campaign.units.get(a).map(|u| u.men));
    }

    use l2_kingdom::industry::BankruptcyAction as A;
    assert_eq!(
        ladder,
        vec![A::Warned, A::Desertion, A::Desertion, A::Desertion, A::LastWarning, A::Mutiny]
    );
    // A tenth off every count above ten, four times: 400, 360, 324, 292, 263.
    assert_eq!(men, vec![Some(400), Some(360), Some(324), Some(292), Some(263), None]);
    assert!(k.campaign.units.get(b).is_none(), "the mutiny takes every army, not one");
    assert_eq!(k.realms[1].bankrupt_stage, 0, "and the counter wraps");
    assert_eq!(k.realms[1].wages, 0, "with nobody left to pay");
}

