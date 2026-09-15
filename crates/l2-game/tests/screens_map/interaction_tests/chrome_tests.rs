#![allow(unused_imports)]
use super::*;
use super::view_and_navigation_tests::*;
use super::selection_and_click_tests::*;
use super::town_and_mine_tests::*;
use super::*;
use super::view_tests::*;
use super::structures_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

#[test]
fn the_map_chrome_shows_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    let clock = find_body(&canvas, &assets, " 1268 ", font::TEXT).expect("the year");
    assert_eq!(clock, (360, 6), "the original draws the year at x 360, y 6");
    let season = find_body(&canvas, &assets, "Winter", font::TEXT).expect("the season");
    assert!(season.0 > clock.0, "the season follows the year, at g_penAdvance + 0x16C");
    // `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)` — the number and then
    // `L2.eng` group 8's *"Crowns."*. `GOLD` was a word of ours.
    let gold = find_body(&canvas, &assets, "1000 ", font::TEXT).expect("the treasury");
    assert_eq!(gold.1, 6, "and the treasury on the same row");
    assert!(find_body(&canvas, &assets, "Crowns.", font::TEXT).is_some(), "with its noun");
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_none());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_none());

    let lead = l2_game::shell::font::SPACE_ADVANCE;
    let pop = find_strip(&canvas, &assets, "435", STRIP_INK).expect("the population");
    assert_eq!(pop, (0x1FC + lead, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "and the happiness beside it"
    );

    assert!(find_body(&canvas, &assets, " 1269 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "1001 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "Summer", font::TEXT).is_none());
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
}

// ---------------------------------------------------------------------------
// **Pixels, asserted.** C59.


