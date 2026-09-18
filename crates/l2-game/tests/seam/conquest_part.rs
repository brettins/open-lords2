#![allow(unused_imports)]
use super::*;
use super::battle::*;
use super::realm::*;
use l2_formats::save::Save;
use l2_game::engagement::{self, Answer, Resolution};
use l2_kingdom::conquest::{self, Attack};
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::unit::{TroopType, Unit, UnitKind, TROOP_TYPES};

#[test]
fn attacking_county_three_levies_the_defence_the_saved_game_holds() {
    let before_save: Save = l2_testkit::fixture!("battle-before.sav");
    let (mut k, attacker) = before(&before_save);
    let during: Save = l2_testkit::fixture!("battle-during.sav");
    let raised = unit_at(&during, DEFENCE_SLOT).expect("battle-during holds the defence");

    let population_before = k.counties[COUNTY as usize].population;
    let restore = k.restore();
    let Kingdom { counties, realms, campaign, options, tables, year, .. } = &mut k;
    let outcome = conquest::attack_county(
        tables,
        &campaign.map,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        attacker,
        COUNTY,
        options.difficulty,
        *year,
        &mut campaign.explored,
        restore,
    );

    let Attack::Battle { defender, .. } = outcome else {
        panic!("a contented neutral county fights: {outcome:?}");
    };
    let d = k.campaign.units.get(defender).expect("the levy");

    assert_eq!(d.men, raised.men, "182 men");
    assert_eq!(d.troops, raised.troops, "122 peasants and 60 archers, not 182 of anything");
    assert_eq!(d.troops[TroopType::Peasant.index()], 122);
    assert_eq!(d.troops[TroopType::Archer.index()], 60);
    assert_eq!(d.owner, raised.owner, "owner 6 — ownerless, nobody's realm");
    assert_eq!(d.owner, l2_kingdom::levy::OWNERLESS);
    assert_eq!(d.defence_mark, raised.defence_mark, "+0x167 = 1, a defence levied on the spot");
    assert_eq!(d.defence_mark, conquest::RAISED);
    assert_eq!(d.home_county, raised.home_county, "it knows the county to go home to");
    assert_eq!(
        d.move_allowance as u8, raised.move_allowance,
        "no moves at all: it stands where it was raised and fights"
    );
    assert_eq!(d.morale as u8, raised.morale, "morale is the county's happiness");

    let during_population = during.county(COUNTY as usize).unwrap().population;
    assert_eq!(
        k.counties[COUNTY as usize].population,
        during_population,
        "county 3 goes {population_before} -> {during_population}"
    );
    assert_eq!(population_before - d.men, during_population, "728 - 182 = 546");
}

