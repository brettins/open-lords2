#![allow(unused_imports)]
use super::*;
use super::army_movement::*;
use super::combat::*;
use super::merchants_and_mobs::*;
use super::purity::*;
use super::*;
use super::spine::*;
use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

#[test]
fn an_army_crossing_a_foreign_field_wrecks_it_during_the_turn() {
    let mut g = with_a_map();
    for x in 6..=9u8 {
        for y in 0..64u8 {
            g.kingdom.campaign.map.set_flags(x, y, flags::ROUGH);
        }
        g.kingdom.campaign.map.set_flags(x, 20, flags::FARMLAND);
        g.kingdom.campaign.map.set_terrain(x, 20, 8);
    }
    assert_eq!(g.kingdom.counties[2].owner, 2, "somebody else's fields");
    // `County_DestroyField` takes the tile's share of the standing crop, and it
    // charges the grain arm against `+0x206` — the sown fields still standing,
    // which a sowing writes beside `fields_grain`. This comment used to say that
    // with `fields_grain` at 0 the function returns without repainting; that was
    // our port, not the original, whose `Terrain_Set(tile, 0)` is outside the
    // `if` (`docs/decisions.md` C195). A county with a standing
    // crop has sown it, so it carries both numbers.
    g.kingdom.counties[2].fields_grain = 4;
    g.kingdom.counties[2].fields_grain_standing = 4;
    g.kingdom.counties[2].crop[1] = 400;

    let id = army(&mut g, 1, 5, 20);
    g.order_unit_move(id, (9, 20)).expect("a path across the fields");

    let outcome = turn::end_turn(&mut g).unwrap();

    assert!(outcome.steps > 0, "the army walked");
    assert!(
        (6..=9u8).any(|x| !g.kingdom.campaign.map.is_standing_field(x, 20)),
        "at least one field was trampled"
    );
    assert!(
        g.kingdom.counties[2].fields_grain < 4,
        "and the county lost a field with it"
    );
}

