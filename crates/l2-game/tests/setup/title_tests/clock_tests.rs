#![allow(unused_imports)]
use super::*;
use super::build_stamp_tests::*;
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

/// A fixed reading: 2026-01-15T19:00:00Z, which is 12:00 in MST.
const NOON_MST: i64 = 1_768_503_600;

/// The title page's own flat pen — `head.flat()` in `SetupScreen::draw`.
fn title_pen(assets: &Assets) -> Pen<'_> {
    Pen {
        assets: &assets.shell,
        ink: &assets.ink,
        chrome: assets.chrome.as_ref(),
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    }
    .flat()
}

/// **The clock is on the page** — the build stamp's identity test, for its
/// reason: counting non-background pixels measures `gateway.pl8`, because the
/// title page carries full-screen artwork and no pixel down there is background.
/// Draw the page, copy it, draw the clock again onto the copy, and require the
/// two to be identical. Text is an opaque blit
/// changes nothing — but only if it was there the first time.
///
/// **This is the test that fails before the change**: with the
/// `crate::wallclock::draw` line absent from the title painter the second pass
/// *adds* the clock, and the canvases differ.
#[test]
fn the_mst_clock_is_painted_on_the_title_page() {
    let (mut game, mut assets) = world!();
    assets.wall_clock = Some(NOON_MST);
    let mut screen = SetupScreen::new(SetupPage::Title);

    let mut page = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut page);
    }

    let mut twice = page.clone();
    l2_game::wallclock::draw(&mut twice, &title_pen(&assets), NOON_MST);

    let differing = (0..l2_view::canvas::HEIGHT)
        .flat_map(|y| (0..l2_view::canvas::WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| page.at(x, y) != twice.at(x, y))
        .count();

    assert_eq!(
        differing, 0,
        "drawing the MST clock again changed {differing} pixels, so it was not on the page to \
         begin with. `crate::wallclock::draw` is one line at the end of the title painter in \
         screens/setup.rs, guarded by `ctx.assets.wall_clock`.",
    );
}

/// **With no reading.** Every test
/// and every headless driver leaves `Assets::wall_clock` at `None`, which is
/// what makes *nothing below the shell may read a clock* enforceable
/// — and it is the ablation for the test above, which would also
/// pass if the clock were drawn from something this file cannot set.
#[test]
fn a_page_with_no_reading_draws_no_clock() {
    let (mut game, assets) = world!();
    assert_eq!(assets.wall_clock, None, "a loaded Assets carries no clock of its own");
    let mut screen = SetupScreen::new(SetupPage::Title);

    let mut page = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut page);
    }

    let mut twice = page.clone();
    l2_game::wallclock::draw(&mut twice, &title_pen(&assets), NOON_MST);
    let differing = (0..l2_view::canvas::HEIGHT)
        .flat_map(|y| (0..l2_view::canvas::WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| page.at(x, y) != twice.at(x, y))
        .count();
    assert!(
        differing > 0,
        "the page drew a clock with no reading to draw it from, so something below the shell \
         found a clock of its own — docs/netcode.md D-5",
    );
}

/// **Where it landed.** The build stamp shipped
/// hanging off the bottom of the screen because its test proved only the second;
/// this asks both, and the two halves fail together.
///
/// It also asks the question this corner cannot avoid: the line is
/// **right-aligned**, so its failure mode is the right edge
/// bottom, and a face wider than the one the margin was chosen against pushes it
/// off the side instead of down.
#[test]
fn every_pixel_of_the_mst_clock_is_inside_the_visible_canvas() {
    let (_game, assets) = world!();
    let pen = title_pen(&assets);
    let s = l2_game::wallclock::hhmm(NOON_MST);
    let left = l2_game::wallclock::left_edge(&pen, &s);
    let top = l2_game::wallclock::top_edge(&pen, &s);
    let (screen_w, screen_h) = (l2_view::canvas::WIDTH as i32, l2_view::canvas::HEIGHT as i32);
    let height = match &assets.shell.body {
        Some(f) => f.height(&s),
        None => l2_view::text::GLYPH_H,
    };
    let width = match &assets.shell.body {
        Some(f) => f.width(&s),
        None => l2_view::text::width(&s),
    };

    assert!(left >= 0, "the clock starts at x {left}, left of the canvas");
    assert!(top >= 0, "the clock starts at y {top}, above the canvas");
    assert!(left + width <= screen_w, "the clock runs to x {}, past {screen_w}", left + width);
    assert!(top + height <= screen_h, "the clock runs to y {}, past {screen_h}", top + height);

    // The observable consequence. A canvas clips, so anything that fell off an
// edge never appears — so the geometry above is not enough
    // on its own.
    let blank = Canvas::screen();
    let mut painted = Canvas::screen();
    l2_game::wallclock::draw(&mut painted, &pen, NOON_MST);
    let marked: Vec<(usize, usize)> = (0..l2_view::canvas::HEIGHT)
        .flat_map(|y| (0..l2_view::canvas::WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| blank.at(x, y) != painted.at(x, y))
        .collect();
    assert!(!marked.is_empty(), "the clock painted nothing at all on a blank canvas");

    let last_row = marked.iter().map(|&(_, y)| y as i32).max().unwrap();
    let last_col = marked.iter().map(|&(x, _)| x as i32).max().unwrap();
    let first_col = marked.iter().map(|&(x, _)| x as i32).min().unwrap();
    assert!(last_row < screen_h, "the clock's lowest painted row is {last_row}");
    assert!(last_col < screen_w, "the clock's rightmost painted column is {last_col}");
    assert!(
        first_col >= left,
        "the clock painted column {first_col}, left of the x {left} it claims to start at",
    );
    assert!(
        last_col >= screen_w - 2 - width,
        "the clock's rightmost painted column is {last_col} on a {screen_w}-wide canvas, so \
         columns have been clipped off the right-hand edge",
    );
    // And it really is in the bottom-right corner the painter says it is in,
    //
    assert!(
        first_col > screen_w / 2 && last_row > screen_h - 20,
        "the clock occupies x {first_col}.. y ..{last_row}, which is not the bottom-right corner",
    );
}

/// **It does not sit on the build stamp**, the other invention in this band of
/// the picture. Three of our own hotspots reached a merge placed in the
/// original's coordinate space without checking what was already there
/// (`docs/arms.json`, `ours/divide-cancel-button`); this is the drawing half of
/// that mistake, asked before it can happen.
#[test]
fn the_clock_and_the_build_stamp_do_not_overlap() {
    let (_game, assets) = world!();
    let pen = title_pen(&assets);
    let stamp = format!("BUILD {}", l2_game::build_id::ID);
    let stamp_w = match &assets.shell.small {
        Some(f) => f.width(&stamp),
        None => l2_view::text::width(&stamp),
    };
    let clock = l2_game::wallclock::hhmm(NOON_MST);
    let clock_x = l2_game::wallclock::left_edge(&pen, &clock);
    assert!(
        4 + stamp_w < clock_x,
        "the build stamp ends at x {} and the clock starts at x {clock_x}",
        4 + stamp_w,
    );
}

/// **The clock asks for a repaint once a minute and at no other time.** A still
/// screen costing nothing is what `Machine::update` is written around; a clock
/// that marked the machine dirty every tick would repaint the whole front end
/// sixty times a second for a picture that changes once in thirty-six hundred.
#[test]
fn the_clock_asks_for_a_repaint_only_when_the_minute_turns() {
    let (mut game, mut assets) = world!();
    let mut screen = SetupScreen::new(SetupPage::Title);

    assets.wall_clock = Some(NOON_MST);
    tick(&mut screen, &mut game, &assets);
    assert!(screen.take_redraw(), "the first reading puts a clock on a page that had none");
    assert!(!screen.take_redraw(), "and it is taken, not left standing");

    assets.wall_clock = Some(NOON_MST + 59);
    tick(&mut screen, &mut game, &assets);
    assert!(!screen.take_redraw(), "12:00 is still 12:00 fifty-nine seconds in");

    assets.wall_clock = Some(NOON_MST + 60);
    tick(&mut screen, &mut game, &assets);
    assert!(screen.take_redraw(), "the minute turned and the page still said 12:00");
}

/// **The clock reaches no simulation** — `docs/netcode.md` D-5, as an assertion
///
///
/// Two copies of one game are stepped through the same ticks under wall-clock
/// readings six months and eleven hours apart. The worlds must stay equal and
/// their checksums identical: `Assets` is in no save, no digest and no
/// `Kingdom`
/// front page.
#[test]
fn the_clock_cannot_reach_the_simulation() {
    let (mut early, mut assets) = world!();
    let mut late = early.clone();

    let mut a = SetupScreen::new(SetupPage::Title);
    let mut b = SetupScreen::new(SetupPage::Title);
    for i in 0..30 {
        assets.wall_clock = Some(NOON_MST + i * 37);
        tick(&mut a, &mut early, &assets);
        assets.wall_clock = Some(NOON_MST + 15_638_400 + 39_600 + i * 91);
        tick(&mut b, &mut late, &assets);
    }

    assert_eq!(
        l2_kingdom::save::checksum(&early.kingdom),
        l2_kingdom::save::checksum(&late.kingdom),
        "two wall clocks produced two kingdoms — a clock has reached the simulation",
    );
    assert_eq!(early, late, "and the games differ in something the checksum does not cover");
}



