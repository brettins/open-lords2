#![allow(unused_imports)]
use super::*;
use super::selection_and_orders::*;
use super::siege_and_garrison::*;
use super::movement_and_turn::*;
use super::rendering::*;
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

/// **The halt that asks.** A march whose route runs onto your own army stops
/// on `Entry::Occupied` — the original's tile stacks and ours does not — and
/// the map puts up `Ui_OpenConfirm(5, …)`, `L2.eng` group 10 index 5
/// *"Combine armies?"*, the question `Map_ConfirmMoveOrder` (`0x004A9252`)
/// raises off `g_hoverMergeUnit`. Yes is `Army_Combine` (`0x004AA181`) **into
/// the standing army**, the direction `MoveOrder_ConfirmCombine`
/// (`0x004A975D`) fixes with `Unit_OrderMove`'s fifth argument.
#[test]
fn a_march_onto_your_own_army_asks_to_combine_and_yes_merges_them() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let mover = army_at(&mut g, 1, 1, 120, here);
    let standing = army_at(&mut g, 1, 1, 200, there);
    assert!(g.order_unit_move(mover, there).is_some_and(|n| n > 0), "the march is ordered");

    run_until(&mut m, &mut g, &a, "the combine question", |_, g| g.combine_ask.is_some());
    assert_eq!(g.combine_ask, Some((mover, standing)), "the halted pair, mover first");

    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    assert!(g.kingdom.campaign.units.get(mover).is_none(), "the mover's slot is gone");
    assert_eq!(
        g.kingdom.campaign.units.get(standing).map(|u| u.men),
        Some(320),
        "and the men are in the army that was standing there",
    );
}

/// **No is the halt that was there before the question** — both armies stand,
/// and nothing is asked again.
#[test]
fn a_march_onto_your_own_army_answered_no_leaves_both_armies_standing() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let mover = army_at(&mut g, 1, 1, 120, here);
    let standing = army_at(&mut g, 1, 1, 200, there);
    g.order_unit_move(mover, there).expect("the march is ordered");

    run_until(&mut m, &mut g, &a, "the combine question", |_, g| g.combine_ask.is_some());

    press_and_wait(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_NO));
    assert!(g.combine_ask.is_none(), "the question is answered and does not come back");
    assert_eq!(
        g.kingdom.campaign.units.get(mover).map(|u| (u.x, u.y)),
        Some(here),
        "the mover stands where Entry::Occupied stopped it",
    );
    assert_eq!(g.kingdom.campaign.units.get(standing).map(|u| u.men), Some(200), "and so does she");
}

