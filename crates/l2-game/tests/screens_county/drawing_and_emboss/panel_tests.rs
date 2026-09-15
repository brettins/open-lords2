#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::county_name_tests::*;
use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::produce_and_pastures::*;
use super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// `docs/arms.json` `0x0042FF10/inset-runs-the-sidebar-guards`. This is the arm
/// that was missing: `CountyScreen::handle` tested the four strip quadrants
/// itself and returned `Stay` for everything else in the column, so with the tax
/// panel open a player could not touch the minimap, the five sidebar buttons,
/// the farm/industry slider, the produce rows or End Turn.
#[test]
fn a_county_panel_leaves_the_whole_sidebar_live_underneath_it() {
    let (mut game, assets) = world!();
    game.select(8);
    assert!(game.is_players(8), "the fixture's county 8 is the local player's");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let court = map::SIDEBAR_BUTTONS[1].rect();
    send_stack(&mut m, &mut game, &assets, Event::Click { x: court.centre_x(), y: court.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Court), "sidebar button 2 opens the court");
    assert_eq!(m.depth(), 2, "and it replaced the panel rather than stacking on it");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let mode = map::MINIMAP_MODE_BUTTONS[0];
    send_stack(&mut m, &mut game, &assets, Event::Click { x: mode.centre_x(), y: mode.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::County(8, Panel::Tax)), "the panel is still open");

    let mut m = over_the_map(ScreenId::County(8, Panel::Ration));
    game.kingdom.counties[8].industry_share = 40;
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(
        game.kingdom.counties[8].industry_share, 0,
        "the press lands on the track's left edge, which is share 0"
    );

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let rows = county::farm_rows(&game.kingdom.counties[8]);
    assert!(!rows.is_empty(), "the fixture's county 8 farms something");
    let pitch = county::farm_pitch(rows.len());
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 500, y: 0x12E + pitch / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, rows[0])), "the first farm row's job popup");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 550, y: 470 });
    assert_ne!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "End Turn is reachable through the panel"
    );
}

#[test]
fn a_county_panel_still_closes_on_its_corner_and_on_the_right_button() {
    let (mut game, assets) = world!();
    game.select(8);

    // **On the release.** `Ui_OkButtonClicked` (`0x0040E7E4`) opens
    // `if (g_mouseLeftReleased == 0) return 0;`, and this test used to drive a
    // press — which passed, because ours answered on the press too. It is the
    // shape a player reported from the other side: *"the game waited on
    // mouse-up."*
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let ok = Panel::Tax.ok_button();
    let (okx, oky) = (ok.centre_x(), ok.y + 4);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: okx, y: oky });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "the PRESS on the corner does nothing — the original tests the release",
    );
    send_stack(&mut m, &mut game, &assets, Event::Release { x: okx, y: oky });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Ui_OkButtonClicked's 24 x 24 corner");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 200 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "a right release anywhere");

    // The ablation: a click on the middle of the panel does nothing at all. Both
    // assertions above would still pass if `handle` closed on any click.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 200, y: 200 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "a click on the panel is not an exit"
    );
}

#[test]
fn the_split_slider_steps_up_at_594_and_refuses_a_county_you_do_not_hold() {
    assert_eq!(map::split_from_click(593, 40), 100, "593 is still the track, and the track clamps at 100");
    assert_eq!(map::split_from_click(594, 40), 44, "594 is the first pixel of the up zone");
    assert_eq!(map::split_from_click(530, 40), 36, "530 is the last pixel of the down zone");
    assert_eq!(map::split_from_click(531, 40), 0, "531 is the first pixel of the track");
    assert_eq!(map::split_from_click(639, 100), 100, "clamped at 100");
    assert_eq!(map::split_from_click(0, 0), 0, "and at 0");

    let (mut game, assets) = world!();
    let other = (1..game.kingdom.counties.len() as u8)
        .find(|&id| game.kingdom.counties[id as usize].owner != game.player);
    let Some(other) = other else { return };
    game.select(other);
    let before = game.kingdom.counties[other as usize].industry_share;
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 560, y: 270 });
    assert_eq!(
        game.kingdom.counties[other as usize].industry_share, before,
        "another lord's peasants do not move"
    );
}


