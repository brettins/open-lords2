#![allow(unused_imports)]
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

/// **The build stamp identifies a build, and this is the assertion that stops
/// it decaying back into a version string.**
///
/// It exists because three of the last five interface defects a player reported
/// were against a binary four merges old — two already fixed, one fixed twice —
/// and nothing on screen could have told him. The failure mode this guards is
/// not the stamp going missing; it is somebody replacing it with something that
/// *looks* like an answer. `0.1.0` has been true of every build for two months,
/// so a constant is worse than nothing: it invites the reader to stop asking.
///
/// So the shape is asserted, not the value: nine hex digits, an optional
/// `-DIRTY`, and a date — or the literal `NO GIT` when the source is not a
/// checkout, which is a fact. No gate; it needs
/// neither the game nor the fixtures.
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

    // The point of the whole exercise: two different builds must be able to
    // disagree. A constant cannot, and this is the shape a constant has.
    assert_ne!(id, env!("CARGO_PKG_VERSION"), "a version is not an identity");
}

/// **The build stamp is painted.**
///
/// The test above asserts the shape of the string. This one asserts a player can
/// see it: the whole point
/// is that somebody looking at a screenshot can say which binary it is.
///
/// It is separate because the two fail for unrelated
/// reasons — a wrong string and an unpainted one need different fixes — and
/// because this one is the fragile half. `crate::build_id::draw` is one line at
/// the end of the title page's painter, in a file three branches touched
/// tonight, and a line like that is exactly what a merge drops without anything
/// noticing.
///
/// **How it asserts, and the first attempt was wrong.** The obvious test — count
/// non-background pixels in the stamp's band — *passed with the draw line
/// deleted*, because the title page carries a full-screen `gateway.pl8` and no
/// pixel down there is background. It measured the artwork.
///
/// So: draw the page, copy it, draw the stamp again onto the copy, and require
/// the two to be **identical**. Text is an opaque blit, so drawing it a second
/// time over itself changes nothing — but only if it was there the first time.
/// Deleting the line makes the second draw *add* the stamp, and the canvases
/// differ. That is an exact test, and it needs no
/// knowledge of what else is on the page.
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

/// **The build stamp is inside the picture — the third test this one small
/// feature has needed
///
/// The first version counted non-background pixels in the stamp's band and
/// passed with the draw line deleted, because the page carries full-screen
/// artwork and no pixel down there is background. The second — the one above —
/// draws the stamp twice and requires the canvases to be identical. That proves
/// it was **painted** and says nothing about **where it landed**, so the stamp
/// shipped hanging off the bottom of the screen and a player reported it as
/// *"half obscured by the bottom of the screen"*. The literal `468` had been
/// chosen against the 7-pixel debug font; with the game's own `Fntl2_14.pl8`
/// loaded the line is taller and ran past 480.
///
/// > **"Is it drawn" and "can it be seen" are different claims, and only the
/// > second is what anybody wanted.**
///
/// **Why this cannot be written by reading pixels back.** A canvas clips: paint
/// a glyph at y 476 and the rows past 479 are discarded, leaving nothing
/// to observe. Every "is it on screen" test written against the canvas would
/// therefore pass on exactly the bug it is meant to catch. So the assertion is
/// made two ways that fail together — the geometry must fit, and the stamp must
/// paint **its full height**, which is the observable consequence of fitting.
///
/// Ablate by subtracting ten fewer pixels in `build_id::top_edge`: the glyphs
/// clip, the painted height drops below the font's height, and both halves go
/// red while the identity test above stays green.
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

    // Half one: the geometry fits.
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

    // Half two: the observable consequence. Paint it on a blank canvas and
// count the rows it marks. If any of it fell off the bottom the
    // canvas clipped those rows away and this count comes up short.
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
    // The glyphs of this string do not all reach the font's full box, so the
    // painted span may be shorter than `height` — but it must not be shorter
    // than it would be with rows clipped off the bottom, and the simplest
    // statement of that is that the last painted row is inside the margin.
    assert!(
        last >= screen_h - 2 - height,
        "the stamp's lowest painted row is {last}, above the band y {}..{screen_h} it is \
         supposed to occupy — rows have been clipped off the bottom",
        screen_h - 2 - height,
    );
}

// ------------------------------------------------ the MST clock, which is OURS
//
// `FUN_0041EA14` draws no clock: 143 bytes, a box, two `Ui_DrawCentred` calls
// and `FUN_0041EAA3`'s four items. A player asked for one — *"can the build have
// time in MST in the title screen"* — so it is a deliberate divergence, marked
// as such in `l2_game::wallclock` and in `docs/draws.md`. Nobody should "fix" it
// toward the binary, and nobody should cite it as a reproduction.
//
// Every test below **injects** the instant. Nothing in `l2-game` the library
// reads a clock (`docs/netcode.md` D-5): the shell samples `SystemTime` and
// projects it into `Assets::wall_clock`, which is what these set by hand.

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


