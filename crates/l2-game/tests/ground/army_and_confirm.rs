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

/// **The division rows sit on parchment, not on a hole.**
///
/// `Screen_SplitArmyRows`' first statement is `Ui_DrawBoxInterior(0x18, 0x80,
/// 0x1A, 0x12)` — the 12 × 12 texture field at `Panels.pl8` frame `0x34`, tiled,
/// with no border. This drew `fill_rect(well, ink.background)` with an outline of
/// ours over it: under our palette a dark plate that reads as deliberate, and
/// under the game's a black rectangle in the middle of the window.
///
/// The expected frame is `TEXTURE + col % 12 + (row % 12) * 12` at the well's
/// own origin, which is `Ui_DrawBoxInterior`'s tiling and not `Ui_DrawBox`'s —
/// the box insets its interior by a cell and this primitive does not, so
/// starting the tiling in the wrong place is a real way to be wrong here.
///
/// **Ablated** two ways: deleting the `pen.box_interior` call, which leaves the
/// window's own interior tiling underneath — a *different* phase of the same
/// texture, because `Ui_DrawBox` insets its interior by a cell and this
/// primitive does not, so the probe disagrees on the first cell; and narrowing
/// `ROWS_WELL_COLS` to 0x19, which turns [`pinned`] red as well.
#[test]
fn the_army_division_rows_sit_on_the_parchment_field() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Divide(0), &mut g, &a);
    let (wx, wy) = (DIVIDE_WELL_AT.0, DIVIDE_WELL_AT.1);
    // **The probes are the two bottom cell rows, and that is not arbitrary.**
    // The well is 26 × 18 cells and the eight troop rows are painted over the
    // top of it — nouns at x 0x18, icons at 0xA8 and 0x158, numbers at 0xD8 and
    // 0x188, arrow pairs at y 120 + 32n — so a probe in the upper two thirds
    // reads a glyph or a sprite
    // test probed cell (0, 0) and read `0x10`, which is the body font's own
    // shadow colour under `Ui_DrawUnitNoun(2, 0x34, 0x18, 0x80)`: the parchment
    // was there all along and the probe was on top of the first row's word.
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

// ------------------------------------------------------------- the yes/no box

/// **The confirmation box is the shared ground and the mailed hands.**
///
/// Two claims, and the second is the one a canvas diff would have missed.
///
/// `Screen_ConfirmBox` (`0x0040CCFA`) is three statements: `FUN_004093E0(x −
/// 0x10, y − 0x10, 0xE, 8)`, an `Eng_DrawString`, and the widget pass. The
/// ground was a `fill_rect(ink.background)` here.
///
/// And `g_confirmWidgets` (`0x004DD310`) carries frames **29 and 31** — a mailed
/// hand thumb up and thumb down. This drew `system::OK` and `system::OK + 2`,
/// which are the close corner and its neighbour: **the right sheet, the wrong
/// frames, at the right coordinates.** Nothing that compares two canvases of our
/// own could tell; only the sheet can.
///
/// **Ablated** by putting `system::OK` (51) back as `CONFIRM_YES_FRAME`, which
/// fails on the thumb's first opaque pixel and in [`pinned`]; and by moving
/// `BOX_SET` to 0, which fails on the corner and in [`pinned`].
#[test]
fn the_yes_no_box_is_the_shared_ground_and_the_thumb_pair() {
    let (mut g, a, s) = world!();
    let mut c = Canvas::screen();
    // The box is drawn by the battlefield's painter while a prompt is up, and
    // — so this draws the two calls directly, at the
    // module's own constants, which is what the painter passes them.
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
    // The frames the painter passes, from the crate — so that a wrong frame in
    // the crate is drawn here and then caught below against the pinned literal.
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

