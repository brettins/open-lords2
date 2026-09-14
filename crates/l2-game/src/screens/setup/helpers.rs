#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::ui::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

/// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` — the name field's own three
/// arguments, in one place because three call sites open it.
fn begin_name(seed: &str) -> TextField {
    TextField::begin(seed, text::NAME_MAX_TYPED, text::NAME_MAX_PIXELS, text::Kind::Text)
}

/// `FUN_00403CF4(x, y, w, h, colour)` — **a flat one-pixel rectangle in one
/// colour**, four `FUN_00403A8F` line draws and nothing else.
///
/// It is not [`shell::inset_rect`] (two-tone, `0x10`/`0x1F`) and it is not
/// [`shell::button_recess`] / `FUN_00403EE4` (two-tone the other way,
/// `0x35`/`0x28`). Three different rectangles that all look like a border, and
/// this module drew the wrong one of the three on pages 3 and 13 until the
/// painters were read side by side.
fn outline_rect(canvas: &mut Canvas, r: Rect, colour: u8) {
    canvas.fill_rect(r.x, r.y, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y + r.h - 1, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y, 1, r.h, colour);
    canvas.fill_rect(r.x + r.w - 1, r.y, 1, r.h, colour);
}

/// What a rectangle on one of these pages does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// The n'th menu item or button of this page.
    Item(usize),
    /// Open custom-game option n's drop-down.
    Open(usize),
    /// Choose value n from the open drop-down.
    Choose(usize),
    /// The n'th visible row of the map list.
    Map(usize),
}

