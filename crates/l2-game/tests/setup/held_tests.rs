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

/// `Hotspot_Test` re-hit-tests the record every frame, so the pointer leaving
/// it ends the hold.
#[test]
fn the_pointer_leaving_the_record_ends_the_pulse() {
    let (mut game, assets) = world!();
    let mut s = page12();
    click(&mut s, &mut game, &assets, 610, 276);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    s.handle(Event::Pointer { x: 300, y: 100 }, &mut ctx);
    ticks(&mut s, &mut game, &assets, PULSE_TICKS * 4);
    assert_eq!(s.skirmish().top, 1, "off the record is not held");
}

