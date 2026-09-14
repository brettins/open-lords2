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

/// The one line of text the strip's font draws.
///
/// `CountyStrip_Draw` uses **`Fntl2_9.pl8`**, and it is the only caller of that
/// font in the whole game (`docs/screens-county.md` §2.1). We draw it where the
/// install has it and fall back to our own 5 × 7 font where it does not, so the
/// *layout* is the original's on every machine and the *letters* are only ours
/// on a machine with no game.
///
/// `colour` is a resolved palette index
/// constants, because one of the rules here is a colour: the achieved ration is
/// **red when it differs from the wanted one**. That rule is the original's; the
/// index we spell red with is ours, out of [`Ink`].
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

/// The same, centred in `width` from `x` — `Ui_DrawCentred`, which clamps the
/// offset at zero.
///
/// Public under a longer name because the End Turn caption is drawn in this
/// font too (`Screen_DrawEndTurn`), and it is the map screen that draws it.
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

/// One line in the **body** font (`Fntl2_14.pl8`), centred in `width` from `x`.
///
/// The strip's own font is the 9-pixel one, but the county's name and the
/// three "sovereign land of …" lines are drawn with `g_fontBody`, embossed —
/// `DAT_005AEA40` is only set for the numeric block between them.
/// **This helper really does right-align, and it is OURS.** It is named after
/// `Ui_DrawNumberRight`, which does not: that function centres (C119), and the
/// resemblance is the name only. Kept because the produce rows were laid out
/// against it and changing the anchoring is a separate, visible decision.
///
/// **Flat, not embossed.** Each produce row sets `DAT_005AEA40 = 1` around its
/// number and clears it after — the same switch the strip's own figures are
/// drawn under — and that global turns `Ui_DrawText`'s emboss off.
/// **`Ui_DrawNumber` (`0x00402F64`) — a number with its sign column.**
///
/// A player: *"Happiness # and population # in the sidebar are slightly left of
/// where they should be."* **Four pixels left, both of them.
/// too.** The cause is one character:
///
/// ```c
/// Ui_NumberToBuffer(value, 1, 0);            /* digits from index 1 */
/// if (lead != '\0') g_numberBuffer = lead;   /* index 0 */
/// ... append suffix ...
/// Ui_DrawText(&g_numberBuffer, x, y, font, colour);
/// ```
///
/// `Ui_NumberToBuffer`'s `start = 1` **leaves index 0 free for a sign**, and
/// every call site fills it — the strip's three pass `' '`. So the string drawn
/// at `x` is `" 435 "`, not `"435"`, and the digits begin one space-advance to
/// the right of `x`. We drew the bare digits at the same `x` and were short by
/// exactly [`SPACE_ADVANCE`](crate::shell::font::SPACE_ADVANCE) = 4.
///
/// **The lead is a column, not padding**, and this workspace already knew that
/// one level down: `SPACE_ADVANCE`'s own doc comment says `'@'` is *"an
/// invisible sign column that still occupies its place in a column of
/// numbers"*. `Ui_DrawDelta` uses that column for `'-'`, `'+'` and `'@'` so
/// that a rising and a falling forecast line up; a plain `Ui_DrawNumber` leaves
/// it blank and keeps the same left edge. Drawing the digits without it silently
/// opts out of the alignment the whole sidebar is built on.
///
/// **The discriminating prediction, because a second cause fitted the report.**
/// Right-anchoring where the original centres would displace a two-digit
/// happiness *further* than a three-digit population. This displaces both by
/// **the same four pixels**, because a lead is one character whatever the value
/// is — and these two are `Ui_DrawNumber`, which has no anchoring argument at
/// all, so the anchoring hypothesis could not apply to them. `Ui_DrawNumberRight`
/// is the one that centres, and it is [`body_number_centred`] below.
///
/// `lead` is `'\0'` for a caller that wants no column — which the original
/// treats as *terminate immediately*, since index 0 is the NUL the buffer was
/// cleared to, so no shipped call site passes it.
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
///
/// **Only a number may be drawn here.** `Font_10.pl8` holds digits and
/// punctuation and a 2 × 2 stub where every letter goes ([`font::TEN`](crate::shell::font::TEN)),
/// so a word drawn in it paints nothing and still advances — the failure a
/// whole-canvas comparison passes straight over. The original never builds such
/// a string; the assertion is so that we cannot either.
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
/// lead in slot 0, digits, suffix, one [`ten_text`].
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
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the routine that would have. The original, in full:
///
/// ```c
/// if (value == 0 && mode == 0) return;                     /* nothing at all */
/// Ui_DrawText(prefix, x, y, font, value < 0 ? colourNeg : colourPos);
/// if      (mode == 2) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else if (value < 0) Ui_DrawNumber(-value, '-', suffix, x + g_penAdvance, …, colourNeg);
/// else if (value < 1) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else                Ui_DrawNumber( value, '+', suffix, x + g_penAdvance, …, colourPos);
/// ```
///
/// Four things in it are worth having exactly, and three of them are the sort a
/// reimplementation drops without noticing:
///
/// * **The minus is a lead *character*, not a mark.** `Ui_DrawNumber` writes it
///   over `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves
///   free for a sign, so sign and digits go out in one `Ui_DrawText`. There is
///   no separate glyph to place or to lose.
/// * **A positive value carries an explicit `'+'`.** Only the *sign* tells the
///   player which way a forecast runs; the row has no other cue.
/// * **`mode == 0` and a value of zero draw nothing whatever.** All seven
///   produce-row calls pass mode 0. That is why an absent delta has read as a
///   quiet row — a county with nothing happening
///   looks the same either way.
/// * **The colour is the sign too**: `0xFA` positive, `0xF9` negative, at every
///   one of the seven call sites.
///
/// **Seven, not eight** — three farm rows and four industry rows; the castle
/// painter has no delta. This comment said eight, and so does
/// `docs/draws-map.md` §5.5.
///
/// The prefix and the suffix are a single space at all seven — read out of
/// `Lords2.exe` at `0x004D3D40 … 0x004D3D84`, eighteen pointers that all hold
/// `" "`. They are drawn as two separate strings, so
/// [`TRAILING`](crate::shell::TRAILING)'s four pixels fall between
/// the prefix and the number and **not** between the number and its suffix.
/// Concatenating the three into one string would lose those four pixels, which
/// is the whole reason this is not a `format!`.
///
/// **The face is `&g_font10`, the seventh argument at all seven**, drawn with
/// the drop shadow its painters set ([`ten_text`]). This used to be
/// `Fntl2_9.pl8` under a comment calling it ours, because `Font_10.pl8` was not
/// loaded.
pub(crate) fn strip_delta(ctx: &Ctx, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    // `if ((value != 0) || (mode != 0))` — every produce row passes mode 0.
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { DELTA_POS };
    let lead = if value < 0 { '-' } else { '+' };
    // `Ui_DrawText(prefix, x, y, font, colour)`, then the number at
    // `x + g_penAdvance` — which is the prefix's width plus `Ui_DrawText`'s own
    // four trailing pixels, not the prefix's width alone.
    let prefix = " ";
    let advance = match ctx.assets.shell.ten.as_ref() {
        Some(f) => f.width(prefix),
        None => text::width(prefix),
    } + crate::shell::TRAILING;
    ten_text(ctx, canvas, x, y, prefix, colour);
    // `Ui_DrawNumber(|value|, lead, suffix, …, &g_font10, …)` — one string,
    // lead in slot 0.
    ten_number(ctx, canvas, value.abs(), lead, " ", x + advance, y, colour);
}

/// **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It is not right-aligned and
/// its tail is `FUN_004025D7(buf, x, y, width, font, colour)`,
/// whose whole body is
///
/// ```c
/// Ui_DrawText(str, x + max(0, (width - Ui_TextWidth(str, font)) / 2), y, font, colour);
/// ```
///
/// The name is the original's shape — `docs/symbols.json`'s
/// comment said *"Ui_DrawNumber, right-aligned inside width"* and that comment
/// is corrected on this branch. Two draw audits found it independently in the
/// same week.
/// read.
///
/// It lands here: the produce rows' stock figure was anchored at x = 540 and
/// belongs centred between 480 and 540. This helper used to be `body_right` and
/// used to do that, which is the same defect the row's missing delta was
/// reported alongside — *"grain not shown as a negative"* and *"grain in the
/// wrong place"* would have looked like one complaint.
fn body_centred_in(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, w: i32, s: &str, colour: u8) {
    let style = crate::shell::font::Style { colour, shadow: None, caps: None };
    body_centred_styled(ctx, canvas, x, y, w, s, style);
}

/// Centred in `width` from `x`, with the emboss pair chosen by the caller — because
/// `CountyStrip_Draw` uses **two different ones** in the same plate. See
/// [`crate::shell::font::SHADOW_GREY`].
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

