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

/// The menu bar reads the clock and the treasury out of the world, and the
/// right column reads the selected county. Checked by finding the actual
/// digits, at the coordinates `Screen_DrawMenuBar` puts them.
#[test]
fn the_map_chrome_shows_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    // **The year, then the season, in `g_fontBody` — not our 5 × 7 font, and
    // not in that order.** This used to look for `"WINTER 1268"` in `ink.text`,
    // which is what the two `l2_view::text::draw` calls at the tail of
    // `draw_menu_bar` drew and what a player called *"still placeholder font in
    // the top right for gold and summer"*. `Screen_DrawMenuBar` draws
    // `Ui_DrawYear(g_year, 0x168, 6, 3)` and puts the season at
    // `g_penAdvance + 0x16C`, both in `&g_fontBody` at colour `0x3F`.
    let clock = find_body(&canvas, &assets, " 1268 ", font::TEXT).expect("the year");
    assert_eq!(clock, (360, 6), "the original draws the year at x 360, y 6");
    let season = find_body(&canvas, &assets, "Winter", font::TEXT).expect("the season");
    assert!(season.0 > clock.0, "the season follows the year, at g_penAdvance + 0x16C");
    // `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)` — the number and then
    // `L2.eng` group 8's *"Crowns."*. `GOLD` was a word of ours.
    let gold = find_body(&canvas, &assets, "1000 ", font::TEXT).expect("the treasury");
    assert_eq!(gold.1, 6, "and the treasury on the same row");
    assert!(find_body(&canvas, &assets, "Crowns.", font::TEXT).is_some(), "with its noun");
    // `TURN n` and `COUNTIES n/m` are ours and are debug overlay now: a normal
    // session draws neither (`the_debug_overlay_is_off_by_default_…`).
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_none());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_none());

    // **The county strip, in the map's own sidebar.** `Screen_DrawCampaign`
    // calls `CountyStrip_Draw` — the map screen used to leave that plate empty
    // and write a box of our own numbers over the jobs plate below it.
    // `Ui_DrawNumber`'s `x` is the *string's* origin and the string opens with
    // the sign column, so the digits sit one `SPACE_ADVANCE` right of the call
    // site's literal. The strip's own test carries the whole argument.
    let lead = l2_game::shell::font::SPACE_ADVANCE;
    let pop = find_strip(&canvas, &assets, "435", STRIP_INK).expect("the population");
    assert_eq!(pop, (0x1FC + lead, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "and the happiness beside it"
    );

    // The near-misses. If the search could match anything it would match these.
    assert!(find_body(&canvas, &assets, " 1269 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "1001 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "Summer", font::TEXT).is_none());
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
}

// ---------------------------------------------------------------------------
// **Pixels, asserted.** C59.
//
// Three visual features have now been reported missing by a human *after* being
// merged — the town flag, the merchant sprite, the minimap tint. Two of the
// three did have a pixel test:
// [`the_county_town_flies_its_owners_flag_and_it_waves`] and
// [`a_merchant_is_drawn_and_opens_the_merchant_screen_from_the_county_it_is_in`]
// both diff two canvases and assert the ink landed in the right box. **The
// minimap's tint had none**, and it is the one the player was still describing
// as wrong.
//
// Both tests below are stronger than a diff, in the same way: a diff says
// *something* changed inside a rectangle, so it passes on a garbage sprite or
// on the wrong frame of the right sheet. These match the **artwork itself** —
// the frame's own palette indices, several hundred of them, standing where the
// blit put them — and then vary one field of the save and require the picture
// to follow it. That is what makes them able to catch a flag that draws, but
// draws the wrong realm's.
//
// The house technique is to turn the visual claim into a number the file can
// settle and then assert the number. Two precedents: *a tick cannot come 4th of
// 84 frames by ink*, and *a two-pixel ring is four pixels of width*.
// ---------------------------------------------------------------------------


