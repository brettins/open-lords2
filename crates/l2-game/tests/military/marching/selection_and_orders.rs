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
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "the deselected army took no order",
    );
}

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

    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "a deselected army takes no orders",
    );

    send(&mut m, &mut g, &a, Event::RightClick { x: hx, y: hy });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "with nothing picked the right button still opens the information panel",
    );
}

#[test]
fn an_unreachable_destination_is_ordered_with_an_empty_path_and_the_army_stands() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x > 26 && x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
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

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| !u.moving),
        "g_moveOrderAvailable was never set, so Map_ConfirmMoveOrder returned",
    );

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

    end_turn(&mut m, &mut g, &a);
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!((unit.x, unit.y), at, "the army stood exactly where it was");
    assert_eq!(unit.moves_used, 0, "and spent nothing standing there");
}

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

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "and neither did the second one");
}

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
    send(&mut m, &mut g, &a, Event::Pointer { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

