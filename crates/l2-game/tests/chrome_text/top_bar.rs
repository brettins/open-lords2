#![allow(unused_imports)]
use super::*;
use super::panels::*;
use super::county_and_units::*;
use super::png_part::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

/// **`Screen_DrawMenuBar` (`0x00419C78`) draws the year, the season and the
/// treasury in `g_fontBody`, and we drew all three in our 5 × 7 debug font.**
///
/// A player reported it as *"still placeholder font in the top right for gold
/// and summer"*, and he was reading the two `l2_view::text::draw` calls that
/// used to be at the tail of `draw_menu_bar`.
///
/// The probe is rendered with `Fntl2_14.pl8` itself, so this test cannot pass
/// while the debug font is drawing them — which is the one thing the previous
/// suite could not tell apart.
#[test]
fn the_clock_and_the_treasury_are_drawn_in_the_game_s_body_font() {
    let (mut game, assets) = world!();
    let body = assets
        .shell
        .body
        .as_ref()
        .expect("Fntl2_14.pl8 is in the install");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let year = format!(" {} ", game.kingdom.year);
    assert!(
        find_in(&canvas, body, &year, font::TEXT).is_some(),
        "the year is not on the bar in Fntl2_14.pl8"
    );

    let season = l2_game::screens::map::season_text(&assets, game.kingdom.season);
    assert!(!season.is_empty(), "L2.eng group 29 has the season names");
    assert!(
        find_in(&canvas, body, &season, font::TEXT).is_some(),
        "{season:?} is not on the bar in Fntl2_14.pl8"
    );
}

/// **The order is the original's: the year first, then the season.**
///
/// `Screen_DrawMenuBar` draws `Ui_DrawYear(g_year, 0x168, …)` and then puts the
/// season at `g_penAdvance + 0x16C` — *after* it. We drew `"{season} {year}"`,
/// which is the other way round.
///
/// Ablated by swapping the two draw calls in `draw_menu_bar`: the season then
/// lands left of the year and this goes red.
#[test]
fn the_year_comes_before_the_season() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let year = format!(" {} ", game.kingdom.year);
    let season = l2_game::screens::map::season_text(&assets, game.kingdom.season);
    let (yx, _) = find_in(&canvas, body, &year, font::TEXT).expect("the year");
    let (sx, _) = find_in(&canvas, body, &season, font::TEXT).expect("the season");
    assert!(yx < sx, "the year is at x {yx} and the season at x {sx}");
}

/// **The treasury is `Ui_DrawCount(gold, 0, …)` — a number and then `L2.eng`
/// group 8's *"Crown."* / *"Crowns."*.** We drew `"GOLD 1234"`, which is a word
/// the game does not have.
#[test]
fn the_treasury_names_crowns_out_of_the_game_s_own_strings() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    let gold = game.gold();
    let noun = assets
        .shell
        .text(
            l2_game::shell::COUNT_NOUN_GROUP,
            l2_game::shell::count_noun(gold, 0),
        )
        .to_string();
    assert!(
        noun.starts_with("Crown"),
        "group 8/0-1 is Crown./Crowns., not {noun:?}"
    );
    assert!(
        find_in(&canvas, body, &noun, font::TEXT).is_some(),
        "{noun:?} is not on the bar in Fntl2_14.pl8"
    );
    // And the *word we invented* is gone. `GOLD` is not in `L2.eng` at all, so
    // this is the half of the assertion that catches a fix which draws both —
    // and it is looked for in the font it used to be drawn in, at the ink the
    // old line passed.
    assert!(
        debug_font_absent(&canvas, "GOLD", assets.ink.text),
        "the placeholder caption GOLD is still drawn in the 5 x 7 debug font"
    );
}

/// Where `s`, drawn in `f` at `colour`, starts **on the row the painter
/// passed** — or `None`.
///
/// A whole-canvas search returns the first match in raster order, which for a
/// short number is often a digit inside some *other* number higher up the
/// screen. Pinning the row turns "is this string anywhere" into "is this string
/// at this call site", which is the claim a position test makes.
fn find_on_row(canvas: &Canvas, f: &Font, s: &str, colour: u8, y: i32) -> Option<i32> {
    let style = Style { colour, shadow: None, caps: None };
    let w = f.width(s).max(1);
    let h = f.height(s).max(1) + 2;
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 1, s, &style);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|py| (0..w).map(move |px| (px, py)))
        .map(|(px, py)| (px, py, probe.at(px as usize, py as usize)))
        .filter(|&(_, _, c)| c != 0)
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let oy = y - 1;
    'next: for ox in 0..=(canvas.width as i32 - w) {
        for &(px, py, c) in &wanted {
            if canvas.at((ox + px) as usize, (oy + py) as usize) != c {
                continue 'next;
            }
        }
        return Some(ox);
    }
    None
}

/// **`Ui_DrawCount` opens the treasury with `'@'`, so its digits start four
/// pixels right of the call site's `x`, and we drew them at the `x`.**
///
/// `Screen_DrawMenuBar` (`0x00419C78`) calls
/// `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)`, and `Ui_DrawCount`
/// (`0x0041AB67`) is `Ui_DrawNumber(value, '@', &DAT_004D41F4, x, y, …)` with
/// `DAT_004D41F4` a NUL. `Ui_DrawText` (`0x00402637`) advances **4** over the
/// glyph-less `'@'` (`local_14 = 4`) and **4** more after the string
/// (`g_penAdvance + 4`), and `Ui_DrawCount` puts the noun at `x + g_penAdvance`.
///
/// **Every expected number is a literal from those functions, not a constant of
/// ours** — the lead's 4, the trailer's 4, 500 and 6 — so ablating `Pen`'s
/// constants cannot move the expectation with the code. Only the digits' own
/// width is measured, and it is measured in the face the call site names.
///
/// **Two assertions because the old code was wrong in two opposite ways that
/// cancelled at the noun.** `blank_lead: true` dropped the lead and invented a
/// trailing space: digits four left, noun right by coincidence. So the digits'
/// x is what catches that; the noun's x catches a fix that adds the lead and
/// keeps the invented space.
///
/// Ablated twice, each red on its own assertion:
/// * `Pen::count_with_noun`'s number put back to the old
///   `format!("{value} ")` — no lead, invented space: the digits are found at
///   **500** where 504 is expected, which is the defect as it shipped.
/// * `COUNT_SUFFIX` `""` → `" "`: the digits stay at 504 and the noun is found
///   at **551** where 547 is expected.
#[test]
fn the_treasury_s_digits_start_one_sign_column_right_of_ui_drawcount_s_x() {
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 500; // Ui_DrawCount(gold, 0, 500, 6, …)
    const Y: i32 = 6;
    const LEAD: i32 = 4; // Ui_DrawText: glyph-less '@' -> local_14 = 4
    const TRAILER: i32 = 4; // Ui_DrawText's last line: g_penAdvance + 4

    let gold = game.gold();
    let digits = gold.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, Y),
        Some(X + LEAD),
        "the treasury's digits are not one sign column right of Ui_DrawCount's x"
    );

    let noun = assets
        .shell
        .text(l2_game::shell::COUNT_NOUN_GROUP, l2_game::shell::count_noun(gold, 0))
        .to_string();
    assert_eq!(
        find_on_row(&canvas, body, &noun, font::TEXT, Y),
        Some(X + LEAD + body.width(&digits) + TRAILER),
        "{noun:?} is not at x + g_penAdvance after \"@{digits}\""
    );
}

/// **`Court_Draw` (`0x00416925`) draws all four store values in
/// `&g_fontHeading`, and we drew them in the body face** — beside labels that
/// were already in heading, so the column read as two different typefaces.
///
/// ```c
/// Ui_DrawCount (g_realms[p].gold,     0, 0xE0, 0x72, &g_fontHeading, 0x3F);
/// Ui_DrawNumber(g_realms[p].iron, ' ', " ", 0xE0, 0x90, &g_fontHeading, 0x3F);
/// ```
///
/// The probe is rendered in the heading face and searched **on the painter's
/// own row**, so finding it is the claim *"this face, at this call site"*.
/// Both leads advance four — `'@'` for the count, `' '` for the number — which
/// is why both expectations are `0xE0 + 4`.
///
/// Ablated by passing `Face::Body` for the two values in `court.rs`: neither is
/// found on its row in the heading face.
#[test]
fn the_court_s_stores_are_drawn_in_the_heading_face_at_court_draw_s_x() {
    use l2_game::screens::court::CourtScreen;
    let (mut game, assets) = world!();
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let (gold, iron) = {
        let r = &game.kingdom.realms[game.player as usize];
        (r.gold, r.iron)
    };
    let mut screen = CourtScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0xE0; // Court_Draw's value column
    const LEAD: i32 = 4; // '@' and ' ' both: Ui_DrawText's local_14 = 4

    assert_eq!(
        find_on_row(&canvas, heading, &gold.to_string(), font::TEXT, 0x72),
        Some(X + LEAD),
        "the court's gold is not in the heading face at (0xE0, 0x72)"
    );
    assert_eq!(
        find_on_row(&canvas, heading, &iron.to_string(), font::TEXT, 0x90),
        Some(X + LEAD),
        "the court's iron is not in the heading face at (0xE0, 0x90)"
    );
}

