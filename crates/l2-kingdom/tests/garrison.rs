//! **Into a castle and out of it again** — the two halves of a garrison's door.
//!
//! ```text
//! cargo test -p l2-kingdom --test garrison
//! ```
//!
//! | the original | what it does |
//! |---|---|
//! | `Army_SplitConfirm` `0x00437AFB` | the castle split: no minimum, capped by the room left |
//! | `Army_Split` `0x00437FD7` | the daughter is given the castle tile and a path, not five moves |
//! | `Unit_ReachCastleBuilding` `0x004686A0` | she arrives: your county, so `Army_Garrison` |
//! | `Army_GarrisonApply` `0x004A79A3` | the links, the teleport and the five moves |
//!
//! **The parent is an obstacle** with the
//! castle beyond the daughter's spawn tile: a route that crosses the tile the
//! parent is on stops dead there, `try_enter`'s `Occupied` arm. The original
//! asks *"Combine armies?"* (`L2.eng` 10/5) instead. Not this file's subject,
//! and noted because a parent standing between the daughter and the castle
//! makes the split look broken.

use l2_kingdom::conquest::{leave_castle, LeftCastle};
use l2_kingdom::county::County;
use l2_kingdom::divide::{split, SplitBasket, SplitInto};
use l2_kingdom::map::{flags, index, terrain, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::Kingdom;

const CASTLE: (u8, u8) = (20, 20);

/// One county, realm 1's, with a palisade standing at [`CASTLE`].
fn kingdom() -> Kingdom {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(2));
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
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k
}

fn army(k: &mut Kingdom, at: (u8, u8), men: i32) -> usize {
    let mut u = Unit::new(UnitKind::Army, 1, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = 1;
    u.home_county = 1;
    u.owner_is_human = true;
    k.campaign.units.spawn(u).expect("a free slot")
}

/// Walk every unit until nothing is moving.
fn until_still(k: &mut Kingdom) {
    for _ in 0..2_000 {
        if !k.units_moving(UnitKind::Army) {
            return;
        }
        k.tick_units();
    }
    panic!("the march never finished");
}

/// **Splitting an army into a castle puts the daughter in the garrison.**
///
/// `Army_Split` (`0x00437FD7`) gives the daughter the castle tile and a path
/// where the plain split charges five moves; `Unit_ReachCastleBuilding`
/// (`0x004686A0`) meets her there and, the county being her owner's, hands her
/// to `Army_GarrisonApply` (`0x004A79A3`), which is what writes county `+0x1BC`
/// and unit `+0x198`.
#[test]
fn a_split_into_a_castle_becomes_the_garrison() {
    let mut k = kingdom();
    let parent = army(&mut k, (26, 26), 300);
    let mut basket = SplitBasket::seed(k.campaign.units.get(parent).expect("the parent"));
    assert_eq!(basket.to_daughter(TroopType::Peasant, 100), 100);

    let Kingdom { tables, counties, realms, campaign, year, .. } = &mut k;
    let daughter = split(
        tables,
        &campaign.map,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        &mut campaign.mercenaries,
        parent,
        &basket,
        SplitInto::Castle { county: 1, tile: CASTLE },
        *year,
    )
    .expect("the castle has room for a hundred");

    assert_eq!(k.counties[1].garrison_unit, 0, "she has not arrived yet");
    assert!(k.campaign.units.get(daughter).expect("the daughter").moving);
    until_still(&mut k);

    assert_eq!(k.counties[1].garrison_unit, daughter, "the castle took her in");
    let d = k.campaign.units.get(daughter).expect("the garrison");
    assert_eq!(d.garrison_county, 1, "unit +0x198");
    assert_eq!(d.men, 100, "the men that left the parent");
    assert_eq!(d.tile(), CASTLE, "teleported onto county +0x74/+0x75");
    assert_eq!(d.moves_used, 17, "twelve walked and the garrison's five");
    assert_eq!(k.campaign.units.get(parent).expect("the parent").men, 200, "the count moved");
}

/// **No minimum on a castle split** — `Army_SplitConfirm` (`0x00437AFB`) tests
/// 50 a side only with no destination county, which is the shipped Readme's
/// *"When splitting into castles, you may split off less than 50 men."*
#[test]
fn a_castle_split_may_be_under_fifty_men() {
    let mut k = kingdom();
    let parent = army(&mut k, (26, 26), 300);
    let mut basket = SplitBasket::seed(k.campaign.units.get(parent).expect("the parent"));
    assert_eq!(basket.to_daughter(TroopType::Peasant, 20), 20);

    let Kingdom { tables, counties, realms, campaign, year, .. } = &mut k;
    let daughter = split(
        tables,
        &campaign.map,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        &mut campaign.mercenaries,
        parent,
        &basket,
        SplitInto::Castle { county: 1, tile: CASTLE },
        *year,
    )
    .expect("twenty men may garrison");
    until_still(&mut k);
    assert_eq!(k.counties[1].garrison_unit, daughter);
    assert_eq!(k.campaign.units.get(daughter).expect("the garrison").men, 20);
}

/// **A garrison ordered out of its castle** — `Army_LeaveCastle` (`0x004374C4`)
/// and its body `FUN_00437535`: a free tile from `Map_FindFreeTileNear`
/// (`0x0046733C`), both links cleared, **no moves charged** and no order given.
#[test]
fn a_garrison_marches_out_onto_a_free_tile_and_pays_nothing() {
    let mut k = kingdom();
    let g = army(&mut k, CASTLE, 150);
    {
        let u = k.campaign.units.get_mut(g).expect("the garrison");
        u.garrison_county = 1;
        u.moves_used = 0;
    }
    k.counties[1].garrison_unit = g;

    let Kingdom { counties, realms, campaign, .. } = &mut k;
    let out = leave_castle(&campaign.map, counties, realms, &mut campaign.units, g, 1);

    let LeftCastle::Marched { tile, sortie } = out else { panic!("she marched: {out:?}") };
    assert_eq!(sortie, None, "nobody was besieging");
    assert_ne!(tile, CASTLE, "off the castle tile");
    assert_eq!(k.counties[1].garrison_unit, 0, "county +0x1BC cleared");
    let u = k.campaign.units.get(g).expect("standing in the open");
    assert_eq!(u.garrison_county, 0, "unit +0x198 cleared");
    assert_eq!(u.tile(), tile);
    assert_eq!(u.men, 150, "the men came out with her");
    assert_eq!(u.moves_used, 0, "the way out is free; only the way in costs five");
    assert_eq!(u.move_allowance - u.moves_used, 15, "and she can march this season");
}

/// **A besieged garrison's sortie names its besieger.** `FUN_00437535` hands
/// `besiegedBy` to `Battle_BeginFromCampaign` and leaves the link standing.
#[test]
fn a_besieged_garrison_leaves_carrying_its_besieger() {
    let mut k = kingdom();
    let g = army(&mut k, CASTLE, 150);
    let besieger = army(&mut k, (24, 20), 200);
    {
        let u = k.campaign.units.get_mut(g).expect("the garrison");
        u.garrison_county = 1;
        u.besieged_by = besieger as u8;
    }
    k.counties[1].garrison_unit = g;
    k.campaign.units.get_mut(besieger).expect("the besieger").besieging_county = 1;

    let Kingdom { counties, realms, campaign, .. } = &mut k;
    let out = leave_castle(&campaign.map, counties, realms, &mut campaign.units, g, 1);
    assert!(matches!(out, LeftCastle::Marched { sortie: Some(b), .. } if b == besieger));
    assert_eq!(
        k.campaign.units.get(g).expect("the garrison").besieged_by as usize,
        besieger,
        "the siege link survives the step out"
    );
}

/// **Nowhere to stand is the end of the army** — `Army_Destroy`, the branch a
/// player can be surprised by.
#[test]
fn a_garrison_with_nowhere_to_stand_is_destroyed() {
    let mut k = kingdom();
    let g = army(&mut k, CASTLE, 150);
    k.campaign.units.get_mut(g).expect("the garrison").garrison_county = 1;
    k.counties[1].garrison_unit = g;
    // Sea to the horizon: nothing within five is passable, so the search fails.
    for i in 0..MAP_TILES {
        k.campaign.map.county[i] = 0;
        k.campaign.map.flags[i] |= flags::NO_COUNTY;
    }

    let Kingdom { counties, realms, campaign, .. } = &mut k;
    let out = leave_castle(&campaign.map, counties, realms, &mut campaign.units, g, 1);
    assert_eq!(out, LeftCastle::Destroyed);
    assert!(k.campaign.units.get(g).is_none(), "Army_Destroy");
    assert_eq!(k.counties[1].garrison_unit, 0);
}
