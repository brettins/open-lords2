#![allow(unused_imports)]
use super::*;
use super::clock_tests::*;
use super::*;
use super::setup_tests::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, MAP_LIST_ROW, MAP_LIST_ROWS, MAP_LIST_X, MAP_LIST_Y, OPTION_CELLS,
    OPTION_LIST,
};
use l2_game::setup::{self, option, SetupOptions, COUNTY_STATUS, STARTING_GOLD, START_ARMOURY};
use l2_game::Game;
use l2_game::shell::{font, Pen};
use l2_view::Canvas;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;

#[test]
fn the_build_stamp_names_a_commit_rather_than_a_version() {
    let id = l2_game::build_id::ID;
    assert!(!id.is_empty(), "build.rs always emits something");

    if id == "NO GIT" {
        return; // A tarball build. Honest, and there is nothing else to check.
    }

    let mut parts = id.splitn(3, ' ');
    let (commit, date, time) = match (parts.next(), parts.next(), parts.next()) {
        (Some(c), Some(d), Some(t)) => (c, d, t),
        _ => panic!("the stamp is `<commit>[-DIRTY] <date> <HH:MM>`, got {id:?}"),
    };
    let hex = commit.strip_suffix("-DIRTY").unwrap_or(commit);
    assert_eq!(hex.len(), 9, "nine hex digits of commit, got {hex:?}");
    assert!(
        hex.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)),
        "upper-case hex, so it reads in our caps font: {hex:?}",
    );
    assert!(
        date.len() == 10 && date.split('-').count() == 3,
        "a date, because `is this old?` is the question a hash cannot answer: {date:?}",
    );
    assert!(
        time.len() == 5 && time.as_bytes()[2] == b':',
        "the build's own HH:MM, which tells two builds of one commit apart: {time:?}",
    );

    assert_ne!(id, env!("CARGO_PKG_VERSION"), "a version is not an identity");
}

#[test]
fn the_build_stamp_is_painted_on_the_title_page() {
    let (mut game, assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Title);

    let mut page = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut page);
    }

    let mut twice = page.clone();
    let pen = Pen {
        assets: &assets.shell,
        ink: &assets.ink,
        chrome: assets.chrome.as_ref(),
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };
    l2_game::build_id::draw(&mut twice, &pen);

    let differing = (0..l2_view::canvas::HEIGHT)
        .flat_map(|y| (0..l2_view::canvas::WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| page.at(x, y) != twice.at(x, y))
        .count();

    assert_eq!(
        differing, 0,
        "drawing the build stamp again changed {differing} pixels, so it was not on the page \
         to begin with. `crate::build_id::draw` is one line at the end of the title painter \
         in screens/setup.rs, and a line like that is what a merge drops silently — which is \
         the whole reason the stamp exists.",
    );
}

#[test]
fn every_pixel_of_the_build_stamp_is_inside_the_visible_canvas() {
    let (_game, assets) = world!();
    let pen = Pen {
        assets: &assets.shell,
        ink: &assets.ink,
        chrome: assets.chrome.as_ref(),
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };

    let s = format!("BUILD {}", l2_game::build_id::ID);
    let top = l2_game::build_id::top_edge(&pen, &s);
    let height = match &assets.shell.small {
        Some(f) => f.height(&s),
        None => l2_view::text::GLYPH_H,
    };
    let screen_h = l2_view::canvas::HEIGHT as i32;

    assert!(top >= 0, "the build stamp starts at y {top}, above the top of the canvas");
    assert!(
        top + height <= screen_h,
        "the build stamp occupies y {top}..{} on a {screen_h}-line canvas, so its last {} \
         row(s) are off the bottom of the screen. A player reported exactly this. \
         `build_id::top_edge` derives the y from the font in use so that it cannot happen; \
         if this fires, the height being subtracted is not the height being drawn.",
        top + height,
        top + height - screen_h,
    );

    let mut blank = Canvas::screen();
    let mut painted = Canvas::screen();
    l2_game::build_id::draw(&mut painted, &pen);
    let rows: Vec<usize> = (0..l2_view::canvas::HEIGHT)
        .filter(|&y| (0..l2_view::canvas::WIDTH).any(|x| blank.at(x, y) != painted.at(x, y)))
        .collect();

    assert!(!rows.is_empty(), "the build stamp painted nothing at all on a blank canvas");
    let (first, last) = (rows[0] as i32, *rows.last().unwrap() as i32);
    assert!(
        last < screen_h,
        "the stamp's lowest painted row is {last} on a {screen_h}-line canvas",
    );
    assert!(
        first >= top,
        "the stamp painted row {first}, above the y {top} it claims to start at",
    );
    assert!(
        last >= screen_h - 2 - height,
        "the stamp's lowest painted row is {last}, above the band y {}..{screen_h} it is \
         supposed to occupy — rows have been clipped off the bottom",
        screen_h - 2 - height,
    );
}

// `FUN_0041EA14` draws no clock: 143 bytes, a box, two `Ui_DrawCentred` calls
// and `FUN_0041EAA3`'s four items. A player asked for one — *"can the build have
// time in MST in the title screen"* — so it is a deliberate divergence, marked
// as such in `l2_game::wallclock` and in `docs/draws.md`. Nobody should "fix" it
// toward the binary, and nobody should cite it as a reproduction.

