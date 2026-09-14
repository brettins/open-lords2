//! The menu bar, the options menu and the turn timer.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.


#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod menu_bar;
pub use menu_bar::*;
mod turn_timer;
pub use turn_timer::*;

use common::*;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screens::map::MapScreen;
use l2_game::screens::menubar;
use l2_game::screens::options::Page as OptionsPage;
use l2_game::screens::saveload::Mode as SaveLoadMode;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::chrome;
use l2_view::Canvas;

/// `n` fixed ticks of a whole [`Machine`] — which is where the turn timer
/// counts, because the original counts it in `Turn_Tick` and not in a screen.
fn tick_stack(m: &mut Machine, game: &mut Game, assets: &Assets, n: u32) {
    for _ in 0..n {
        let mut ctx = Ctx { game: &mut *game, assets };
        m.update(&mut ctx);
    }
}

/// The same stack drawn with the time limit taken away for the one frame, so
/// that `with == without` says *the timer drew nothing* as an equality, with no
/// threshold and no knowledge of what else is on the screen.
fn draw_stack_without_the_timer(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let limit = game.kingdom.options.time_limit;
    game.kingdom.options.time_limit = 0;
    let canvas = draw_stack(m, game, assets);
    game.kingdom.options.time_limit = limit;
    canvas
}

/// Where the digits of `Ui_DrawNumberRight(v, ' ', &DAT_004D41D0, 0x1A8, 0x1BA,
/// 0x32, &g_fontBody, 0x3F)` are, searched for **inside that box only**.
fn timer_digits(canvas: &Canvas, assets: &Assets, v: i32) -> Option<(i32, i32)> {
    let window = crop(canvas, 0x1A8, 0x1BA, 0x32 + 8, 20);
    find_body(&window, assets, &v.to_string(), font::TEXT)
}

/// Where they belong, in the box's own coordinates.
///
/// The buffer is lead, digits and suffix, centred whole. **The suffix is under
/// test and the expectation does not go through it**: `&DAT_004D41D0` is `20 00`
/// in the image, one space, and it is written here as one more space's advance
/// Not read from `turn_clock::SUFFIX`; emptying the constant moves
/// the picture two pixels and leaves this where it is.
fn timer_digits_expected(assets: &Assets, v: i32) -> (i32, i32) {
    let space = body_width(assets, " ");
    let buffer = body_width(assets, &format!(" {v}")) + space;
    (((0x32 - buffer) / 2).max(0) + space, 0)
}

