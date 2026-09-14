#![allow(unused_imports)]
use super::*;
use super::panel::*;
use super::screen::*;
use super::*;
use super::layout::*;
use super::strip::*;
use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};
use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

/// One `L2.eng` string, from the install if it has one and from our own
/// transcription if it does not.
///
/// # `Ui_DrawNumberRight` centres, and its name is a false claim
///
/// Recorded here because it is what this panel's five numbers depend on and two
/// branches found it independently within a day. It is `Ui_NumberToBuffer`
/// followed by `FUN_004025D7`, which is
/// `Ui_DrawText(s, x + max(0, (width - textWidth) / 2), y, …)` — and
/// `Ui_DrawCentred` calls **the same function**. One alignment, two names, one
/// of them true. This module right-aligned these columns because the symbol said
/// *right*.
///
/// **The symbol's entry is `[V]` and its comment says the opposite of its
/// body**, so the verification carried the error: a wrong name with a wrong
/// verified comment is believed twice — once for the name and once for the tier
/// — with nothing left to contradict it. `[V]` records that somebody read it,
/// not that somebody read it correctly. Twenty call sites in the original
/// inherit it and **eighteen beyond this panel are unaudited.**
pub(crate) fn line_text(ctx: &Ctx, l: Line) -> String {
    eng(ctx, l.group, l.index, l.ours)
}

pub(crate) fn ration_name(level: i32) -> &'static str {
    RATION_NAMES
        .get(level.clamp(0, RATION_LEVEL_COUNT as i32 - 1) as usize)
        .copied()
        .unwrap_or("?")
}

/// `Eng_DrawString(20, healthBand, …)` — the five health words, with
/// [`HEALTH_BAND_NAMES`] as the fallback.
pub(super) fn health_label(ctx: &Ctx, band: u8) -> String {
    let ours = HEALTH_BAND_NAMES.get(band as usize).copied().unwrap_or("?");
    eng(ctx, GROUP_HEALTH_BANDS, band as usize, ours)
}

/// A `Ui_DrawNumber(v, '@', "", 0x150, y, heading)` row: the label in the left
/// column and the value **left-aligned from x = 336**, both in the 22-pixel
/// font, which is what the three plain rows on the two graph panels are.
pub(crate) fn heading_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
}

/// The same with `Pl8_DrawFrame(Misc_cty, 0x17, pen + 0x150, y + 3)` after it —
/// the happiness panel's *Last season* and *This Season* rows both carry the
/// face, three pixels below the text's own line.
pub(crate) fn heading_row_face(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    let x = pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
    pen.misc_frame(canvas, FRAME_FACE, x, y + 3);
}

/// `Ui_DrawDelta(value, 0, "", "", 0x150, y, body, 0x3F, 0xF9)` — the label and
/// the value.
pub(crate) fn delta_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.body(canvas, LABEL_X, y, label, font::TEXT);
    delta_value(pen, canvas, y, value);
}

/// The value half on its own, for the emigration row, which puts a county name
/// between the label and the number.
///
/// Three things it is easy to get wrong:
/// **a zero draws nothing at all** — mode 0 with `value == 0` returns before the
/// first `Ui_DrawText` — the digits are **left-aligned from `x`**
/// right-anchored to it, and the empty prefix still advances the pen by
/// [`TRAILING`], so they start four pixels right of it.
pub(super) fn delta_value(pen: &Pen, canvas: &mut Canvas, y: i32, value: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = if value < 0 { '-' } else { '+' };
    pen.body(canvas, VALUE_LEFT + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
}

/// `Ui_DrawHappinessDelta` (`0x0041AC95`), whole:
///
/// ```text
///   Ui_DrawText("(", x, y, font, 0x3F)              pen = w("(") + 4
///   pen -= 4
///   Ui_DrawDelta(value, 1, "", "", x + pen, y, font, colourPos, colourNeg)
///   Pl8_DrawFrame(g_miscCtySheet, 0x17, x + pen, y - 2)
///   Ui_DrawText(")", x + pen + 0x14, y, font, 0x3F)
/// ```
///
/// So it is `( ±n ☺ )`, the bracket sits at `x`, and the closing bracket is
/// [`FACE_W`] = 20 pixels past the face — which is exactly the face's own width
/// in `Misc_cty.pl8`, so the gap is the picture's and not a guess. **Mode 1
/// never suppresses a zero**, unlike the panel rows: a zero draws `0` with the
/// blank sign column.
pub(crate) fn happiness_delta(pen: &Pen, canvas: &mut Canvas, x: i32, y: i32, value: i32) {
    let after_bracket = pen.body(canvas, x, y, "(", font::TEXT) - TRAILING;
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = match value.signum() {
        -1 => "-",
        1 => "+",
        _ => "",
    };
    let face_x =
        pen.body(canvas, after_bracket + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
    pen.misc_frame(canvas, FRAME_FACE, face_x, y - 2);
    pen.body(canvas, face_x + FACE_W, y, ")", font::TEXT);
}


