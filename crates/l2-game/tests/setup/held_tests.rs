//! `Hotspot_Test` (`0x0040E3EE`) kind 2 on page 12: the down edge, then the
//! handler again every 320 ms while the button is down on the record. The
//! coordinates are the widget table's own — `node tools/oracle/widgets.js
//! widgets 4dcf68 22`.
#![allow(unused_imports)]
use super::*;
use l2_game::input::Event;
use l2_game::press::{HELD_PULSE_MS, TICK_MS};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen};

/// How many [`TICK_MS`] ticks one pulse takes: 320 / 16.
const PULSE_TICKS: u32 = HELD_PULSE_MS / TICK_MS;

fn page12() -> SetupScreen {
    SetupScreen::new(SetupPage::Skirmish)
}

fn release(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Release { x, y }, &mut ctx);
}

fn ticks(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, n: u32) {
    for _ in 0..n {
        tick(screen, game, assets);
    }
}

/// Ablation: give the scroll arrow `Kind::Press` in `held_kind` and the top
/// never leaves 1.
#[test]
fn holding_the_scroll_arrow_repeats_after_320_ms() {
    let (mut game, assets) = world!();
    let mut s = page12();
    // `FUN_0043D9CD`, the lower arrow, hotspot +1.
    click(&mut s, &mut game, &assets, 610, 276);
    assert_eq!(s.skirmish().top, 1, "the down edge acts once");
    ticks(&mut s, &mut game, &assets, PULSE_TICKS - 1);
    assert_eq!(s.skirmish().top, 1, "304 ms is not a pulse");
    ticks(&mut s, &mut game, &assets, 1);
    assert_eq!(s.skirmish().top, 2, "the 320 ms pulse is the second call");
    ticks(&mut s, &mut game, &assets, PULSE_TICKS);
    assert_eq!(s.skirmish().top, 3, "and it repeats flat, no ramp");
}

/// Ablation: drop the `Event::Release` arm from `SetupScreen::press_event` and
/// the top runs to the clamp on its own.
#[test]
fn a_plain_click_acts_once() {
    let (mut game, assets) = world!();
    let mut s = page12();
    click(&mut s, &mut game, &assets, 610, 276);
    release(&mut s, &mut game, &assets, 610, 276);
    ticks(&mut s, &mut game, &assets, PULSE_TICKS * 4);
    assert_eq!(s.skirmish().top, 1, "no repeat after the release");
}

fn pointer(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Pointer { x, y }, &mut ctx);
}

/// `Hotspot_Test` re-hit-tests the record every frame and keeps no memory of
/// where the press began (`00400000.c:7878-7891`), so off the record the pulse
/// reaches no handler and back on it the press's own count — `DAT_0058FEB0`,
/// zeroed at `7882` and nowhere else — carries on.
///
/// Ablation: zero the pulse count on re-entry and the 320th millisecond passes
/// with the top still 1.
#[test]
fn the_pointer_leaving_the_record_suspends_the_pulse_and_returning_resumes_it() {
    let (mut game, assets) = world!();
    let mut s = page12();
    click(&mut s, &mut game, &assets, 610, 276);
    pointer(&mut s, &mut game, &assets, 300, 100);
    ticks(&mut s, &mut game, &assets, PULSE_TICKS - 1);
    assert_eq!(s.skirmish().top, 1, "off the record is not held");
    pointer(&mut s, &mut game, &assets, 610, 276);
    ticks(&mut s, &mut game, &assets, 1);
    assert_eq!(s.skirmish().top, 2, "the count is the press's, not the re-entry's");
}

/// The kind-2 arm is reached from the loop that hit-tests the pointer, so the
/// button held while the pointer crosses to the other arrow pulses **that**
/// arrow — the two are one table, `0x004DCF68`.
#[test]
fn dragging_onto_the_other_arrow_pulses_the_other_arrow() {
    let (mut game, assets) = world!();
    let mut s = page12();
    click(&mut s, &mut game, &assets, 610, 276);
    ticks(&mut s, &mut game, &assets, PULSE_TICKS);
    assert_eq!(s.skirmish().top, 2, "two pulses down");
    // `FUN_0043D9CD` again, hotspot −1.
    pointer(&mut s, &mut game, &assets, 610, 190);
    ticks(&mut s, &mut game, &assets, PULSE_TICKS);
    assert_eq!(s.skirmish().top, 1, "the up arrow, on the next pulse of the same divider");
}

