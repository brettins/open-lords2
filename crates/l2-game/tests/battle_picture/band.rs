//! It has one. `Screen_DrawBattlefield` (`0x004233F7`) opens with
//! `Gfx_ClearScreen` (`0x004B1867`), which is
//! `FUN_004B3E51(DAT_004EA1A8, 0x4B000)`; `FUN_004B3E51` (`0x004B3E51`) is a
//! `memset` to 0 and `0x4B000` is 640 × 480. Every later blit in that function
//! and in `FUN_00423530` (`0x00423530`) is at x ≥ `0x1E0`, and every sprite is
//! clipped to `Clip_Vertical(0x18, 0x1D8)`. The band is therefore the clear's
//! leftover, black, for the life of the screen.
//!
//! The probes are the binary's own literals — `0x1D8`, `0x1E0`, and index 0
//! out of `FUN_004B3E51` — never an expression of the code under test.

use super::*;

const BAND_X1: usize = 0x1E0;
const BAND_Y0: usize = 0x1D8;
const BAND_Y1: usize = 480;

fn over_dirt(dirt: u8) -> Canvas {
    let (assets, _platform) = install().expect("checked by the caller");
    let (mut g, mut m) =
        staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| {
            (x as i32 - 4, y as i32 - 6)
        });
    let mut canvas = Canvas::screen();
    canvas.clear(dirt);
    paint(&mut m, &mut g, &assets, &mut canvas);
    canvas
}

#[test]
fn the_bottom_band_is_the_entry_clears_black_and_nothing_repaints_it() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no battlefield to repaint");
    }
    let canvas = over_dirt(0xAB);
    let stale = (BAND_Y0..BAND_Y1)
        .flat_map(|y| (0..BAND_X1).map(move |x| (x, y)))
        .filter(|&(x, y)| canvas.at(x, y) != 0)
        .count();
    assert_eq!(stale, 0, "{stale} of the band's pixels are not the clear's index 0");
}

/// **The clear is the whole 640 × 480 frame, not the viewport.** `0x4B000` is
/// the byte count `Gfx_ClearScreen` passes, so no row of the screen survives a
/// repaint unpainted — the menu bar's rows and the column's are covered by
/// their own blits, and the band by nothing.
#[test]
fn the_entry_clear_covers_the_frame_below_the_column_too() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no battlefield to repaint");
    }
    let canvas = over_dirt(0xAB);
    for (x, y) in [(0usize, BAND_Y0), (BAND_X1 - 1, BAND_Y1 - 1)] {
        assert_ne!(canvas.at(x, y), 0xAB, "({x}, {y}) is still the previous screen");
    }
    // And the heartbeat's own corner, (1, 475) — `FUN_0041A844` draws a 10 × 5
    // outline there in multiplayer, so the original holds that corner blank.
    assert_eq!(canvas.at(1, 475), 0, "the heartbeat's corner is not blank");
}
