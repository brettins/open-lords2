#![allow(unused_imports)]
use super::*;
use super::selection_and_orders::*;
use super::siege_and_garrison::*;
use super::merging::*;
use super::movement_and_turn::*;
use super::*;
use super::battle_part::*;
use super::raising::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;


#[test]
fn the_two_screens_have_graduated_out_of_the_shell_table() {
    use l2_game::screens::shells;
    assert!(shells::find(0x17).is_none(), "the raise-army screen is implemented");
    assert!(shells::find(0x11).is_none(), "the army-division screen is implemented");
    assert!(
        !shells::SHELLS.iter().any(|s| s.name.to_lowercase().contains("mercenaries")),
        "and nothing in the table still claims there is a mercenaries screen",
    );
}

/// `FUN_00405487` (`0x00405487`) walks the screen lattice and calls
/// `Map_DrawArmies` (`0x00408438`) once per *cell*; the painter's whole body is
/// a walk of that one tile's unit list. So the pass is ordered by lattice row
/// then column, and a figure on a lower row covers one behind it. Ours iterated
/// `g_units` instead, so the lower id won — see
/// [`map::units_in_paint_order`](l2_game::screens::map::units_in_paint_order).
///
/// **Ablation.** Drop the `sort_unstable` in `units_in_paint_order` and the
/// first assertion goes red: the fixture's slots are deliberately the reverse
/// of its rows.
#[test]
fn armies_are_painted_down_the_lattice_and_not_up_the_unit_array() {
    let (mut g, _a) = world();
    let front = army_at(&mut g, 1, 1, 100, (6, 6)); // row 13
    let middle = army_at(&mut g, 1, 1, 100, (5, 5)); // row 11
    let sharing = army_at(&mut g, 1, 1, 100, (5, 5)); // row 11, the same tile
    let back = army_at(&mut g, 1, 1, 100, (4, 4)); // row 9
    assert!(front < middle && middle < sharing && sharing < back, "slots descend with depth");

    let order = map::units_in_paint_order(&g);
    assert_eq!(
        order,
        vec![back, middle, sharing, front],
        "the pass is lattice row, then column, then the tile's own list"
    );

    g.kingdom.campaign.units.get_mut(back).expect("the army").y = 9; // row 14
    assert_eq!(
        map::units_in_paint_order(&g).last().copied(),
        Some(back),
        "an army that marched to the front row is painted last"
    );
}


