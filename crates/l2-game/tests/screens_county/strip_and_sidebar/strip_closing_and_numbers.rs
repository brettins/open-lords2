#![allow(unused_imports)]
use super::*;
use super::strip_quadrants_and_sidebar_buttons::*;
use super::strip_slider_and_pointer::*;
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

/// `Screen_FrameInput` — the fourth and unnamed `g_screenId` dispatcher, and the one
/// that decides how every screen is *left* — has a right-release arm for almost
/// every screen id there is, and `L2.eng` group 12 index 0 is the game printing
/// *"Click Right to Exit"* on the value spinner.
///
/// On the campaign map it does the reverse: `if (onATile && rightReleased) {
/// g_screenId = 4; FUN_0043CAF4(); }` opens the information panel, which is the
/// pop-up the shipped `Readme.txt` errata describes on an army.
#[test]
fn the_right_button_closes_a_panel_and_opens_the_map_information_screen() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    game.select(8);

    let screen = MapScreen::new();
    let (px, py) = (240, 240);
    assert!(screen.map_clip().contains(px, py), "that pixel is on the map");
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "the information panel, and it now knows what the click resolved to",
    );

    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));

    for panel in county::PANELS {
        for (x, y) in [(240, 240), (500, 195), (620, 470)] {
            let mut m = Machine::new(ScreenId::County(8, panel));
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.handle(Event::RightClick { x, y }, &mut ctx);
            assert_eq!(m.depth(), 0, "{panel:?} closed by a right click at ({x}, {y})");
        }
    }

    for id in [ScreenId::Village(8), ScreenId::Job(8, 0)] {
        let mut m = Machine::new(id);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 200, y: 200 }, &mut ctx);
        assert_eq!(m.depth(), 0, "{id:?} closed by a right click");
    }
}

#[test]
fn the_county_strip_shows_the_saves_numbers_where_the_original_puts_them() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.ration_achieved), (435, 72, 3));

    let lead = l2_game::shell::font::SPACE_ADVANCE;
    assert_eq!(lead, 4, "the sign column is four pixels wide");
    assert_eq!(
        find_strip(&canvas, &assets, "435", STRIP_INK),
        Some((0x1FC + lead, 189)),
        "the population, at Ui_DrawNumber(pop, ' ', …, 0x1FC, 0xBD) plus its sign column"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `Ui_DrawNumber` has no
        // only in their value and their x. `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "the happiness, displaced by the same four pixels and not by more"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "0%", STRIP_INK),
        Some((0x1FA + lead, 226)),
        "the tax rate at 0x1FA — its suffix is '%' and its lead is still a space"
    );
    // The county's name comes out of `L2.eng` group 100 at
    // `scenarioIndex * 20 + id` and is drawn in the **body** font, so it is
    // neither of the two the rest of the strip uses.
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 8);
    assert_eq!(name, "Dyfed", "L2.eng group 100, index map_slot * 20 + 8");
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_INK).is_some());
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_BAD).is_none());

    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "73", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "Double", STRIP_INK).is_none());
}


