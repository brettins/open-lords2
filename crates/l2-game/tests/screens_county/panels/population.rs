#![allow(unused_imports)]
use super::*;
use super::tax::*;
use super::rations::*;
use super::turns_and_sidebar::*;
use super::*;
use super::strip_and_sidebar::*;
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

/// The population panel is `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)` with its rows
/// at the y coordinates `Panel_Population` draws them, and it is reached from
/// the strip's top-left quadrant and from nowhere else.
#[test]
fn the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should() {
    let (mut game, assets) = world!();
    let ink = &assets.ink;

    // **Through the machine, with the campaign map underneath**, because that
    // is where `CountyStrip_Click` lives: `Screen_FrameInput`'s arm for `0x14`
    // and its three siblings runs six guards belonging to the map's right-hand
    // column *before* any verb of the panel's own, and the strip's 2 × 2
    // hotspot is the third of them. The panel returns `Transition::Pass` for
    // the whole column and the map answers.
    //
    // This used to send the click straight to a bare `CountyScreen`, which
    // could not tell "the panel switched" from "the panel swallowed the whole
    // sidebar" — and it swallowed it. `docs/arms.json`
    // `0x0042FF10/inset-runs-the-sidebar-guards`.
    game.select(8);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::County(8, Panel::Tax));
    let hot = Panel::Population.strip_hotspot();
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: hot.centre_x(), y: hot.y + 4 }, &mut ctx);
    }
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Population)),
        "the strip switched panel rather than closing or stacking one"
    );
    assert_eq!(m.depth(), 2, "and it landed at the map's depth, not on top of the tax panel");

    // **The panel is drawn in the game's own fonts and from the game's own
    // `L2.eng`** — so the strings here are the file's words, lower case and
    // All; the search is [`find_body`]/[`find_heading`].
    // 5 x 7 probe. It used to be our own transcriptions in our own font.
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        find_heading(&canvas, &assets, "Last season", font::TEXT),
        Some((48, 266)),
        "group 73 index 1, at Eng_DrawString(0x49, 1, 0x30, 0x10A)"
    );
    assert_eq!(
        find_heading(&canvas, &assets, "417", font::TEXT),
        Some((336, 266)),
        "and its value **left**-aligned from 0x150 — Ui_DrawNumber does not measure"
    );
    assert_eq!(find_body(&canvas, &assets, "Births", font::TEXT), Some((48, 298)), "0x12A");
    assert_eq!(find_body(&canvas, &assets, "Deaths", font::TEXT), Some((48, 314)), "0x13A");
    assert_eq!(find_body(&canvas, &assets, "Army", font::TEXT), Some((48, 330)), "0x14A");
    assert_eq!(
        find_heading(&canvas, &assets, "This Season", font::TEXT),
        Some((48, 386)),
        "0x182"
    );
    assert_eq!(
        find_heading(&canvas, &assets, "435", font::TEXT),
        Some((336, 386)),
        "this season's population, left-aligned from 0x150"
    );
    // `Eng_DrawString(100, slot * 0x14 + county, pen + 0x16, 0x38, heading)` —
    // the county's own name after the title, which the panel used to omit.
    assert!(
        find_heading(&canvas, &assets, "Population in", font::TEXT).is_some(),
        "group 73 index 0 at (20, 56)"
    );

    // The graph is a stub, because g_countyHistory is not simulated. Its two
    // lines are **ours** and are debug overlay only, so by default the recess
    // is empty.
    assert!(find_text(&canvas, "NOT SIMULATED", ink.bad).is_none());

    // And the tax panel is gone.
    assert!(find_body(&canvas, &assets, "Tax rate", font::TEXT).is_none());
}

/// A zero row draws **no number at all** — `Ui_DrawDelta(value, 0, ...)`
/// returns before it formats anything. This is the detail a reimplementation
/// gets wrong by printing `0`, so it is asserted both ways round.
#[test]
fn a_zero_delta_row_draws_its_label_and_no_number() {
    let (mut game, assets) = world!();
    let ink = &assets.ink;
    let mut screen = CountyScreen::new(8, Panel::Tax);
    screen.open(Panel::Population);

    game.kingdom.counties[8].births = 0;
    let blank = draw(&mut screen, &mut game, &assets);
    assert!(
        find_body(&blank, &assets, "Births", font::TEXT).is_some(),
        "the label is still drawn"
    );
    assert!(find_body(&blank, &assets, "+0", font::TEXT).is_none(), "and nothing beside it");

    game.kingdom.counties[8].births = 63;
    let filled = draw(&mut screen, &mut game, &assets);

    // **The strong form: the two frames differ only inside the births row.**
    // A bare "0" is no longer a usable near-miss, because every line on the
    // panel is now drawn in the one colour the painter passes (0x3F) and some
    // other number on it contains the digit. This says the same thing without
    // depending on what else is on the page: turning births from 0 to 63 puts
    // ink in the births row and nowhere else, so the blank frame's own value
    // cell was empty.
    let band = 294..312;
    let outside: usize = (0..filled.height)
        .filter(|y| !band.contains(&(*y as i32)))
        .map(|y| {
            (0..filled.width).filter(|x| blank.at(*x, y) != filled.at(*x, y)).count()
        })
        .sum();
    assert_eq!(outside, 0, "births changed something outside its own row");
    assert!(
        (0..filled.width)
            .any(|x| band.clone().any(|y| blank.at(x, y as usize) != filled.at(x, y as usize))),
        "and it did change its own row"
    );

    // **Left-aligned from 0x150, plus the four pixels `Ui_DrawText` advances
    // for `Ui_DrawDelta`'s empty prefix.** It used to be right-anchored *to*
    // 336, which is what `docs/screens-county.md` §5.1 says and neither
    // `Ui_DrawNumber` nor `Ui_DrawDelta` does.
    assert_eq!(
        find_body(&filled, &assets, "+63", font::TEXT),
        Some((340, 298)),
        "a non-zero row draws a signed number in the value column"
    );
    assert!(blank.diff_count(&filled) > 0, "and the two frames differ");
    let _ = ink;
}

