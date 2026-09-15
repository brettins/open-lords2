#![allow(unused_imports)]
use super::*;

use super::*;
use super::battle_part::*;
use super::raising::*;
use super::marching::*;
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
fn a_selected_army_opens_the_division_screen_and_an_unselected_one_does_not() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'a');
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "there is nothing selected");

    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)));
}

/// `g_splitWidgets`' steppers are `Widget_Test` kind 4, whose guard reads
/// `g_mouseLeftPressed || g_mouseLeftDoubleClick`,
/// again. And `App_WndProc` (`0x004B29BE`) handles `WM_LBUTTONDBLCLK` by setting
/// the double-click bit and nothing else, so `g_mouseLeftDown` stays clear and
/// the hold branch returns: however long the button stays down after a double
/// click, it does not repeat. This screen dropped the double click entirely.
#[test]
fn a_double_click_on_a_divide_stepper_steps_once_more_and_does_not_hold() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);

    let at = on(divide::parent_button(0));
    for _ in 0..5 {
        click(&mut m, &mut g, &a, at);
        send(&mut m, &mut g, &a, Event::Release { x: at.0, y: at.1 });
    }
    send(&mut m, &mut g, &a, Event::DoubleClick { x: at.0, y: at.1 });
    for _ in 0..200 {
        tick(&mut m, &mut g, &a);
    }
    press_and_wait(&mut m, &mut g, &a, on(divide::SPLIT_TICK));

    assert_eq!(g.kingdom.campaign.units.len(), 2, "the split went through");
    let daughter = g.kingdom.campaign.units.iter().map(|(_, u)| u.men).min().unwrap();
    assert_eq!(daughter, 60, "five presses and a double click, ten men each, and no repeat");
}

#[test]
fn the_division_screen_splits_an_army_in_two_and_both_halves_pay_five_moves() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);

    for _ in 0..12 {
        click(&mut m, &mut g, &a, on(divide::parent_button(0)));
    }
    press_and_wait(&mut m, &mut g, &a, on(divide::SPLIT_TICK));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the screen closed");
    assert_eq!(g.kingdom.campaign.units.len(), 2, "two armies now");
    let parent = g.kingdom.campaign.units.get(id).expect("the parent").clone();
    assert_eq!(parent.men, 180);
    assert_eq!(parent.moves_used, l2_kingdom::divide::SPLIT_MOVE_COST);
    let (_, daughter) = g
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(slot, _)| *slot != id)
        .expect("the daughter");
    assert_eq!(daughter.men, 120);
    assert_eq!(daughter.owner, 1);
    assert_eq!(daughter.morale, parent.morale, "she inherits the parent's morale");
    assert_eq!(daughter.moves_used, l2_kingdom::divide::SPLIT_MOVE_COST);
    assert_eq!(parent.men + daughter.men, 300, "men are conserved");
    assert_ne!((daughter.x, daughter.y), (parent.x, parent.y), "and she stands elsewhere");
}

#[test]
fn a_split_that_would_leave_fewer_than_fifty_a_side_is_refused_on_the_screen() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);

    for _ in 0..3 {
        click(&mut m, &mut g, &a, on(divide::parent_button(0)));
    }
    press_and_wait(&mut m, &mut g, &a, on(divide::SPLIT_TICK));
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)), "the screen stays up");
    assert_eq!(g.kingdom.campaign.units.len(), 1, "and nothing was split");
}

/// It travels the original's road now: `Panel_DisbandButton` is record 1 of the
/// **information panel's** table and not a button on the division screen, which
/// is where a button of ours used to be. `docs/arms.json`
/// `0x00437002/info-disband`.
#[test]
fn the_disband_button_returns_the_men_to_their_county_and_the_weapons_to_the_realm() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    g.kingdom.campaign.units.get_mut(id).unwrap().troops = [100, 0, 0, 200, 0, 0, 0];
    let (pop, swords) = (g.kingdom.counties[1].population, g.kingdom.realms[1].weapons[2]);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))),
        "the right release opens the unit half",
    );
    click(&mut m, &mut g, &a, info_button(1));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    assert!(g.kingdom.campaign.units.get(id).is_none(), "the army is gone");
    assert_eq!(g.kingdom.counties[1].population, pop + 300, "the men joined the county");
    assert_eq!(g.kingdom.realms[1].weapons[2], swords + 200, "two hundred swords came back");
}

/// The Readme's disband rule, both clauses, through the screen: an army whose
/// home county has fallen *and* which is standing in enemy country is refused,
/// with `L2.eng` group 145's own words for why.
#[test]
fn an_army_with_no_friendly_county_to_go_to_cannot_disband() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    {
        let u = g.kingdom.campaign.units.get_mut(id).unwrap();
        u.home_county = 1;
        u.county = 2; // standing in the enemy's county
    }
    g.kingdom.counties[1].owner = 2; // and home has fallen

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(1));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel closes either way");
    assert!(g.kingdom.campaign.units.get(id).is_some(), "and the army is still there");
}

#[test]
fn the_move_button_on_the_information_panel_starts_a_move_order() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(0));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel closed");
    assert_eq!(g.begin_move_order, Some(id), "and asked for the selection");
    tick(&mut m, &mut g, &a);
    assert_eq!(g.begin_move_order, None, "which the map consumed");
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "move-order mode is this screen with a selection, not another screen",
    );
}

/// **The Split button pushes, so `0x11` goes back to
/// `0x04`** — every one of the division screen's three exits writes
/// `g_screenId = 0x04`, not 0. `docs/arms.json`
/// `0x0042FF10/back-one-rather-than-to-the-map`.
#[test]
fn the_division_screen_returns_to_the_information_panel_and_not_to_the_map() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    right_click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, info_button(2));
    assert_eq!(m.top_id(), Some(ScreenId::Divide(id)));
    press_and_wait(&mut m, &mut g, &a, on(divide::SPLIT_CROSS));
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))),
        "back to the panel the Split button was on",
    );
    click(&mut m, &mut g, &a, info_button(2));
    right_click(&mut m, &mut g, &a, (200, 200));
    assert_eq!(m.top_id(), Some(ScreenId::Info(l2_game::screens::info::Target::Unit(id))));
}


#[test]
fn the_division_screen_paints_inside_the_window_the_painter_opens() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    army_at(&mut g, 1, 1, 300, here);
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    let before = frame(&mut m, &mut g, &a);

    press(&mut m, &mut g, &a, 'a');
    tick(&mut m, &mut g, &a);
    let after = frame(&mut m, &mut g, &a);

    let window = divide::window();
    let (mut inside, mut above) = (0usize, 0usize);
    for (x, y) in differing(&before, &after) {
        if window.contains(x, y) {
            inside += 1;
        } else if y < window.y {
            above += 1;
        }
    }
    assert!(inside > 500, "the painter drew almost nothing: {inside} pixels");
    assert_eq!(above, 0, "it painted above its own window, over the map");
}

#[test]
fn the_levy_window_lifts_off_the_armoury_and_leaves_the_room_behind() {
    let (mut g, a, mut m) = on_the_map();
    press(&mut m, &mut g, &a, 'r');
    tick(&mut m, &mut g, &a);
    let with_window = frame(&mut m, &mut g, &a);

    press_and_wait(&mut m, &mut g, &a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    let armoury_only = frame(&mut m, &mut g, &a);

    let window = army::window(false);
    let changed = differing(&with_window, &armoury_only);
    assert!(!changed.is_empty(), "Continue changed nothing at all");
    let outside = changed.iter().filter(|&&(x, y)| !window.contains(x, y)).count();
    let inside = changed.len() - outside;
    assert!(inside > 500, "the levy window did not lift: {inside} pixels changed inside it");
    let elsewhere = (640 * 480 - window.w * window.h) as usize;
    assert!(
        outside * 20 < elsewhere,
        "{outside} of {elsewhere} pixels outside the levy window changed: \
         the two screens are not standing on the same picture",
    );
}


