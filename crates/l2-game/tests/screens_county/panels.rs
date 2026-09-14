#![allow(unused_imports)]
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

/// The ration panel's slider is the third order the original's county panels
/// give, and the one we never had: `Ration_SliderClick` jumps the split to
/// `mouseX - 224`, and the two caps step it by one.
#[test]
fn the_ration_split_slider_sets_the_field_the_original_sets() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    screen.open(Panel::Ration);

    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 37, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 37, "the track jumps to mouseX - 224");

    let down = county::split_down_button();
    send(&mut screen, &mut game, &assets, Event::Click { x: down.centre_x(), y: down.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 36, "the left cap steps down one");

    let up = county::split_up_button();
    for _ in 0..3 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 8 });
    }
    assert_eq!(game.kingdom.counties[8].ration_split, 39);

    // The knob follows: two different splits must not draw the same picture.
    let a = draw(&mut screen, &mut game, &assets);
    game.kingdom.counties[8].ration_split = 90;
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 0, "the knob moves with the value");
}

/// **The slider's effect has to be visible in the same frame, and it was not.**
///
/// A player reported *"rations slider moves but is inoperable"*. It was writing
/// `ration_split` correctly the whole time — a test three functions up asserted
/// exactly that and passed — and every number on the panel stayed where it was
/// until the turn ended, because `Game::set_ration_split` wrote the field and
/// stopped. `Ration_SetSplit` re-runs the food pass, reallocates the county
/// twice and repaints.
///
/// So this asserts the *effect* and not the fixture: draw the panel, move the
/// slider, draw it again, and require pixels to differ **outside the slider's
/// own rectangle**. Masking the slider out is the whole point — a thumb that
/// moves is what the player could already see, and it is not evidence of
/// anything.
///
/// Ablation, which was run: replace `Kingdom::set_ration_split`'s body with the
/// old one-line write and this fails with zero pixels changed.
#[test]
fn moving_the_ration_slider_changes_a_number_on_the_panel_in_the_same_frame() {
    let (mut game, assets) = world!();
    let county = 8; // the player's, in the England turn-one fixture
    assert_eq!(game.kingdom.counties[county].owner, game.player);

    // **The fixture's own county cannot demonstrate this.
    // rule worth knowing.** County 8 at England turn one has 435 people and
    // 101 head; the standing herd feeds five people a head *without being
    // slaughtered*, so 505 mouths' worth of dairy covers 435 and the county eats
    // nothing at all. `herd_eaten` and `grain_eaten` are 0 at **every** split,
    // so the slider has nothing to divide and the panel is inert — in the
    // original as much as here. That is very likely what the player was looking
    // At. `docs/rules.md` explains this.
    //
    // So the herd is cut to something the county has to eat *around*. The state
    // is the input and the screen is the subject; asserting on a county whose
    // numbers cannot move would be a test that passes for the wrong reason,
    // which is the whole family `docs/agents.md` catalogues.
    game.kingdom.counties[county].herd = 20;
    game.kingdom.counties[county].grain = 400;

    let mut screen = CountyScreen::new(county as u8, Panel::Ration);
    let track = county::split_track();

    // Start at one end so the move is as large as the control allows.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x, y: track.y + 8 });
    let before = draw(&mut screen, &mut game, &assets);
    let split_before = game.kingdom.counties[county].ration_split;

    // A drag: the button goes down on the track and the pointer walks to the
    // far end. `Ration_SliderClick` fires on held-and-moved, not on the click.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 4, y: track.y + 8 });
    for step in (4..=track.w).step_by(8) {
        send(
            &mut screen,
            &mut game,
            &assets,
            Event::Pointer { x: track.x + step, y: track.y + 8 },
        );
    }
    send(&mut screen, &mut game, &assets, Event::Release { x: track.x + track.w, y: track.y + 8 });
    let after = draw(&mut screen, &mut game, &assets);

    assert_ne!(
        game.kingdom.counties[county].ration_split, split_before,
        "the drag did not reach the field at all",
    );

    // Everything except the slider's own row. `split_track` is the track; the
    // two caps sit either side of it on the same row, so the mask is the whole
    // band.
    let masked = |c: &l2_view::Canvas, other: &l2_view::Canvas| {
        let mut n = 0usize;
        for y in 0..l2_view::canvas::HEIGHT {
            for x in 0..l2_view::canvas::WIDTH {
                let (xi, yi) = (x as i32, y as i32);
                let in_slider = yi >= track.y - 8
                    && yi < track.y + track.h + 8
                    && xi >= track.x - 64
                    && xi < track.x + track.w + 64;
                if !in_slider && c.at(x, y) != other.at(x, y) {
                    n += 1;
                }
            }
        }
        n
    };
    let changed = masked(&before, &after);
    assert!(
        changed > 0,
        "the slider moved and nothing else on the panel did. The thumb is not the \
         effect: Ration_SetSplit runs the food pass on the spot, so `Eaten`, `Achieved` \
         and the happiness deltas move with it. docs/decisions.md C118.",
    );
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

/// County 1 belongs to realm 5. The strip says so, all four panels still open,
/// and every order is refused — by the mouse as well as by the keyboard.
#[test]
fn another_realms_county_can_be_looked_at_and_not_ordered() {
    let (mut game, assets) = world!();
    assert_eq!(game.kingdom.counties[1].owner, 5);

    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    // The name at (480, 180)
    // 165, then group 15's two lines and the owner at 240 / 260 / 280 — all
    // four in the body font and in the owning realm's own colour.
    // **The pen is the realm's shield colour**, `g_realmColour[shield]`, which
    // is what `CountyStrip_Draw` passes for all three lines. Not `Ink::realm`
    // and not the realm id — see `sovereign_lines` below and C62.
    let shield = game.kingdom.realms[5].shield_index;
    let realm5 = l2_view::chrome::realm_pen(shield).expect("realm 5 flies a shield");
    // **The lord's real name, out of the save.** This read `"REALM 5"` while
    // nothing filled `g_playerNames`; the save's own player table does now, so
    // the line says what the original's says. The near miss is another realm's
    // Lord.
    let lord5 = game.player_names[5].as_str().to_owned();
    let lord4 = game.player_names[4].as_str().to_owned();
    assert!(!lord5.is_empty() && lord5 != lord4, "the fixture names its lords: {lord5:?}");
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        find_body(&canvas, &assets, &name, STRIP_INK).map(|p| p.1),
        Some(180),
        "the name sits lower on the 162 x 274 plate"
    );
    // **The third line is the lord, out of the game's own two sources.** It
    // read `REALM 5` — ours, invented — until the strip was put on
    // `message::lord_name` with the court and the battle prompt: the save's
    // player table (`Game::player_names`) first, then `L2.eng` group 7 by the
    // realm's **lord**, which is the pair `Game_NewGame` seeds the array from.
    // This fixture's save names its lords, so here it is the **table** that
    // answers — which is what the equality below pins, and it is the seam
    // between the shared accessor and the table that now fills it.
    let lord = l2_game::screens::message::lord_name(&Ctx { game: &mut game, assets: &assets }, 5);
    assert_eq!(lord, lord5, "the shared accessor answers realm 5 out of the save's table");
    assert!(!lord.starts_with("REALM "), "realm 5's lord is named, and it said {lord:?}");
    assert!(find_body(&canvas, &assets, &lord, realm5).is_some(), "the strip names {lord:?}");
    assert!(find_body(&canvas, &assets, &lord4, realm5).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "REALM 5", realm5).is_none(), "and not a name of ours");
    assert!(find_body(&canvas, &assets, "693", STRIP_INK).is_none(), "no numbers at all");

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    let up = Panel::Tax.increase_button().expect("the arrows are still drawn");
    send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "no order lands on another realm's county");

    screen.open(Panel::Ration);
    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 40, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[1].ration_split, 0, "and neither does the slider");
}

/// End turn, from the map, with the mouse — and the numbers move on screen.
///
/// This is the whole slice in one test: a England turn-one scenario, a real map, a click
/// on a button, `l2-kingdom`'s season pipeline, and the changed numbers read
/// back off the canvas.
#[test]
fn ending_the_turn_from_the_map_moves_the_numbers_and_the_screen_follows() {
    let (mut game, assets) = world!();
    // `TURN n` is our counter and debug overlay only; it is how this test reads
    // the turn off the picture.
    game.prefs.debug_overlay = true;
    let mut screen = MapScreen::new();
    game.select(8);
    let before = draw(&mut screen, &mut game, &assets);
    // The year and the season, in the face `Screen_DrawMenuBar` passes.
    assert!(find_body(&before, &assets, " 1268 ", font::TEXT).is_some());
    assert!(find_body(&before, &assets, "Winter", font::TEXT).is_some());
    assert!(find_text(&before, "TURN 1", assets.ink.dim).is_some());

    let population = game.kingdom.counties[8].population;
    let gold = game.gold();

    let b = map::END_TURN_BUTTON;
    send(&mut screen, &mut game, &assets, Event::Click { x: b.centre_x(), y: b.y + 4 });
    assert_eq!(
        game.kingdom.turn_count, 1,
        "the click starts the turn; it does not run it inside the click",
    );
    run_turn(&mut screen, &mut game, &assets);

    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(game.kingdom.season, 1, "Winter gave way to Spring");
    assert_ne!(game.kingdom.counties[8].population, population, "the county changed");
    assert_eq!(game.kingdom.counties[8].pop_last, population);
    assert_eq!(game.gold_last[game.player as usize], gold, "and the treasury is remembered");

    let after = draw(&mut screen, &mut game, &assets);
    assert!(find_body(&after, &assets, "Spring", font::TEXT).is_some(), "the clock moved");
    assert!(find_text(&after, "TURN 2", assets.ink.dim).is_some());
    assert!(find_body(&after, &assets, "Winter", font::TEXT).is_none());
    assert!(before.diff_count(&after) > 0);

    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&after, &assets, &shown, STRIP_INK).is_some(),
        "the right column shows the new population, {shown}"
    );
    assert!(
        find_strip(&after, &assets, &population.to_string(), STRIP_INK).is_none(),
        "and not the old one"
    );
}

/// The turn also runs through the machine, from the keyboard, with the county
/// panel's own numbers following. Four turns, so a season wrap is included.
#[test]
fn four_turns_run_through_the_machine_and_the_panel_keeps_up() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx_seasons = Vec::new();
    for _ in 0..4 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::KeyDown(Key::Char('E')), &mut ctx);
        // Four turns, and each of them takes the frames it takes.
        let before = game.kingdom.turn_count;
        let mut done_at = None;
        for n in 1..2_000u32 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.update(&mut ctx);
            if done_at.is_none() && game.kingdom.turn_count > before {
                done_at = Some(n);
            }
            if done_at.is_some_and(|t| n >= t + l2_view::fade::PHASES as u32) {
                break;
            }
        }
        ctx_seasons.push((game.kingdom.season, game.kingdom.year));
    }
    assert_eq!(ctx_seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
    assert_eq!(game.turns_played, 4);

    let mut panel = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut panel, &mut game, &assets);
    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&canvas, &assets, &shown, STRIP_INK).is_some()
            || find_text(&canvas, &shown, assets.ink.good).is_some()
            || find_text(&canvas, &shown, assets.ink.bad).is_some(),
        "the panel shows the population it now has ({shown})"
    );
}

/// **The sidebar stays live with the village open — and goes dead mid-drag.**
///
/// A player, having gone and checked against the original: *"The slider does
/// indeed still work with town square open and causes no issues."* He is right,
/// and `Screen_FrameInput`'s `g_screenId == 0x02` arm says so before any
/// village verb is reached — six guards, all of them the campaign map's
/// sidebar, `Labour_SplitSliderDrag` fourth among them. It did not work in
/// ours, because [`Machine::handle`] offered input to the top screen and
/// stopped there.
///
/// The second half is the half a person would never think to check, and it is
/// The reason this is a test: the `0x05`
/// (banding) and `0x06` (carrying) arms test **no sidebar guard at all**, so
/// the sidebar is dead for as long as a peasant is in the air. Making
/// all three behave alike would look like a tidy-up and would be wrong. C59.
#[test]
fn the_sidebar_slider_still_works_with_the_village_open_but_not_mid_drag() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let share = |g: &Game| g.kingdom.counties[county as usize].industry_share;

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));
    assert_eq!(m.depth(), 2, "the village is an inset over the map, not a replacement");

    // The press lands on the split slider's track, through the village.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 561, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 60, "the sidebar's slider is dead with the village open");
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)], "and it stayed open");

    // Held and moved, still through the village: `FUN_00439122` runs on the
    // level and the movement, so this is the drag continuing.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Pointer { x: 541, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 20, "the drag tracks the pointer over the village too");

    // A click on the village's own half of the screen must NOT reach the map:
    // `Map_Click` is not in the `0x02` ladder. The county under the inset is
    // whatever it was; nothing selects a new one.
    let before = game.selected;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 200, y: 200 }, &mut c);
    }
    assert_eq!(game.selected, before, "a click on the map round the inset is not a map click");

    // And the drag states. Reaching `Phase::Band` needs a press inside the
    // village's own area and nine pixels of travel.
    let mut screen = VillageScreen::new(county);
    let top = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        VillageScreen::top_y(&ctx)
    };
    send(&mut screen, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 140, y: top + 100 });
    assert_eq!(screen.phase(), village_screen::Phase::Band, "the band is up");
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: 561, y: 270 }),
        Transition::Stay,
        "screen 0x05 tests no sidebar guard, so the click must not pass to the map"
    );
}

