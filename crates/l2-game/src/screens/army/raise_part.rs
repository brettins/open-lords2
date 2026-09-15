#![allow(unused_imports)]
use super::*;

use raise::*;
use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::armoury;
use crate::shell::{self, font, Pen};
use crate::widget;
use crate::screens::armoury::Raised;


pub const fn base(offer: bool) -> i32 {
    if offer {
        0x80
    } else {
        0xA0
    }
}

pub const fn rows(offer: bool) -> i32 {
    if offer {
        0x10
    } else {
        0x0D
    }
}

pub fn window(offer: bool) -> Rect {
    Rect::new(BOX_X, base(offer) - 0x10, BOX_COLS * 16, (rows(offer) + 1) * 16)
}

pub fn rack_row(offer: bool) -> i32 {
    base(offer) + (rows(offer) - 4) * 0x10
}

pub fn footer_row(offer: bool) -> i32 {
    (rows(offer) - 2) * 0x10 + base(offer) + 4
}

pub fn county_name_index(ctx: &Ctx, county: u8) -> usize {
    ctx.game.map_slot * COUNTY_NAMES_STRIDE + county as usize
}

pub fn merc_well(offer: bool) -> Rect {
    Rect::new(0x70, base(offer) + 0x58, 0x1A0, if offer { 0x60 } else { 0x32 })
}

/// `Widget_Draw(0, yOffset, &DAT_004DD340, …)` — the offset the whole table is
/// drawn and tested at. `0x10` with an offer, `0` without.
pub fn widget_offset(offer: bool) -> i32 {
    if offer {
        0x10
    } else {
        0
    }
}

/// **The Continue button** — record 0, frame 33, 24 pixels, at (480, 336) plus
/// the table offset. `FUN_00435CBF` sets `g_screenId = 0x0A` and re-seeds the
/// basket.
pub fn continue_button(offer: bool) -> Rect {
    Rect::new(480, 336 + widget_offset(offer), 24, 24)
}

pub fn hire_yes(offer: bool) -> Rect {
    Rect::new(352, 256 + widget_offset(offer), 32, 32)
}

pub fn hire_no(offer: bool) -> Rect {
    Rect::new(400, 260 + widget_offset(offer), 32, 32)
}

/// **`DAT_004DD340` as a table: three kind-5 records**, and
/// `DAT_00522F58` of them live — 3 on the affordable branch, 1 otherwise.
///
/// `node tools/oracle/kinds.js` files `RaiseArmy_Continue` and
/// `RaiseArmy_HireToggle` under `widget 5`. `[V]` So Continue, the tick and the
/// cross all go down on the press and act twenty frames later; this screen
/// answered raw clicks, so all three acted at once with no picture and no
/// click. Index 0 is Continue, 1 the tick, 2 the cross. Each `arm!` is the
/// marker and the kind.
pub(super) fn widgets(offer: bool, affordable: bool) -> Vec<Widget> {
    let mut out = vec![Widget::new(
        continue_button(offer),
        crate::arm!("0x00435CBF/raise-army-continue", Delayed),
    )];
    if offer && affordable {
        out.push(Widget::new(hire_yes(offer), crate::arm!("0x00435C89/hire-yes", Delayed)));
        out.push(Widget::new(hire_no(offer), crate::arm!("0x00435C89/hire-no", Delayed)));
    }
    out
}

pub fn hire_readout(offer: bool) -> Rect {
    Rect::new(0x1D0, base(offer) + 0x98 - 2, 0x40, 20)
}

pub struct RaiseArmyScreen {
    pub(crate) county: u8,
    pub(crate) status: String,
    /// **It is the global, not the widget table's business.** `WM_LBUTTONDOWN`
    /// sets `DAT_004EABC2` whatever the press landed on, so a press that
    /// `Widget_Test` consumed — Continue, the tick, the cross — still sets it,
    /// and sliding from Continue onto the track moves the knob. That is why
    /// this is written before the table is asked and not in its `else`.
    ///
    /// **A double click does not set it.** The window procedure's
    /// `WM_LBUTTONDBLCLK` arm (`0x203`) sets only `DAT_004EADA1`, the
    /// double-click flag; the down bit in `DAT_004EABC2` is set by
    /// `WM_LBUTTONDOWN` alone, and the first click's `WM_LBUTTONUP` has already
    /// cleared it. `[V]`, `0x004B29BE`.
    pub(crate) left_down: bool,
    /// `DAT_004DD340`'s press timers. See [`widgets`].
    pub(crate) press: Press,
}

