//! The wiring: `PeasantMob_Tick`'s call to `FUN_004ABD0F` from
//! [`crate::units_tick`], and the three kinds that do not make it.

use crate::kingdom::Kingdom;
use crate::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use crate::movement;
use crate::unit::{Unit, UnitKind};
use crate::units_tick::{Posted, OWNERLESS};
use crate::Options;

/// Counties 1 and 2 either side of x = 32, a road along y = 10. County 2 is
/// realm 2's, which gives the letter somebody to be addressed to.
fn kingdom(happiness: i32) -> Kingdom {
    let mut k = Kingdom::new(0x2E5);
    assert!(k.set_county_count(2));
    k.year = 1268;
    k.options = Options { armies_eat: false, ..Options::default() };

    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    k.campaign.map = map;
    for id in 1..=2 {
        k.counties[id].population = 500;
        k.counties[id].happiness = 70;
        k.counties[id].owner = id as u8;
        k.counties[id].anchor_x = if id == 1 { 10 } else { 50 };
        k.counties[id].anchor_y = 10;
        k.realms[id].in_play = true;
    }
    k.counties[2].happiness = happiness;
    k
}

fn walk(k: &mut Kingdom, kind: UnitKind, owner: u8) -> Vec<Posted> {
    let mut u = Unit::new(kind, owner, 30, 10);
    u.men = 100;
    u.troops[0] = 100;
    u.county = 1;
    let id = k.campaign.units.spawn(u).expect("a free slot");
    movement::order_move(
        &k.campaign.map,
        &mut k.campaign.units,
        id,
        (36, 10),
        movement::Routing::Direct,
    )
    .expect("a road path");
    let mut posted = Vec::new();
    for _ in 0..80 {
        posted.extend(k.tick_units().posted);
    }
    posted
}

/// **A mob walking into a contented county posts 156 and costs it ten.**
///
/// Ablation: delete the `UnitKind::PeasantMob` arm in `step_one` and both
/// asserts go red; delete the `settle` call and only the second does.
#[test]
fn a_mob_crossing_a_border_writes_to_the_counties_owner_and_takes_ten() {
    let mut k = kingdom(70);
    let posted = walk(&mut k, UnitKind::PeasantMob, OWNERLESS);
    let letters: Vec<_> = posted
        .iter()
        .filter_map(|p| match p {
            Posted::Letter(l) => Some((l.from, l.to, l.group, l.category, l.county)),
            _ => None,
        })
        .collect();
    assert_eq!(letters, vec![(0, 2, super::GROUP_PEOPLE_TROUBLED, 3, 2)], "{posted:?}");
    assert_eq!(k.counties[2].happiness, 60);
    assert_eq!(k.counties[2].shown_events, -10);
}

/// **A wretched county is raised by the mob that walked in**: letter 154, the
/// county leaves its owner, the population loses the men who joined, and the
/// thirty back puts happiness at `4 + 30 - 10`.
///
/// Ablation: drop the `make_county_independent` and the owner stays 2; drop the
/// `population -= men` and it stays 500.
#[test]
fn a_mob_entering_a_wretched_county_raises_it_and_leaves_it_independent() {
    let mut k = kingdom(4);
    k.counties[2].unrest = 3;
    let posted = walk(&mut k, UnitKind::PeasantMob, OWNERLESS);
    let groups: Vec<u16> = posted
        .iter()
        .filter_map(|p| match p {
            Posted::Letter(l) => Some(l.group),
            _ => None,
        })
        .collect();
    assert_eq!(groups, vec![super::GROUP_REVOLUTION_SPREAD], "{posted:?}");
    assert_eq!(k.counties[2].owner, 0, "County_RaiseRevolt makes it independent");
    assert_eq!(k.counties[2].population, 350, "30% of 500 left with the mob");
    assert_eq!((k.counties[2].happiness, k.counties[2].unrest), (24, 0));
}

/// **The letter belongs to the mob and to nothing else.** An army crossing the
/// same border is `Unit_EnterCounty`'s business — it is not its destination
/// county and county 2 is owned, so the army says nothing at all; a merchant
/// says nothing anywhere.
///
/// Ablation: widen the kind test to `is_combatant()` and the army row goes red.
#[test]
fn an_army_and_a_merchant_crossing_the_same_border_say_none_of_this() {
    for kind in [UnitKind::Army, UnitKind::Merchant] {
        let mut k = kingdom(70);
        let posted = walk(&mut k, kind, 1);
        let mob_letters: Vec<_> = posted
            .iter()
            .filter(|p| matches!(p, Posted::Letter(l) if (0x9A..=0x9C).contains(&l.group)))
            .collect();
        assert!(mob_letters.is_empty(), "{kind:?}: {posted:?}");
        assert_eq!(k.counties[2].happiness, 70, "{kind:?} took nothing");
    }
}
