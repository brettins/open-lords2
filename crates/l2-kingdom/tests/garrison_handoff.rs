//! | the original | what it does |
//! |---|---|
//! | `County_ChangeOwner` `0x004A72FE` | owner, happiness, shield, peak — **and no garrison statement of its own** |
//! | `County_MakeIndependent` `0x004AC3C6` | its tail: `if (county.garrisonUnit) FUN_00437535(garrisonUnit, county)` |
//! | `FUN_00437535` `0x00437535` | the hand-off: a free tile, both links cleared, or `Army_Destroy` |
//!
//! **Who keeps the men.** `FUN_00437535` never writes the unit's owner byte, so
//! the garrison stays the *loser's* army — it is only turned out of the castle.
//!
//! The taker gets an empty castle: county `+0x1BC` is cleared by
//! `FUN_00437535`, not by either caller. `[V]` from all three bodies.

use l2_kingdom::conquest::{change_owner, Capture, LeftCastle};
use l2_kingdom::county::County;
use l2_kingdom::explore::Explored;
use l2_kingdom::map::{flags, index, terrain, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::Kingdom;

const CASTLE: (u8, u8) = (20, 20);

fn kingdom(counties: usize) -> Kingdom {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(counties));
    for i in 0..MAP_TILES {
        k.campaign.map.county[i] = 1;
    }
    let i = index(CASTLE.0, CASTLE.1);
    k.campaign.map.flags[i] |= flags::SETTLEMENT;
    k.campaign.map.terrain[i] = terrain::CASTLE_PLOT + 1;
    let c: &mut County = &mut k.counties[1];
    c.owner = 1;
    c.castle_type = 1;
    c.anchor_x = 20;
    c.anchor_y = 24;
    c.population = 2_000;
    c.happiness = 100;
    for realm in 1..=2usize {
        k.realms[realm].in_play = true;
    }
    k
}

fn garrison(k: &mut Kingdom, owner: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, CASTLE.0, CASTLE.1);
    u.men = 150;
    u.troops[TroopType::Peasant.index()] = 150;
    u.county = 1;
    u.home_county = 1;
    let g = k.campaign.units.spawn(u).expect("a free slot");
    k.campaign.units.get_mut(g).expect("the garrison").garrison_county = 1;
    k.counties[1].garrison_unit = g;
    g
}

fn capture(k: &mut Kingdom, taker: u8) -> Capture {
    let restore = k.restore();
    let mut explored = Explored::new();
    let Kingdom { tables, counties, realms, campaign, .. } = k;
    let map = campaign.map.clone();
    change_owner(
        tables,
        counties,
        realms,
        &mut campaign.units,
        taker,
        1,
        0,
        &map,
        &mut explored,
        restore,
    )
}

/// **The capture that the ledger says was not ported.** `County_ChangeOwner`
/// writes the owner; the garrison is handed to `FUN_00437535` (`0x00437535`),
/// which turns it out onto a free tile with its men and its flag intact.
///
/// **The ablation is the three assertions after the owner check**: clear only
/// county `+0x1BC` — the state before this port — and the unit is still sitting
/// on the castle tile with `garrison_county == 1`, a garrison of a castle it
/// does not hold.
#[test]
fn a_captured_castles_garrison_marches_out_under_its_old_flag() {
    let mut k = kingdom(2);
    let g = garrison(&mut k, 1);

    let taken = capture(&mut k, 2);

    assert!(taken.governable, "realm 2 held nothing, so it may hold anything");
    assert_eq!(k.counties[1].owner, 2, "the county changed hands");
    assert_eq!(k.counties[1].garrison_unit, 0, "the taker gets an empty castle");
    let u = k.campaign.units.get(g).expect("the men are not destroyed");
    assert_eq!(u.owner, 1, "FUN_00437535 writes no owner byte: the loser keeps the men");
    assert_eq!(u.garrison_county, 0, "unit +0x198 cleared");
    assert_ne!(u.tile(), CASTLE, "off the castle tile");
    assert_eq!(u.men, 150, "and out with all of them");
    let Some(LeftCastle::Marched { tile, sortie }) = taken.garrison else {
        panic!("she marched: {:?}", taken.garrison)
    };
    assert_eq!(tile, u.tile());
    assert_eq!(sortie, None, "nobody was besieging");
}

/// **A besieged castle taken over the garrison's head.** `FUN_00437535` hands
/// `besiegedBy` to `Battle_BeginFromCampaign` and leaves the link standing, so
/// the sortie travels out on the [`l2_kingdom::conquest::Capture`] — this crate
/// cannot fight it.
#[test]
fn a_captured_garrison_carries_its_besieger_out() {
    let mut k = kingdom(2);
    let g = garrison(&mut k, 1);
    let mut b = Unit::new(UnitKind::Army, 2, 24, 20);
    b.men = 200;
    b.troops[TroopType::Peasant.index()] = 200;
    let besieger = k.campaign.units.spawn(b).expect("a free slot");
    k.campaign.units.get_mut(g).expect("the garrison").besieged_by = besieger as u8;
    k.campaign.units.get_mut(besieger).expect("the besieger").besieging_county = 1;

    let taken = capture(&mut k, 2);

    assert!(
        matches!(taken.garrison, Some(LeftCastle::Marched { sortie: Some(s), .. }) if s == besieger),
        "the sortie names the besieger: {:?}",
        taken.garrison
    );
    assert_eq!(
        k.campaign.units.get(g).expect("the garrison").besieged_by as usize,
        besieger,
        "the siege link survives the step out"
    );
}

#[test]
fn a_captured_garrison_with_nowhere_to_stand_is_lost() {
    let mut k = kingdom(2);
    let g = garrison(&mut k, 1);
    for i in 0..MAP_TILES {
        k.campaign.map.county[i] = 0;
        k.campaign.map.flags[i] |= flags::NO_COUNTY;
    }

    let taken = capture(&mut k, 2);

    assert_eq!(taken.garrison, Some(LeftCastle::Destroyed));
    assert!(k.campaign.units.get(g).is_none(), "Army_Destroy");
    assert_eq!(k.counties[1].garrison_unit, 0);
    assert_eq!(k.counties[1].owner, 2);
}

/// **The taker's own garrison is not evicted.** The original reaches this
/// branch with a *foreign* garrison only from `Battle_ReturnToCampaign`, which
/// has already destroyed the loser; a garrison that already belongs to the
/// taker is nobody's hand-off, and `FUN_00437535` is not called on it. `[I]`,
/// with the guard `County_ChangeOwner` itself does not have — see
/// `change_owner`.
#[test]
fn the_takers_own_garrison_keeps_its_castle() {
    let mut k = kingdom(2);
    let g = garrison(&mut k, 2);

    let taken = capture(&mut k, 2);

    assert_eq!(taken.garrison, None, "no hand-off");
    assert_eq!(k.counties[1].garrison_unit, g, "county +0x1BC stands");
    let u = k.campaign.units.get(g).expect("in the castle still");
    assert_eq!(u.garrison_county, 1);
    assert_eq!(u.tile(), CASTLE);
}

/// **A county too far to govern hands its garrison off through the other
/// door** — `County_ChangeOwner`'s `else` branch is `County_MakeIndependent`
/// (`0x004AC3C6`), whose last statement is the hand-off. The county ends up
/// nobody's, so the castle is nobody's, so the garrison is out.
#[test]
fn a_county_too_far_to_govern_hands_its_garrison_off_too() {
    let mut k = kingdom(3);
    k.counties[2].owner = 2;
    k.counties[2].population = 1_000;
    let g = garrison(&mut k, 1);

    let taken = capture(&mut k, 2);

    assert!(!taken.governable, "county 1 borders nothing of realm 2's");
    assert_eq!(k.counties[1].owner, 0, "County_MakeIndependent");
    assert_eq!(k.counties[1].garrison_unit, 0);
    let u = k.campaign.units.get(g).expect("the men are not destroyed");
    assert_eq!(u.owner, 1, "still the loser's");
    assert_eq!(u.garrison_county, 0);
    assert_ne!(u.tile(), CASTLE);
    assert!(matches!(taken.garrison, Some(LeftCastle::Marched { .. })));
}

#[test]
fn an_empty_castle_hands_nothing_off() {
    let mut k = kingdom(2);
    let taken = capture(&mut k, 2);
    assert_eq!(taken.garrison, None);
    assert_eq!(k.counties[1].owner, 2);
}
