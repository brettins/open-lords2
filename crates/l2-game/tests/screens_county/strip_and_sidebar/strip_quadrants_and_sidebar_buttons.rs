#![allow(unused_imports)]
use super::*;
use super::strip_slider_and_pointer::*;
use super::strip_closing_and_numbers::*;
use super::*;
use super::panels::*;
use super::drawing_and_emboss::*;
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

#[test]
fn each_quadrant_of_the_strip_opens_its_own_panel_from_the_campaign_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    for panel in county::PANELS {
        let hot = panel.strip_hotspot();
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: hot.centre_x(), y: hot.y + hot.h / 2 },
        );
        assert_eq!(t, Transition::Push(ScreenId::County(8, panel)), "{panel:?}'s own quadrant");
    }
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: 558, y: 200 });
    assert_eq!(t, Transition::Stay, "the health bar is deliberately not clickable");
}

/// **The five sidebar buttons.** `g_sidebarButtons` (`0x004DC680`) is a table of
/// five, and every one of them sets a `g_screenId` we can draw. They were under
/// a rectangle of ours that opened the county panel and wrote COUNTY PANEL
/// across their artwork.
#[test]
fn the_five_sidebar_buttons_each_open_the_screen_the_original_opens() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    assert!(game.is_players(8), "county 8 is the player's, so the gated three are allowed");
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        assert_eq!(
            t,
            Transition::Push(map::sidebar_destination(id, 8)),
            "{} opens {id:#04X}",
            b.name
        );
    }

    game.select(1);
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        if matches!(id, 0x17 | 0x18 | 0x1B) {
            assert_eq!(t, Transition::Stay, "{} is refused on another realm's county", b.name);
        } else {
            assert_eq!(
                t,
                Transition::Push(map::sidebar_destination(id, game.selected)),
                "{} is not gated",
                b.name,
            );
            // The ungated two are the court (`0x09`, still a shell) and the
            // lords (`0x0B`, which has graduated) — so this asks
            // `sidebar_destination` too. The
            // ungating is the point: **the diplomacy screen is about realms,
            // not counties**, and `FUN_0043611B` has no county gate at all.
            assert_eq!(t, Transition::Push(map::sidebar_destination(id, 1)), "{} is not gated", b.name);
        }
    }
}

