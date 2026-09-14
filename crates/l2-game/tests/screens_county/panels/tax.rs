#![allow(unused_imports)]
use super::*;
use super::population::*;
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

/// Setting the tax rate changes the record *and* what is on the screen. Both
/// halves matter: a panel that showed a number it did not set, or set a number
/// it did not show, would pass one of them alone.
#[test]
fn setting_the_tax_rate_changes_the_county_and_the_picture() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let before = draw(&mut screen, &mut game, &assets);

    for _ in 0..7 {
        send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);

    let after = draw(&mut screen, &mut game, &assets);
    assert!(before.diff_count(&after) > 0);
    // `Ui_DrawNumber(taxRate, ' ', "%", 0x100, 0xA8, body, 0x3F)`.
    assert!(find_body(&after, &assets, "7%", font::TEXT).is_some());
    assert!(find_body(&after, &assets, "0%", font::TEXT).is_none());

    // Down moves to the ration panel, and Right there does not touch the tax.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    assert_eq!(screen.panel(), Panel::Ration);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);
    assert_eq!(game.kingdom.counties[8].ration_wanted, 4);
}

/// **The tax ceiling is 50, and it is the original's.**
///
/// `Tax_Increase` guards `taxRate < 0x32`; `g_taxHappinessOther` has exactly
/// 51 entries. `docs/screens-county.md` §6.3. The screen stops there and the
/// number on it stops there too — a clamp that the picture disagreed with
/// would be a clamp the player cannot see.
#[test]
fn the_tax_rate_stops_at_the_originals_own_ceiling_of_fifty() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let up = Panel::Tax.increase_button().expect("the tax panel has an up arrow");

    for _ in 0..60 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, MAX_TAX_RATE);
    assert_eq!(MAX_TAX_RATE, 50, "and the constant is the reading, not a round number");

    let canvas = draw(&mut screen, &mut game, &assets);
    assert!(find_body(&canvas, &assets, "50%", font::TEXT).is_some());
    assert!(find_body(&canvas, &assets, "51%", font::TEXT).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "60%", font::TEXT).is_none());
}

/// **The tax panel is the ration panel again**, and this is the same test one
/// door along: press the arrow, and require the panel to differ **outside the
/// arrows themselves**.
///
/// `Tax_IncreaseCounty` (`0x0043AA83`) is `taxRate++`, `Tax_RecomputePreview`,
/// `Panel_Tax()`. Ours wrote the rate and returned, so *"People pay"* kept the
/// zero it was born with and both happiness lines kept last season's.
///
/// Ablation, which was run: drop the `tax_shown` line from
/// `tax::recompute_preview` and this fails; drop the whole
/// `Kingdom::set_tax_rate` body back to a bare write and it fails harder.
#[test]
fn stepping_the_tax_rate_changes_the_panel_in_the_same_frame() {
    let (mut game, assets) = world!();
    let county = 8;
    assert_eq!(game.kingdom.counties[county].owner, game.player);
    assert_eq!(game.kingdom.counties[county].tax_rate, 0, "the fixture starts at nothing");

    let mut screen = CountyScreen::new(county as u8, Panel::Tax);
    let up = Panel::Tax.increase_button().expect("the tax panel has arrows");

    let before = draw(&mut screen, &mut game, &assets);
    for _ in 0..10 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    }
    let after = draw(&mut screen, &mut game, &assets);
    assert_eq!(game.kingdom.counties[county].tax_rate, 10);

    // Everything except the two arrows' own row, so the difference is a number
    // and not the control.
    let mut changed = 0usize;
    for y in 0..l2_view::canvas::HEIGHT {
        for x in 0..l2_view::canvas::WIDTH {
            let (xi, yi) = (x as i32, y as i32);
            let in_arrows = yi >= up.y - 4 && yi < up.y + up.h + 4 && xi >= up.x - 40;
            if !in_arrows && before.at(x, y) != after.at(x, y) {
                changed += 1;
            }
        }
    }
    assert!(
        changed > 0,
        "ten clicks of the tax arrow and nothing on the panel moved. \
         Tax_RecomputePreview writes taxShown, dHapTaxLocal and taxHapOther, and \
         Tax_IncreaseCounty calls it before it repaints. \
         docs/decisions.md C125.",
    );
    assert!(
        game.kingdom.counties[county].tax_shown > 0,
        "and 'People pay' is what should have moved first",
    );
}

/// **The same panel read off the canvas instead of diffed**, because the test
/// above cannot tell a live number from a constant.
///
/// `stepping_the_tax_rate_changes_the_panel_in_the_same_frame` asserts that
/// *something* outside the arrows moved and that the field is non-zero. Both
/// survive a painter that draws a literal `0` on the *People pay* row, because
/// the two happiness lines move on their own and the field it checks is not the
/// one the pixels came from. The player's report was about the screen, so the
/// assertion has to be about the screen.
///
/// Three claims, one panel:
///
/// * **The words are the player's own `L2.eng`.** `Panel_Tax` (`0x0041152F`)
///   fetches group 86 four times — indices 1, 2, 3 and 4 — and index 0,
///   *"Tax in"*, is drawn by nothing. The install's strings are mixed case and
///   [`county::…::g86`]'s fallbacks are upper case, so *"People pay"* on the
///   canvas and *"PEOPLE PAY"* absent is the whole of `CLAUDE.md` rule 6 for
///   this screen: an equality.
/// * **The *This county* line is right on the frame the game is loaded**, not
///   only after an arrow is pressed. `Tax_RecomputePreview` writes county
/// `+0x0F = 5 - taxRate` and the original's own saves store exactly that, so
///   a county at rate 0 reads `( +5 ☺ )`. Nothing imported `+0x0F`, so ours
///   read `( 0 ☺ )` until the player touched a control.
/// * **The *People pay* number is the arithmetic**, at a rate the fixture does
///   not start at: `Pct(Pct(435, 640), 20)` is **556**, and 640 is
///   `g_castleTaxBase[3]` written out.
///
/// **Ablation, run:** delete `c.d_hap_tax_local = *d_hap_tax_local;` from
/// `l2_scenario::Scenario::apply_counties` and the `+5` clause fails; replace
/// `line_text(ctx, g86::PEOPLE_PAY)` with `g86::PEOPLE_PAY.ours` and the
/// *"People pay"* clause fails; drop the `tax_shown` line from
/// `l2_kingdom::tax::recompute_preview` and the `556` clause fails.
#[test]
fn the_tax_panel_draws_the_originals_numbers_in_the_originals_words() {
    let (mut game, assets) = world!();
    let county = 8usize;
    // The fixture's own numbers, so the expected crown count below is a
    // statement about the arithmetic and not about the save.
    assert_eq!(game.kingdom.counties[county].owner, game.player);
    assert_eq!(game.kingdom.counties[county].population, 435);
    assert_eq!(game.kingdom.counties[county].castle_type, 3);
    assert_eq!(game.kingdom.counties[county].tax_rate, 0, "the fixture starts at nothing");

    let mut m = over_the_map(ScreenId::County(county as u8, Panel::Tax));
    let before = draw_stack(&mut m, &mut game, &assets);

    // The words are from the install.
    assert_eq!(assets.shell.text(86, 2), "People pay", "the install's own string");
    assert_eq!(
        find_body(&before, &assets, "People pay", font::TEXT).map(|p| p.0),
        Some(96),
        "Eng_DrawString(86, 2, 0x60, 0xC8, body)"
    );
    assert!(
        find_body(&before, &assets, "PEOPLE PAY", font::TEXT).is_none(),
        "our transcription is the fallback for an install with no L2.eng, not the panel"
    );
    assert!(
        find_body(&before, &assets, "Tax in", font::TEXT).is_none(),
        "86/0 has no call site in the whole corpus"
    );

    // --- This county, on the frame the game was loaded, before any control.
    // The canvas first and the field second, so the assertion that fails is the
    // one about the screen: the field is how it got there, not the claim.
    assert_eq!(
        find_body(&before, &assets, "+5", font::TEXT).map(|p| p.1),
        Some(232),
        "Ui_DrawHappinessDelta(realm+0x28 + county+0x0F) on the This county row. \
         county +0x0F is 5 - taxRate, the save stores 5 in all fourteen, and it \
         reached the county as {}",
        game.kingdom.counties[county].d_hap_tax_local
    );
    assert_eq!(game.kingdom.counties[county].d_hap_tax_local, 5);
    assert!(
        find_body(&before, &assets, "+4", font::TEXT).is_none(),
        "a near miss: the delta is five, not four"
    );

    // --- People pay, at a rate the fixture does not start at
    let up = Panel::Tax.increase_button().expect("the tax panel has arrows");
    for _ in 0..20 {
        send_stack(&mut m, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    }
    assert_eq!(game.kingdom.counties[county].tax_rate, 20);
    let after = draw_stack(&mut m, &mut game, &assets);

    // `Pct(Pct(population, g_castleTaxBase[castleType]), taxRate)`, with the
    // multiplier written out: nothing in this expression is read from the table
    // the assertion is about.
    let expected = 435 * 640 / 100 * 20 / 100;
    assert_eq!(expected, 556);
    assert_eq!(
        find_body(&after, &assets, &expected.to_string(), font::TEXT).map(|p| p.1),
        Some(200),
        "Ui_DrawCount(county+0xC0, …) on the People pay row"
    );
    assert!(
        find_body(&after, &assets, "557", font::TEXT).is_none(),
        "a near miss, so 556 cannot have been found in some other number"
    );

    // …and the happiness line followed it down. 5 - 20 is -15, and county 8's
    // realm holds one county, so the empire term is g_taxHappinessOther[20].
    assert!(
        find_body(&after, &assets, "+5", font::TEXT).is_none(),
        "the This county line did not stay where it was"
    );
    assert_eq!(
        find_body(&after, &assets, "-16", font::HIGHLIGHT).map(|p| p.1),
        Some(232),
        "-15 local and -1 empire, and a negative delta is drawn in the highlight pen"
    );
}

