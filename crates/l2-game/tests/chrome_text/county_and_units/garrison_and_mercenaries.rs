#![allow(unused_imports)]
use super::*;
use super::unit_panels::*;
use super::zoom_and_battle::*;
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

/// **The castle's garrison is `"@40 "`, and *"troops."* is chained off it, so
/// both moved.**
///
/// ```c
/// /* Screen_CastleBuildPanel, 0x004198AA */
/// Ui_DrawNumber(g_castleGarrisonCap[sel], '@', &DAT_004D4160, 0xC, 0xE2, &g_fontBody, 0x3F);
/// Eng_DrawString(0x47, 0xC, g_penAdvance + 0xE, 0xE2, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D4160` is one space. The old `Pen::number(…, true)` drew `"40 "`: the
/// space right by accident, the lead missing — digits **and** noun four left.
///
/// Ablated twice, each red on its own assertion:
/// * the old `pen.body(…, &format!("{cap} "))` restored at the call site — the
///   digits are found at **12** where 16 is expected, the defect as it shipped;
/// * suffix `" "` → `""` — the digits stay at 16 and the noun is found at **51**
///   where 55 is expected.
#[test]
fn the_castle_s_garrison_and_its_noun_both_start_one_sign_column_right() {
    use l2_game::screens::castle::CastleScreen;
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut screen = CastleScreen::new(county);
    let cap = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        l2_kingdom::industry::garrison_cap(&ctx.game.kingdom.tables, screen.castle_type(&ctx))
    };
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x0C; // Ui_DrawNumber(cap, '@', " ", 0xC, 0xE2, …)
    const Y: i32 = 0xE2;
    const NOUN_X: i32 = 0x0E; // Eng_DrawString(71, 0xC, g_penAdvance + 0xE, …)

    let digits = cap.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, Y),
        Some(X + LEAD),
        "the garrison's digits are not one sign column right of 0x0C"
    );
    let noun = assets.shell.text(71, 0x0C).to_string();
    assert!(!noun.is_empty(), "L2.eng 71/12 is the garrison's noun");
    assert_eq!(
        find_on_row(&canvas, body, &noun, font::TEXT, Y),
        Some(NOUN_X + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{noun:?} is not at g_penAdvance + 0xE after \"@{digits} \""
    );
}

/// **The mercenary's price line: both numbers moved and neither noun did.**
///
/// ```c
/// /* Screen_RaiseArmy, 0x00418653 */
/// g_penAdvance = 0;
/// Ui_DrawNumber(price, '@', &DAT_004D40C8, 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Eng_DrawString(0x45, 0, g_penAdvance + 0x70, base + 0x7C, &g_fontBody, 0x3F);
/// Ui_DrawNumber(men / 2, '@', &DAT_004D40CC, g_penAdvance + 0x70, …);
/// Eng_DrawString(0x45, 1, g_penAdvance + 0x70, …);
/// ```
///
/// Both suffixes are NUL. The old `"{n} "` lost four at the front and added four
/// at the back, so the nouns landed right and the digits did not —
/// the nouns are asserted as well: a fix that adds the lead and keeps the
/// invented space moves them.
///
/// Ablated twice, each red on its own assertion:
/// * the price put back to the old `pen.body(…, &format!("{} ", price))` — found
///   at **112** where 116 is expected;
/// * the price's suffix `""` → `" "` — the price stays at 116 and *"crowns to
///   hire."* is found at **164** where 160 is expected.
#[test]
fn the_mercenary_price_line_moves_its_numbers_and_not_its_nouns() {
    use l2_game::screens::army::{self, RaiseArmyScreen};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const BAND: u8 = 3;
    game.kingdom.counties[county as usize].mercenary_offer = BAND;
    let rules = &l2_kingdom::mercenary::ROSTER[BAND as usize];
    let mut screen = RaiseArmyScreen::new(county);
    let canvas = draw(&mut screen, &mut game, &assets);

    const X: i32 = 0x70;
    let y = army::base(true) + 0x7C;

    let price = rules.price.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &price, font::TEXT, y),
        Some(X + LEAD),
        "the price is not one sign column right of 0x70"
    );
    let hire = assets.shell.text(0x45, 0).to_string();
    let hire_x = X + LEAD + body.width(&price) + TRAILER;
    assert_eq!(
        find_on_row_from(&canvas, body, &hire, font::TEXT, y, X),
        Some(hire_x),
        "{hire:?} is not at g_penAdvance + 0x70 after \"@{price}\""
    );
    let wages = (rules.men / 2).to_string();
    let wages_x = hire_x + body.width(&hire) + TRAILER + LEAD;
    assert_eq!(
        find_on_row_from(&canvas, body, &wages, font::TEXT, y, hire_x),
        Some(wages_x),
        "the wages are not one sign column after {hire:?}"
    );
    let seasonal = assets.shell.text(0x45, 1).to_string();
    assert_eq!(
        find_on_row_from(&canvas, body, &seasonal, font::TEXT, y, wages_x),
        Some(wages_x + body.width(&wages) + TRAILER),
        "{seasonal:?} is not at g_penAdvance + 0x70 after \"@{wages}\""
    );
}

