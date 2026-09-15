#![allow(unused_imports)]
use super::*;
use super::draw::*;
use super::*;
use super::layout::*;
use super::draw::*;
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

fn strip_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            // `CountyStrip_Draw` sets `DAT_005AEA40 = 1` for the whole numeric
            // block and clears it after, and that global switches
            // `Ui_DrawText`'s emboss **off**. The strip's numbers are flat.
            let style = crate::shell::font::Style { colour, shadow: None, caps: None };
            f.draw(canvas, x, y, s, &style);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

pub fn strip_centred_at(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    s: &str,
    colour: u8,
) {
    strip_centred(ctx, canvas, x, y, width, s, colour)
}

pub(crate) fn strip_centred(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) {
    let w = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    };
    strip_text(ctx, canvas, x + ((width - w) / 2).max(0), y, s, colour);
}

/// The strip's own font is the 9-pixel one, but the county's name and the
/// three "sovereign land of …" lines are drawn with `g_fontBody`, embossed —
/// `DAT_005AEA40` is only set for the numeric block between them.
///
/// **This helper really does right-align, and it is OURS.** It is named after
/// `Ui_DrawNumberRight`, which does not: that function centres (C119), and the
/// resemblance is the name only. Kept because the produce rows were laid out
/// against it and changing the anchoring is a separate, visible decision.
///
/// **Flat, not embossed.** Each produce row sets `DAT_005AEA40 = 1` around its
/// number and clears it after — the same switch the strip's own figures are
/// drawn under — and that global turns `Ui_DrawText`'s emboss off.
///
/// **`Ui_DrawNumber` (`0x00402F64`) — a number with its sign column.**
///
/// `Ui_DrawNumber(value, lead, suffix, x, y, &g_fontSmall, colour)` — the
/// numeric block's population, happiness and tax rate, which are flat
/// (`DAT_005AEA40 = 1`). The jobs plate's numbers are [`ten_number`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn strip_number(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    colour: u8,
) {
    strip_text(ctx, canvas, x, y, &format!("{lead}{value}{suffix}"), colour);
}

/// **`Ui_DrawText(s, x, y, &g_font10, colour)` with `g_dropShadow` set** — how
/// every number on the jobs plate is drawn. **[V]**
///
/// All nine `&g_font10` call sites in the image are inside the eight row
/// painters, and every one of those sets `g_dropShadow = 1` on entry with
/// `DAT_005AEA40` already clear, so the face and the shadow always travel
/// together; see [`font::DROP_SHADOW_COLOUR`](crate::shell::font::DROP_SHADOW_COLOUR).
fn ten_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    debug_assert!(
        !s.chars().any(|c| c.is_ascii_alphabetic()),
        "{s:?} drawn in Font_10.pl8, whose letters are 2x2 stubs: words on the strip are g_fontSmall"
    );
    match ctx.assets.shell.ten.as_ref() {
        Some(f) => {
            f.draw_dropped(canvas, x, y, s, colour);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// `Ui_DrawNumber(value, lead, suffix, x, y, &g_font10, colour)` (`0x00402F64`):
#[allow(clippy::too_many_arguments)]
pub(crate) fn ten_number(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    colour: u8,
) {
    ten_text(ctx, canvas, x, y, &format!("{lead}{value}{suffix}"), colour);
}

/// **`&g_fontSmall` with `g_dropShadow` set** — the castle cell's two captions,
/// `Ui_DrawUnitNoun`'s *"Season(s)"* and `L2.eng` 71/18 *"Needed"*, which
/// `CountyStrip_DrawCastleIcon` draws inside the same `g_dropShadow = 1` as its
/// number. The rest of the strip's `&g_fontSmall` text is flat ([`strip_text`]).
pub(crate) fn small_dropped(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            f.draw_dropped(canvas, x, y, s, colour);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// **`Ui_DrawNumberRight` (`0x004030C6`) — the same buffer, laid out in a
/// width.** Its tail is `FUN_004025D7`, which **centres**; see
/// [`body_centred_in`]. The lead column is `Ui_DrawNumber`'s, so it widens the
/// string and moves the digits half a space right of a bare centring.
pub(super) fn body_number_centred(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    w: i32,
    colour: u8,
) {
    body_centred_in(ctx, canvas, x, y, w, &format!("{lead}{value}{suffix}"), colour);
}

/// **`Ui_DrawDelta` (`0x00402E0C`) — the produce rows' signed forecast.**
///
/// The prefix and the suffix are a single space at all seven — read out of
/// `Lords2.exe` at `0x004D3D40 … 0x004D3D84`, eighteen pointers that all hold
/// `" "`. They are drawn as two separate strings, so
/// [`TRAILING`](crate::shell::TRAILING)'s four pixels fall between
/// the prefix and the number and **not** between the number and its suffix.
pub(crate) fn strip_delta(ctx: &Ctx, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { DELTA_POS };
    let lead = if value < 0 { '-' } else { '+' };
    let prefix = " ";
    let advance = match ctx.assets.shell.ten.as_ref() {
        Some(f) => f.width(prefix),
        None => text::width(prefix),
    } + crate::shell::TRAILING;
    ten_text(ctx, canvas, x, y, prefix, colour);
    ten_number(ctx, canvas, value.abs(), lead, " ", x + advance, y, colour);
}

/// **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It is not right-aligned and
/// its tail is `FUN_004025D7(buf, x, y, width, font, colour)`,
/// whose whole body is
fn body_centred_in(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, w: i32, s: &str, colour: u8) {
    let style = crate::shell::font::Style { colour, shadow: None, caps: None };
    body_centred_styled(ctx, canvas, x, y, w, s, style);
}

pub(crate) fn body_centred_styled(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: i32,
    s: &str,
    style: crate::shell::font::Style,
) {
    match ctx.assets.shell.body.as_ref() {
        Some(f) => {
            f.draw_centred(canvas, x, y, w, s, &style);
        }
        None => {
            text::draw_centred(canvas, x + w / 2, y, s, style.colour);
        }
    }
}

