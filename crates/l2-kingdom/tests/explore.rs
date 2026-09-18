//! | line deleted | test that goes red |
//! |---|---|
//! | `explored.reveal_square(..)` at the end of `levy::create_army` | `raising_an_army_…` (0 tiles, not 169) |
//! | the reveal at the top of `Kingdom::tick_units`' loop | `an_army_walking_…` (169, not 299) |
//! | the arrival reveal in `step_one` | `an_army_walking_…` (286, not 299) |
//! | `u.kind == UnitKind::Army` in both reveals at once | `a_merchant_walking_…` |
//! | `explored.reveal_county(..)` in `conquest::change_owner` | `taking_a_county_…` |
//! | the `explored` section in `save::encode_campaign` | `the_seen_plane_…` (decode fails) |

use l2_kingdom::conquest::change_owner;
use l2_kingdom::explore::{Explored, ARMY_SIGHT};
use l2_kingdom::levy::{create_army, LevyBasket, Muster, OWNERLESS};
use l2_kingdom::map::{flags, index};
use l2_kingdom::movement::{order_move, Routing};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::{CampaignMap, Kingdom, Unit, UnitKind};

fn one_county() -> Kingdom {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(2));
    for i in 0..l2_kingdom::MAP_TILES {
        k.campaign.map.county[i] = 1;
    }
    let c = &mut k.counties[1];
    c.owner = 1;
    c.anchor_x = 20;
    c.anchor_y = 20;
    c.population = 500;
    c.happiness = 100;
    k
}

fn raise(k: &mut Kingdom, realm: u8) -> usize {
    let basket = LevyBasket::seed(&k.realms[realm as usize], 100);
    let Kingdom { tables, counties, realms, campaign, year, .. } = k;
    create_army(
        tables,
        &campaign.map,
        counties,
        realms,
        &mut campaign.units,
        &mut campaign.names,
        &basket,
        Muster { realm, county: 1, happiness_cost: 0, year: *year },
        &mut campaign.explored,
    )
    .expect("open ground round the anchor")
}

fn seen_by_anyone(e: &Explored) -> usize {
    (0..MAX_REALMS as u8).map(|r| e.count(r)).sum()
}

/// **`Army_Create` (`0x004A9A9A`)**: `if (g_localPlayer == realm)
/// FUN_0046E067(unit.x, unit.y, 6)`. Thirteen by thirteen round the tile the
/// army was mustered on — and for the realm that raised it and nobody else.
#[test]
fn raising_an_army_shows_its_realm_thirteen_tiles_square_round_it() {
    let mut k = one_county();
    let id = raise(&mut k, 1);
    let u = k.campaign.units.get(id).unwrap();
    let (x, y) = (u.x, u.y);
    let e = &k.campaign.explored;

    assert_eq!(e.count(1), 169, "a 13 × 13 square, clear of every edge");
    assert!(e.is_seen(1, index(x, y)));
    assert!(e.is_seen(1, index(x - ARMY_SIGHT as u8, y - ARMY_SIGHT as u8)), "the corner");
    assert!(!e.is_seen(1, index(x - 7, y)), "and not a tile past it");
    assert_eq!(seen_by_anyone(e), 169, "no other realm saw any of it");
}

#[test]
fn a_militia_raised_by_nobody_shows_nobody_anything() {
    let mut k = one_county();
    k.counties[1].owner = 0;
    let id = raise(&mut k, 0);
    assert_eq!(k.campaign.units.get(id).unwrap().owner, OWNERLESS);
    assert_eq!(seen_by_anyone(&k.campaign.explored), 0);
}

fn walk(kind: UnitKind) -> Kingdom {
    let mut k = Kingdom::new(1);
    for x in 10..=20u8 {
        k.campaign.map.set_flags(x, 10, flags::ROAD);
    }
    let id = k.campaign.units.spawn(Unit::new(kind, 1, 10, 10)).unwrap();
    order_move(&k.campaign.map, &mut k.campaign.units, id, (20, 10), Routing::Direct)
        .expect("a road to walk");
    for _ in 0..10_000 {
        if !k.campaign.units.get(id).is_some_and(|u| u.moving) {
            break;
        }
        k.tick_units();
    }
    let u = k.campaign.units.get(id).unwrap();
    assert_eq!((u.x, u.y), (20, 10), "it arrived");
    k
}

/// **`Unit_Step` (`0x00465D28`)**: at every tile centre the loop reaches,
/// `if (kind == 1 && owner == g_localPlayer) FUN_0046E067(x, y, 6)`.
///
/// columns 4 … 26 and rows 4 … 16, **23 × 13 = 299** tiles. The number is not
/// taken from a run; it is the union of eleven squares, and the two ablations
/// the header lists land on 169 and 286, so each reveal is separately necessary.
#[test]
fn an_army_walking_sees_six_tiles_round_every_tile_it_reaches() {
    let k = walk(UnitKind::Army);
    let e = &k.campaign.explored;
    assert_eq!(e.count(1), 299);
    assert!(e.is_seen(1, index(4, 4)) && e.is_seen(1, index(26, 16)));
    assert!(e.is_seen(1, index(26, 10)), "the destination's own square");
    assert!(!e.is_seen(1, index(27, 10)) && !e.is_seen(1, index(15, 17)));
}

#[test]
fn a_merchant_walking_the_same_road_sees_nothing() {
    for kind in [UnitKind::Merchant, UnitKind::Transport] {
        let k = walk(kind);
        assert_eq!(seen_by_anyone(&k.campaign.explored), 0, "{kind:?}");
    }
}

/// **`County_ChangeOwner` (`0x004A72FE`)**, first statement:
///
/// `if (newOwner == g_localPlayer) FUN_0046DFD5(county)` — every tile of the
/// county, each with its one-tile border. A 5 × 4 county is seen as 7 × 6, by
/// the taker; the loser's bits are untouched, because nothing ever clears one.
#[test]
fn taking_a_county_shows_the_taker_the_county_and_a_border_round_it() {
    let mut k = Kingdom::new(1);
    assert!(k.set_county_count(2));
    let mut map = CampaignMap::empty();
    for y in 30..=33u8 {
        for x in 40..=44u8 {
            map.set_county(x, y, 2);
        }
    }
    k.counties[2].owner = 2;
    let mut explored = Explored::new();
    explored.set_seen(2, index(42, 31));

    let restore = k.restore();
    change_owner(
        &k.tables, &mut k.counties, &mut k.realms, &mut k.campaign.units, 1, 2, 0, &map,
        &mut explored, restore,
    );

    assert_eq!(k.counties[2].owner, 1);
    assert_eq!(explored.count(1), 42, "7 × 6");
    assert!(explored.is_seen(1, index(39, 29)) && explored.is_seen(1, index(45, 34)));
    assert!(!explored.is_seen(1, index(38, 31)));
    assert_eq!(explored.count(2), 1, "the loser keeps what it saw");
}

#[test]
fn the_seen_plane_survives_a_save_and_a_load() {
    let mut k = walk(UnitKind::Army);
    k.campaign.explored.reveal_square(3, 50, 50, 2);
    let bytes = l2_kingdom::save::encode(&k);
    let back = l2_kingdom::save::decode(&bytes, k.tables).expect("our own save loads");
    assert_eq!(back.campaign.explored, k.campaign.explored);
    assert_eq!(back.campaign.explored.count(1), 299);
    assert_eq!(back.campaign.explored.count(3), 25);
}
