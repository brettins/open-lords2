#![allow(unused_imports)]
use super::*;
use super::siege_and_garrison::*;
use super::merging::*;
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

// ---------------------------------------------------------------------------
// 2. March
// ---------------------------------------------------------------------------

/// The two clicks: one selects, one orders. Both land on the map,
/// is never left.
#[test]
fn two_clicks_on_the_map_select_an_army_and_send_it_marching() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "selecting an army does not leave the map");
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving), "not ordered yet");

    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "Unit_OrderMove writes moveState = 2");
    assert_eq!(unit.dest, Some(there));
    assert!(!unit.path.is_empty(), "and a path to walk");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the map is still what is on screen");
}

/// **A second click on the selected army ends the selection and orders
/// nothing — and it is a refused destination.**
///
/// The distinction is the whole of what was wrong here. We had an explicit
/// "clicking the same army again deselects it" branch in `click_unit`, which
/// was ours: while move-order mode is up, `g_screenId` is `0x10` and
/// `Map_Click` is not reachable at all, so the original never sees a second
/// click on an army *as* a click on an army. It sees a destination, and
/// `Map_HoverUnitTarget` has already cleared `g_moveOrderAvailable` for that
/// tile because the flood fill's raw distance there is 1 — the army is
/// standing on it. `Map_ConfirmMoveOrder` returns without writing anything,
/// and `Screen_FrameInput` had already put the screen back to `0`.
///
/// Same outcome, different mechanism,
/// **every** tile the fill did not reach behaves this way.
#[test]
fn clicking_the_selected_army_again_ends_the_selection_and_orders_nothing() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a destination the fill never reached is not an order",
    );
    // And the selection is gone,
    //
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "the deselected army took no order",
    );
}

/// **The right button is how an army is deselected, and it was missing.**
///
/// A player, one minute after reporting that the march preview only appears
/// after the click: *"you cannot deselect an army."* Both were the same gap.
/// `Screen_FrameInput`'s `0x10` arm has exactly four clauses, and this is one
/// of them: `if (g_mouseRightReleased != 0) { g_screenId = 0; g_redrawRequest
/// = 2; }`. We had the right button bound to screen `0`'s arm — the
/// information panel — with no test for the mode, so it opened a panel where
/// the original cancels.
#[test]
fn the_right_button_deselects_an_army_and_does_not_open_the_information_panel() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    let (hx, hy) = pixel(here.0, here.1).unwrap();
    click(&mut m, &mut g, &a, (hx, hy));

    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the information panel did not open over the selection",
    );

    // The selection is gone,
    // on open ground is no longer a destination: it is a click on plain
    // ground, which does nothing at all.
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a deselected army takes no orders",
    );

    // And with nothing selected the same gesture reaches screen 0x04, which
// is what makes the arm above a *mode* test.
    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "with nothing picked the right button still opens the information panel",
    );
}

/// **An unreachable destination is an accepted order with nothing in it**, and
/// that distinction is `docs/armies.md` §2.3's own correction.
///
/// `Move_ExtractPath` returns *success* with a zero-length path when the greedy
/// descent never reached the destination, so `Unit_OrderMove` writes the
/// destination, sets `moveState = 2` and copies an empty path. The army is
/// ordered and stands still. Only a dead end in the descent returns 0 and
/// leaves the record untouched.
///
/// A reimplementation that treated "no path" as a refusal would leave
/// `moveState` at 0,
/// lockstep difference, not a cosmetic one.
///
/// **But a human click on the map cannot reach that state, and this test used
/// to say it could.** `Map_ConfirmMoveOrder` opens with `if
/// (g_moveOrderAvailable != 1) return;`, and `Map_HoverUnitTarget` clears that
/// flag whenever the flood fill's raw distance at the hovered tile is below 2
/// — which is every tile the fill never reached. So the empty-path order is
/// real and is what the AI, the network command and the phase machine produce;
/// **from the map it is unreachable, because a gate stands in front of it that
/// we had not implemented.** The order is asserted where it happens,
/// in `l2-kingdom`,
#[test]
fn an_unreachable_destination_is_ordered_with_an_empty_path_and_the_army_stands() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x > 26 && x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    // Wall the army in on all eight sides.
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            g.kingdom.campaign.map.set_flags(
                (here.0 as i32 + dx) as u8,
                (here.1 as i32 + dy) as u8,
                flags::IMPASSABLE,
            );
        }
    }
    let at = (
        g.kingdom.campaign.units.get(id).unwrap().x,
        g.kingdom.campaign.units.get(id).unwrap().y,
    );

    // The screen refuses first: two clicks place no order at all, because the
    // hover gate never lit.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "g_moveOrderAvailable was never set, so Map_ConfirmMoveOrder returned",
    );

    // `Unit_OrderMove` itself,
    // call, accepts it — and that is the part that must not drift.
    let steps = l2_kingdom::movement::order_move(
        &g.kingdom.campaign.map,
        &mut g.kingdom.campaign.units,
        id,
        there,
        l2_kingdom::movement::Routing::Direct,
    );
    assert_eq!(steps, Some(0), "success, with nothing in the buffer");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(unit.moving, "the order was accepted");
    assert_eq!(unit.dest, Some(there), "and it names the tile that was asked for");
    assert!(unit.path.is_empty(), "with no path to walk");

    // And a whole turn of ticking moves it nowhere.
    end_turn(&mut m, &mut g, &a);
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!((unit.x, unit.y), at, "the army stood exactly where it was");
    assert_eq!(unit.moves_used, 0, "and spent nothing standing there");
}

/// **One press never both selects an army and orders it.**
///
/// The original needs `g_moveOrderClickGuard` — forty frames of deadness — for
/// this, because `Screen_FrameInput` polls the mouse button's *level* every
/// frame, so one physical press is read as a click on every frame it is held
/// down and the press that opened move-order mode would otherwise be read again
/// as the press that confirms the destination.
///
/// **Ours needs no guard, and this is why**: `Event::Click` is edge-triggered —
/// `main.rs` synthesises exactly one per `WindowEvent::MouseInput{Pressed}` —
/// and `Map_Click`'s army branch `return`s, so the selecting click cannot fall
/// through to `Map_ConfirmMoveOrder` in the same call. The guard is a
/// consequence of a polled input model we do not have. This asserts the
/// property the guard exists to protect,
/// would mean nothing here.
#[test]
fn the_click_that_selects_an_army_never_also_orders_it() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the selecting click did not also place an order");
    assert_eq!(u.dest, None, "and named no destination");
    assert!(u.path.is_empty());
    assert_eq!(u.moves_used, 0, "and cost nothing");

    // Repeating the same press — which is what a held button looks like to a
    // polled reader — orders nothing either, by the original's own route: the
    // tile is the army's own, the fill's distance there is `START_DISTANCE`,
    // and `Map_ConfirmMoveOrder` returns on `g_moveOrderAvailable != 1`.
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "and neither did the second one");
}

/// **The map stops taking orders while the turn is being wound on.**
///
/// Every hotspot on the map writes to state the phase machine is in the middle
/// of reading. A click that landed mid-turn would race it, so it is refused —
/// and pointer motion is *not*, because a frozen cursor would look like a hang
///
#[test]
fn the_map_refuses_orders_while_the_turn_is_running() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    press(&mut m, &mut g, &a, 'e');
    tick(&mut m, &mut g, &a);
    assert!(l2_game::turn::turn_in_flight(&g), "a turn is in flight");

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert_eq!(
        g.kingdom.campaign.units.get(id).and_then(|u| u.dest),
        None,
        "no order was taken from a click during the turn",
    );
    // Pointer motion still arrives, so the map can still scroll under the
    // cursor while the season winds on.
    send(&mut m, &mut g, &a, Event::Pointer { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

