#![allow(unused_imports)]
use super::*;
use super::pinned_part::*;
use super::diplomacy_part::*;
use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

#[test]
fn the_army_division_rows_sit_on_the_parchment_field() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Divide(0), &mut g, &a);
    let (wx, wy) = (DIVIDE_WELL_AT.0, DIVIDE_WELL_AT.1);
    for (col, row) in [(0i32, 16i32), (25, 16), (0, 17), (25, 17)] {
        let index = PANELS_TEXTURE
            + (col as usize % PANELS_TEXTURE_DIM)
            + (row as usize % PANELS_TEXTURE_DIM) * PANELS_TEXTURE_DIM;
        frame_is_drawn(
            &c,
            &s.panels,
            index,
            wx + col * PANELS_CELL,
            wy + row * PANELS_CELL,
            &format!("the division well's cell ({col}, {row})"),
        );
    }
}


/// `Screen_ConfirmBox` (`0x0040CCFA`) is three statements: `FUN_004093E0(x −
/// 0x10, y − 0x10, 0xE, 8)`, an `Eng_DrawString`, and the widget pass. The
/// ground was a `fill_rect(ink.background)` here.
///
/// And `g_confirmWidgets` (`0x004DD310`) carries frames **29 and 31** — a mailed
/// hand thumb up and thumb down. This drew `system::OK` and `system::OK + 2`,
/// which are the close corner and its neighbour: **the right sheet, the wrong
/// frames, at the right coordinates.** Nothing that compares two canvases of our
/// own could tell; only the sheet can.
#[test]
fn the_yes_no_box_is_the_shared_ground_and_the_thumb_pair() {
    let (mut g, a, s) = world!();
    let mut c = Canvas::screen();
    let pen = l2_game::shell::Pen {
        assets: &a.shell,
        ink: &a.ink,
        chrome: a.chrome.as_ref(),
        shadow: Some(l2_game::shell::font::SHADOW),
        caps: None,
    };
    pen.window(
        &mut c,
        battlefield::CONFIRM_BOX.x,
        battlefield::CONFIRM_BOX.y,
        battlefield::CONFIRM_COLS,
        battlefield::CONFIRM_ROWS,
        battlefield::BOX_SET,
    );
    pen.system_frame(
        &mut c,
        battlefield::CONFIRM_YES_FRAME,
        battlefield::CONFIRM_YES.x,
        battlefield::CONFIRM_YES.y,
    );
    pen.system_frame(
        &mut c,
        battlefield::CONFIRM_NO_FRAME,
        battlefield::CONFIRM_NO.x,
        battlefield::CONFIRM_NO.y,
    );
    let _ = &mut g;

    frame_is_drawn(
        &c,
        &s.panels,
        PANELS_CORNER_TL + PANELS_SET_B,
        CONFIRM_AT.0,
        CONFIRM_AT.1,
        "the confirmation box's corner",
    );
    frame_is_drawn(
        &c,
        &s.system,
        CONFIRM_YES_FRAME,
        CONFIRM_YES_AT.0,
        CONFIRM_YES_AT.1,
        "the confirmation box's thumb up",
    );
    frame_is_drawn(
        &c,
        &s.system,
        CONFIRM_NO_FRAME,
        CONFIRM_NO_AT.0,
        CONFIRM_NO_AT.1,
        "the confirmation box's thumb down",
    );
}

