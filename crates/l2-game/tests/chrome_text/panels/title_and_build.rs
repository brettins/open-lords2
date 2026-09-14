#![allow(unused_imports)]
use super::*;
use super::drop_down_plate::*;
use super::tile_panel::*;
use super::*;
use super::top_bar::*;
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

/// **`FUN_0041EA14` puts the title at `y = 0x1E`, and we put it at `0x20`.**
///
/// Two pixels, and it is the sort of number that is only ever wrong because
/// nobody read the painter's arguments back. The subtitle beside it — `0x3A` —
/// was right all along, which is what made the pair worth checking.
#[test]
fn the_title_page_draws_the_game_s_own_name_where_the_painter_puts_it() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install");
    };
    let platform = Platform::builder()
        .base(&dir)
        .build()
        .expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let mut screen = SetupScreen::new(SetupPage::Title);
    let canvas = draw(&mut screen, &mut game, &assets);

    let title = assets.shell.text(11, 0).to_string();
    assert_eq!(title, "Lords of the Realm 2", "L2.eng 11/0");
    // The heading's own mode: **embossed** — `FUN_0041EA14` never sets
    // `DAT_005AEA40` for it — with `DAT_0058FE2C`'s drop capitals on. The
    // gateway pair, because this page runs under `gateway.256` and
    // `Ui_DrawText` swaps its shadow colours on `g_screenId == 0x1F`.
    let style = Style {
        colour: font::TEXT,
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };
    let (_, y) = find_styled(&canvas, heading, &title, &style)
        .expect("the title is not on the page in Fntl2_22.pl8");
    // **`0x1E` is written out, not read from `setup::TITLE_Y`.** Deriving the
    // expectation from the constant under test is `docs/agents.md`'s first way
    // to ablate wrongly: change the constant and the probe moves with it, and
    // the test stays green while asserting the code agrees with itself. This
    // number is pinned from `FUN_0041EA14`'s own argument list and no
    // expression here mentions the constant.
    assert_eq!(
        y, 0x1E,
        "FUN_0041EA14: Ui_DrawCentred(11, 0, 0x80, 0x1E, 0x180, &g_fontHeading, 0x3F)"
    );
}

/// **The build stamp is inside the canvas, in the plain face.** Three earlier
/// versions of this check were each true of something adjacent — see
/// `docs/agents.md`. This one asserts the two things anybody wanted: every
/// pixel it writes is on screen, and it is not drawn in a blackletter face.
#[test]
fn the_build_stamp_is_legible_and_on_screen() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install");
    };
    let platform = Platform::builder()
        .base(&dir)
        .build()
        .expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let small = assets.shell.small.as_ref().expect("Fntl2_9.pl8");
    let mut screen = SetupScreen::new(SetupPage::Title);
    let canvas = draw(&mut screen, &mut game, &assets);
    let s = format!("BUILD {}", l2_game::build_id::ID);
    let (_, y) = find_in(&canvas, small, &s, assets.ink.dim).expect("the stamp, in Fntl2_9.pl8");
    assert!(
        y + small.height(&s) <= l2_view::canvas::HEIGHT as i32,
        "the stamp's last row is at {} and the canvas ends at {}",
        y + small.height(&s),
        l2_view::canvas::HEIGHT
    );
}

// ------------------------------------------------------------------- a look

