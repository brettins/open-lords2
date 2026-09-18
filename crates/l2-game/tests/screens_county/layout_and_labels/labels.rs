#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
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

/// `FUN_0041062E` (wood): `Ui_DrawDelta(next, 0, " ", " ", 0x22C, pitch*row +
/// 0x139, &g_font10, 0xFA, 0xF9)` inside `g_dropShadow = 1`. The coordinate is
/// typed from that line, and wood is made the column's only row, so `row` is 0.
#[test]
fn the_industry_forecast_is_a_dropped_font_10_number() {
    let (mut game, assets) = world!();
    const POS: u8 = 0xFA;
    let ten = assets.shell.ten.as_ref().expect("Font_10.pl8");
    let small = assets.shell.small.as_ref().expect("Fntl2_9.pl8");

    let county = 8;
    game.select(county as u8);
    {
        let c = &mut game.kingdom.counties[county];
        for i in &mut c.industry {
            i.enabled = false;
        }
        c.castle_degraded = 0;
        c.industry[l2_kingdom::tables::Commodity::Wood.index()].enabled = true;
        c.industry[l2_kingdom::tables::Commodity::Wood.index()].next_season = 5;
    }
    let canvas = draw(&mut MapScreen::new(), &mut game, &assets);

    let wood = find_font_text(&canvas, ten, "+5 ", POS).expect("the wood forecast, in Font_10.pl8");
    assert_eq!(wood, (0x22C + 8, 0x139), "FUN_0041062E's delta, row 0");
    assert!(is_dropped(&canvas, ten, "+5 ", wood), "g_dropShadow is set around the wood row");
    assert!(find_font_text(&canvas, small, "+5 ", POS).is_none(), "and not in Fntl2_9.pl8");
}

/// `CountyStrip_DrawCastleIcon` (`0x004107D1`), with the materials paid:
///
/// The castle is the column's only row, so `row` is 0 and `nudge` is 6 (the
/// function's `DAT_0056D68C < 2`). Coordinates are typed from the lines above.
///
/// Ablations, all run: drawing the number as `"{seasons} "` with no lead finds
/// the digits at `(572, 320)`, not `(576, 320)`; pointing `small_dropped` at
/// `shell.ten` loses the word (*"Seasons" is not on the castle cell*);
/// deleting the shadow blit from `Font::draw_dropped` fails the number's shadow
/// claim. Two assertions were never separately red and say so: the word's ink
/// count cannot fail while its glyph search passes, and the word's shadow claim
/// sits behind the number's, which fires first on the same ablation.
#[test]
fn the_castle_cell_puts_its_number_in_font_10_and_its_word_in_fntl2_9() {
    let (mut game, assets) = world!();
    const POS: u8 = 0xFA;
    let ten = assets.shell.ten.as_ref().expect("Font_10.pl8");
    let small = assets.shell.small.as_ref().expect("Fntl2_9.pl8");

    let county = 8;
    game.select(county as u8);
    {
        let c = &mut game.kingdom.counties[county];
        for i in &mut c.industry {
            i.enabled = false;
        }
        c.castle_degraded = 1;
        c.castle_switch = true;
        c.castle_stone_owed = 0;
        c.castle_wood_owed = 0;
        c.labour[l2_kingdom::tables::JOB_CASTLE_BUILDING] = 0;
    }
    let seasons = l2_kingdom::industry::castle_seasons_left(
        &game.kingdom.tables,
        &game.kingdom.counties[county],
    );
    assert_eq!(seasons, 100, "setup: an unstaffed castle is a hundred seasons off");
    let canvas = draw(&mut MapScreen::new(), &mut game, &assets);

    let number = find_font_text(&canvas, ten, "100 ", POS).expect("the seasons, in Font_10.pl8");
    assert_eq!(number, (0x23C + 4, 0x13A + 6), "lead ' ' at 0x23C, digits after; nudge 6");
    assert!(is_dropped(&canvas, ten, "100 ", number), "g_dropShadow is set around the castle cell");

    // The word: group 8 index 0x43, plural because the value is not 1.
    let noun = assets.shell.text(8, 0x43).to_string();
    assert!(
        noun.chars().any(|c| c.is_ascii_lowercase()),
        "setup: L2.eng 8/0x43 should be a lowercase-bearing word, got {noun:?}"
    );
    let at = find_font_text(&canvas, small, &noun, POS)
        .unwrap_or_else(|| panic!("{noun:?} is not on the castle cell in Fntl2_9.pl8"));
    assert_eq!(at, (0x234, 0x146 + 6), "Ui_DrawUnitNoun's x and y");
    let (w, h) = (small.width(&noun), small.height(&noun));
    let drawn = ink_in(&canvas, at.0, at.1, w, h, POS);
    assert!(drawn >= 20, "{noun:?} must render something in its box: {drawn} pixels of 0xFA");
    assert!(is_dropped(&canvas, small, &noun, at), "and the word is dropped too");
}

/// A player: *"in the original, the text 'END TURN' would disappear when you
/// click it, until the new turn was ready."* `Screen_DrawEndTurn`
/// (`0x0041A734`) blits the strip unconditionally and draws the label only when
/// `g_realms[g_localPlayer].aiStep < 999` — `Turn_End` (`0x0043AC23`) sets that
/// to 999 on the click and `Turn_BeginPlayersTurn` puts it back to 0 at the top
/// of the next turn. So it is a **conditional draw**, and the interval is
/// exactly *turn in flight*.
#[test]
fn the_end_turn_label_disappears_while_the_turn_runs() {
    let (mut game, assets) = world!();
    game.select(8);
    let mut screen = MapScreen::new();

    let label_band = |canvas: &Canvas| -> Vec<u8> {
        let mut out = Vec::new();
        for y in 460..480usize {
            for x in 478..640usize {
                out.push(canvas.at(x, y));
            }
        }
        out
    };

    let idle = draw(&mut screen, &mut game, &assets);
    let idle_again = draw(&mut screen, &mut game, &assets);
    assert_eq!(label_band(&idle), label_band(&idle_again), "an idle repaint is idempotent");

    send(&mut screen, &mut game, &assets, Event::Click { x: 500, y: 470 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(
        l2_game::turn::turn_in_flight(&game),
        "the click should have started a turn, or this test asserts nothing",
    );
    let running = draw(&mut screen, &mut game, &assets);
    assert_ne!(
        label_band(&idle),
        label_band(&running),
        "the End Turn strip is unchanged while the turn runs, so the label never went",
    );

    run_turn(&mut screen, &mut game, &assets);
    assert!(!l2_game::turn::turn_in_flight(&game), "the turn should have finished");
    let done = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        label_band(&idle),
        label_band(&done),
        "the label did not come back the way it went",
    );
}

