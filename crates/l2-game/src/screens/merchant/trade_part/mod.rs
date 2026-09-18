#![allow(unused_imports)]

mod render;
pub use render::*;
mod trade_screen;
pub use trade_screen::*;

use super::*;
use super::merchant::*;
use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `FUN_004093E0(0x30, 0x40, 0x22, 0x10)`.
pub const PANEL: Rect = Rect::new(0x30, 0x40, 0x22 * 16, 0x10 * 16);
pub const PANEL_OK: Rect = Rect::new(0x22C, 0x114, 24, 24);
pub const ICON_WELL: Rect = Rect::new(0x44, 0x66, 0x32, 0x32);
pub const ICON_AT: (i32, i32) = (0x45, 0x67);

pub const HEADING_AT: (i32, i32) = (0x88, 0x68);
pub const ALE_DELTA_DX: i32 = 0x90 - HEADING_AT.0;
pub const ALE_DELTA_Y: i32 = 0x6E;
pub const HAPPINESS_FACE: usize = 0x17;
pub const HAPPINESS_CLOSE_DX: i32 = 0x14;

pub const PRICE_AT: (i32, i32) = (0x88, 0x88);
pub const HAVE_AT: (i32, i32) = (0x58, 0xB0);
pub const HAVE_ICON_Y: i32 = 0xAA;
pub const HAVE_ICON_ADVANCE: i32 = 0x24;

pub const ADVICE_WELL: Rect = Rect::new(0x50, 0xD0, 0x1D0, 0x4C);
pub const ADVICE_CELLS: (i32, i32) = (0x1D, 5);
pub const QTY_AT: (i32, i32) = (0x60, 0xE0);
pub const TOTAL_AT: (i32, i32) = (0x100, 0xE0);
pub const ASK_AT: (i32, i32) = (0x60, 0x100);
pub const ADVICE_X: i32 = 0x60;
pub const ADVICE_Y: i32 = 0xF6;
pub const ADVICE_W: i32 = 0x1C0;

/// `Widget_Draw(0x30, 0x50, &DAT_004DD838, …)` — every widget's stored position
/// is relative to this.
pub const WIDGET_ORIGIN: (i32, i32) = (0x30, 0x50);

/// The six widgets of `0x004DD838`, in table order, at their stored positions.
const WIDGETS: [(i32, i32, i32); 6] = [
    (96, 136, 24),  // frame 21, up      -> FUN_00435339
    (120, 136, 24), // frame 23, down    -> FUN_0043543D
    (152, 136, 24), // frame 72, ceiling -> FUN_004355DB
    (176, 136, 24), // frame 70, floor   -> FUN_00435541
    (272, 168, 32), // frame 29, tick    -> FUN_00435286
    (312, 168, 32), // frame 31, cross   -> FUN_004352F2
];

pub(super) fn widget_rect(i: usize) -> Rect {
    let (x, y, s) = WIDGETS[i];
    Rect::new(WIDGET_ORIGIN.0 + x, WIDGET_ORIGIN.1 + y, s, s)
}

pub fn up_button() -> Rect {
    widget_rect(0)
}
pub fn down_button() -> Rect {
    widget_rect(1)
}
pub fn ceiling_button() -> Rect {
    widget_rect(2)
}
pub fn floor_button() -> Rect {
    widget_rect(3)
}
pub fn confirm_button() -> Rect {
    widget_rect(4)
}
pub fn cancel_button() -> Rect {
    widget_rect(5)
}

/// **`DAT_004DD838` as a table: six kind-4 records**, and `DAT_00553F58` of
/// them live — four with nothing agreed, six with a quantity pending.
///
/// `node tools/oracle/kinds.js` files all six handlers under `widget 4`. `[V]`
/// So every one fires on the press, clicks, shows `base + 1`, and repeats while
/// held; this screen answered raw clicks, so none of them clicked, drew a
/// pressed picture or repeated — and the thumbs up and down at the bottom are
/// the same mailed hands as every yes/no in the game.
fn trade_widgets(pending: bool) -> Vec<Widget> {
    let mut out = vec![
        Widget::new(up_button(), crate::arm!("0x00435339/trade-more", Repeat)),
        Widget::new(down_button(), crate::arm!("0x0043543D/trade-less", Repeat)),
        Widget::new(ceiling_button(), crate::arm!("0x004355DB/trade-all", Repeat)),
        Widget::new(floor_button(), crate::arm!("0x00435541/trade-none", Repeat)),
    ];
    if pending {
        out.push(Widget::new(confirm_button(), crate::arm!("0x00435286/trade-agree", Repeat)));
        out.push(Widget::new(cancel_button(), crate::arm!("0x004352F2/trade-refuse", Repeat)));
    }
    out
}

/// **`if (DAT_00591554 < 0x2C)` — the repeat step from which the trade arrows
/// move ten at a time.** `FUN_00435339` and `FUN_0043543D` both read it, and it
/// is `0` on the press. `[V]` See [`crate::press::Press::repeat_step`].
pub const TRADE_FAST_STEP: u8 = 0x2C;

pub struct TradeScreen {
    unit: usize,
    good: Good,
    /// `DAT_00554170` — signed: positive buys, negative sells.
    qty: i32,
    /// `DAT_0057C994` — 1 when the ceiling refused, -1 when the floor did, and
    /// only when the limit was itself zero.
    limit: i32,
    status: String,
    /// `DAT_004DD838`'s press timers and repeat counter. See [`trade_widgets`].
    press: Press,
}

