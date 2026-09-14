#![allow(unused_imports)]
use super::*;
use super::garrison_and_mercenaries::*;
use super::unit_panels::*;
use super::foraging_and_shoot::*;
use super::*;
use super::top_bar::*;
use super::panels::*;
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

/// **The box at the far zoom was empty, and the original fills it.**
///
/// `Screen_DrawCampaign`'s (`0x0040F5FD`) zoom-2 arm, whole:
///
/// ```c
/// Ui_DrawBox(0, 0x19C, 0x1E, 4);
/// DAT_0058FE2C = 1;  g_penAdvance = 0;
/// Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
/// Eng_DrawString(0x22, 0,  g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
/// Ui_DrawYear(g_year,      g_penAdvance + 0x60, 0x1A8, 1);
/// DAT_0058FE2C = 0;
/// Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
/// ```
///
/// We drew the box and, inside it, a status line of our own — which C173 then
/// gated behind the debug overlay, leaving the box blank. Group 34 has exactly
/// one consumer in the binary and it is this arm, so its two strings *are* this
/// box's vocabulary (`CLAUDE.md` rule 6), and the second of them is the game
/// saying what the far zoom is for.
///
/// **Ablated, one draw at a time:** removing any of the four turns exactly one
/// of the assertions below red.
#[test]
fn the_far_zoom_box_carries_the_map_name_the_year_and_the_instruction() {
    let (mut game, assets) = world!();
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install");
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let mut screen = MapScreen::new();
    // `Map_ToggleZoom` — the box exists only at zoom 2.
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::KeyDown(Key::Char('Z')), &mut ctx);
    }
    let canvas = draw(&mut screen, &mut game, &assets);

    // The three heading draws run with `DAT_0058FE2C` set: capitals come out in
    // colour 1 whatever the caller passed,
    // mode as well as the face.
    let caps = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: Some(1) };

    let name = assets.shell.text(101, game.map_slot).to_string();
    assert!(!name.is_empty(), "L2.eng group 101 has the sixty map names");
    let (nx, ny) = find_styled(&canvas, heading, &name, &caps)
        .unwrap_or_else(|| panic!("{name:?} is not in the far-zoom box"));
    assert_eq!((nx, ny), (0x40, 0x1A8), "Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8)");

    let label = assets.shell.text(34, 0).to_string();
    assert_eq!(label, "Year", "L2.eng 34/0");
    let (lx, ly) = find_styled(&canvas, heading, &label, &caps).expect("{label:?} is not drawn");
    assert_eq!(ly, 0x1A8, "the label shares the map name's row");
    assert!(lx > nx + heading.width(&name), "the label is at {lx}, not after the name");

    // `Ui_DrawYear(…, 1)` puts group 26's era *before* the digits, both in the
    // heading face, and lifts an AD year by one pixel — `year < 0 ? y : y - 1`.
    // Finding the digits is the claim that the year was drawn.
    let digits = format!(" {} ", game.kingdom.year);
    let (yx, yy) = find_styled(&canvas, heading, &digits, &caps).expect("the year is not drawn");
    assert_eq!(yy, 0x1A8 - 1, "an AD year sits one pixel above its row");
    assert!(yx > lx, "the year is at {yx} and the label at {lx}");

    // And the instruction, in the body face with the drop capitals off again.
    let advice = assets.shell.text(34, 1).to_string();
    assert!(!advice.is_empty(), "L2.eng 34/1");
    let flat = Style { colour: font::TEXT, shadow: Some(font::SHADOW), caps: None };
    assert_eq!(
        find_styled(&canvas, body, &advice, &flat),
        Some((0x50, 0x1C6)),
        "Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F): {advice:?}"
    );
}

/// **`Screen_BattleMasterRatings` (`0x00421707`) draws each score in
/// `&g_fontHeading`** — the one heading-face figure on the page — and we drew it
/// in body. The lead and suffix, `' '` and one space, were already right.
///
/// ```c
/// Ui_DrawText(&g_playerNames[p], 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Eng_DrawString(0x25, 4, g_penAdvance + 0xD8, 0x6E, &g_fontBody, 0x3F);
/// Ui_DrawNumber(score, ' ', &DAT_004D43B4, g_penAdvance + 0xEC, 0x69, &g_fontHeading, 0x3F);
/// ```
///
/// The name is `g_playerNames[realm]`, which this test writes so the pen
/// advance in front of the score is a known width; it is measured, not assumed.
///
/// Ablated: `Face::Heading` → `Face::Body` at the call site — the score is not
/// found on row `0x69` in the heading face (`None` where `Some(422)` is expected).
#[test]
fn the_battle_master_score_is_in_the_heading_face() {
    use l2_game::screens::ratings::{self, Ratings};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    // The block is keyed by `g_localPlayer`'s realm and draws his name, so the
    // advance in front of the score is that name's width.
    game.player_names[Ratings::default().realms.0 as usize] =
        l2_game::text::PlayerName::new("PLAYER 1");
    let mut screen = ratings::RatingsScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    const NAME_X: i32 = 0xD8;
    const SCORE_X: i32 = 0xEC;
    const SCORE_Y: i32 = 0x69;
    let (mine, _) = ratings::score(&Ratings::default());
    let scored = assets.shell.text(0x25, 4).to_string();
    let advance = body.width("PLAYER 1") + TRAILER + body.width(&scored) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, heading, &mine.to_string(), font::TEXT, SCORE_Y, NAME_X),
        Some(SCORE_X + advance + LEAD),
        "the score is not in the heading face at g_penAdvance + 0xEC, 0x69"
    );
}

