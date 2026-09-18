#![allow(unused_imports)]
use super::*;
use super::trade_screen::*;
use super::*;
use super::merchant::*;
use l2_kingdom::trade::{self, Good, Order, Quote, Refusal};
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `Ui_DrawBevelRect` (`0x00403FDD`) — **four clipped lines and no fill**, and
/// it is [`crate::shell::inset_rect`] with its two colours the other way round:
pub fn bevel_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const TOP_RIGHT: u8 = 0x1F;
    const BOTTOM_LEFT: u8 = 0x10;
    if w < 1 || h < 1 {
        return;
    }
    canvas.fill_rect(x, y, w, 1, TOP_RIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, TOP_RIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, BOTTOM_LEFT);
    canvas.fill_rect(x, y, 1, h, BOTTOM_LEFT);
}

/// `Ui_DrawHappinessDelta` (`0x0041AC95`) — **four draws**: an opening bracket,
/// `Ui_DrawDelta` in mode 1, `Misc_cty.pl8` frame `0x17` and a closing bracket.
///
/// **On the trade screen it is the ale preview**, and only there: the whole
/// call is `if (good == 4 && g_alePreviewHappiness != 0)`. It reads
/// `g_alePreviewHappiness` (`0x0053E8C0`), which `Ale_PreviewGain`
/// (`0x00435673`) writes from the *pending* crown total — so the number beside
/// *"Ale"* is what the purchase would buy, before it is made, and it is capped
/// at `5 - county.aleHappinessGiven`. It is the one place a county field is
/// shown on a screen that is otherwise entirely about the realm's purse.
pub fn happiness_delta(pen: &Pen, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    let after = pen.body(canvas, x, y, "(", font::TEXT) - shell::TRAILING;
    let colour = if value < 0 { ASK_BUY_COLOUR } else { font::TEXT };
    let sign = if value < 0 { '-' } else { '+' };
    let after = pen.body(canvas, after, y, &format!("{sign}{}", value.abs()), colour);
    pen.misc_frame(canvas, HAPPINESS_FACE, after, y - 2);
    pen.body(canvas, after + HAPPINESS_CLOSE_DX, y, ")", font::TEXT);
}

