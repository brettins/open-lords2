#![allow(unused_imports)]
use super::*;
use super::starvation::*;
use super::mercenaries::*;
use super::mines::*;
use super::persistence::*;
use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};

// ---------------------------------------------------------------------------
// Conquest, and the save
// ---------------------------------------------------------------------------

/// The one thing the whole layer exists to produce, driven through the kingdom
///: an army that reaches a defenceless county's
/// castle takes it, and the realm's county count follows.
#[test]
fn an_army_that_reaches_a_castle_takes_the_county() {
    let mut k = kingdom();
    k.counties[2].population = 10; // below the defence floor
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let restore = k.restore();
    let Kingdom { campaign, counties, realms, tables, .. } = &mut k;
    let (steps, outcome) =
        conquest::march_and_fight(tables, campaign, counties, realms, id, 0, 1268, restore);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].reached_castle, Some(2));
    assert!(matches!(outcome, Some(conquest::Attack::Captured(_))), "got {outcome:?}");

    assert_eq!(k.counties[2].owner, 1);
    assert_eq!(k.realms[1].county_count, 2);
    assert_eq!(k.counties[2].happiness, 77 - 10, "a human at difficulty 0 costs 10");
    assert_eq!(k.campaign.units.get(id).unwrap().moves_used, conquest::ATTACK_MOVE_COST);
}

/// A county that can defend itself produces a battle instead, and the layer
/// hands the pair over.
#[test]
fn a_county_that_can_defend_itself_produces_a_battle_and_keeps_its_owner() {
    let mut k = kingdom();
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let restore = k.restore();
    let Kingdom { campaign, counties, realms, tables, .. } = &mut k;
    let (_, outcome) = conquest::march_and_fight(tables, campaign, counties, realms, id, 0, 1268, restore);
    let Some(conquest::Attack::Battle { attacker, defender }) = outcome else {
        panic!("expected a battle, got {outcome:?}");
    };
    assert_eq!(attacker, id);
    assert_ne!(defender, id);
    assert_eq!(k.counties[2].owner, 0, "nothing changes hands until the battle is fought");
    assert_eq!(k.campaign.units.get(defender).unwrap().county, 2);
}

