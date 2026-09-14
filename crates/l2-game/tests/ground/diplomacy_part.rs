#![allow(unused_imports)]
use super::*;
use super::pinned_part::*;
use super::army_and_confirm::*;
use l2_formats::pl8::DecodedFrame;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battlefield, diplomacy, divide};
use l2_game::Game;
use l2_view::chrome::panels;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// **The other lords sits on `Panels.pl8`, in border set 1.**
///
/// `Diplo_DrawScreen`'s first draw is `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)`,
/// and `FUN_004093E0` is `Ui_DrawBoxBorder(**1**, …)` — so the corner at the
/// box's origin is `Panels.pl8` frame `CORNER_TL + 0xCC` and not frame 0. This
/// painter drew a `widget::panel` there: a flat `ink.panel` fill with a
/// one-pixel `ink.border` round it.
///
/// The **set** is half the claim and it has bitten this project once already:
/// adding `0xCC` to an *interior* index sends it past 255 and into the banner
/// frames, which is how the custom-game screen's option boxes came out full of
/// shields. Asserting the corner
/// visible at all.
///
/// **Ablated** two ways: `pen.window(…, WINDOW_SET)` back to
/// `widget::panel(canvas, ink, WINDOW)` — every corner pixel becomes `ink.panel`
/// or `ink.border`; and `WINDOW_SET` from 1 to 0, which fails on the first
/// corner pixel that differs between the two sets.
#[test]
fn the_diplomacy_window_is_panels_pl8_in_border_set_one() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.panels,
        PANELS_CORNER_TL + PANELS_SET_B,
        DIPLO_WINDOW_AT.0,
        DIPLO_WINDOW_AT.1,
        "the diplomacy window's top-left corner",
    );
}

/// **A lord card is `Ui_DrawInsetRect`: four lines and no fill.**
///
/// `Diplo_DrawLordCard` opens with `Ui_DrawInsetRect(0x30, slot*100 + 0x31,
/// 0x52, 0x4E)` — colour `0x10` along the top and right edges and `0x1F` along
/// the bottom and left, and *nothing in the middle*. This screen drew a filled
/// `widget::panel` there, which is the same shape as the armoury's black hole:
/// the fill is invisible under our own palette, where the interface's panel
/// colour is the parchment colour, and is a rectangle of black under
/// `Panels.pl8`'s.
///
/// The two probes are the **left** and **top** edges, and they are chosen so
/// nothing can overwrite them: the portrait is blitted at `(r.x + 1, r.y + 1)`,
/// one pixel inside, so a sprite of any size leaves column `r.x` and row `r.y`
/// alone. The colours `0x1F` and `0x10` are written out here as literals from
/// `Ui_DrawInsetRect` (`0x00403DEB`)
/// because `docs/agents.md`'s first way to ablate wrongly is computing the probe
/// from the constant being ablated.
///
/// **Ablated** by deleting `pen.inset(canvas, r)`: both probes become the
/// window's parchment.
#[test]
fn a_lord_card_is_an_inset_and_not_a_plate() {
    let (mut g, a, _s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    for slot in 0..2i32 {
        let (rx, ry, rw, rh) =
            (CARD_AT.0, slot * CARD_PITCH + CARD_AT.1, CARD_AT.2, CARD_AT.3);
        assert_eq!(
            px(&c, rx, ry + rh / 2),
            INSET_BOTTOM_LEFT,
            "card {slot}'s left edge at ({}, {}) is not Ui_DrawInsetRect's 0x1F",
            rx,
            ry + rh / 2,
        );
        assert_eq!(
            px(&c, rx + rw / 2, ry),
            INSET_TOP_RIGHT,
            "card {slot}'s top edge at ({}, {}) is not Ui_DrawInsetRect's 0x10",
            rx + rw / 2,
            ry,
        );
    }
}

/// **The menu has one recess behind all of it, and the height is the layout's.**
///
/// `Diplo_DrawScreen` draws `Ui_DrawInsetRect(0xD0, 0x60, 0xE8, h)` once, with
/// `h` `0xD0` on the no-ally layout, `0x130` when allied to this lord and `0xA0`
/// when allied elsewhere — and **nothing at all** on the fourth. This painter
/// drew a filled box *per menu row* instead, which is a shape the original does
/// not have anywhere on the screen.
///
/// The bottom edge is the probe because it is the only thing that moves when the
/// height is wrong, and a per-row box could never put `0x1F` there.
///
/// **Ablated** by returning `Some(0xE0)` from `Menu::inset_height`: the bottom
/// edge lands sixteen pixels lower and the probe reads the window's parchment.
#[test]
fn the_diplomacy_menu_has_one_recess_and_it_is_the_layouts_height() {
    let (mut g, a, _s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    // Nobody is allied in `rivals()`, so this is the no-ally layout, whose
    // recess is 0xD0 tall. Every number here is the painter's, not the crate's.
    let (ix, iy, _iw, ih) = MENU_INSET_AT;
    let bottom = iy + ih - 1;
    let x = ix + 4;
    assert_eq!(
        px(&c, x, bottom),
        INSET_BOTTOM_LEFT,
        "the recess's bottom edge at ({x}, {bottom})",
    );
    assert_eq!(px(&c, ix, iy + 4), INSET_BOTTOM_LEFT, "and its left edge");
}

/// **The six menu pictures are `System.pl8` frame 64**, which nothing in this
/// tree drew: the menu widgets were filled rectangles of ours.
///
/// The frame is not a guess. `g_diploWidgets` (`0x004DD940`) is six 24-byte
/// records and every one carries 64 at `+4`, read out of the executable by
/// `tools/oracle/widgets.js widgets 4dd940 6`.
///
/// **Ablated** by moving `MENU_FRAME` to 65 — and the first time, with the test
/// reading `diplomacy::MENU_FRAME` for its own expectation, **it stayed green**.
/// It now reads the pinned literal, and the same ablation turns this test *and*
/// [`pinned`] red. That is the case a canvas diff cannot see, and it took two
/// goes to build a test that could.
#[test]
fn the_diplomacy_menu_widget_is_system_frame_64() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.system,
        MENU_WIDGET_FRAME,
        MENU_WIDGET_AT.0,
        MENU_WIDGET_AT.1,
        "the first menu widget",
    );
}

/// **The corner is `Ui_OkButton`'s picture and not the word OK.**
///
/// `Ui_OkButton(0x1A8, 0x1A6, 0)` draws `System.pl8` frame `0x33`, which decodes
/// to a cursor arrow pointing into a small black hole — among the darkest frames
/// in the sheet. This screen drew the two letters `OK` in our 5 × 7 debug font,
/// the last of the three `Ui_OkButton` inventions the draw audit found, and
/// `tools/draws/screens.json` named it in `literals_ours` until this pass.
///
/// **Ablated** by putting `widget::button(canvas, ink, OK, "OK", true)` back:
/// the frame's opaque pixels land on our own rectangle instead.
#[test]
fn the_diplomacy_corner_is_the_ok_picture_and_carries_no_letters() {
    let (mut g, a, s) = world!();
    let c = frame_of(ScreenId::Diplomacy, &mut g, &a);
    frame_is_drawn(
        &c,
        &s.system,
        OK_FRAME,
        DIPLO_OK_AT.0,
        DIPLO_OK_AT.1,
        "the diplomacy screen's Ui_OkButton",
    );
}

// ------------------------------------------------------------ army division

