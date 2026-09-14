//! **Category `0x13` — the help window**, `Msg_DrawWindow`'s
//! (`0x0047309E`) table-driven arm. The numbers, the reading of the arm and
//! what the three of them do are in [`crate::message::help`]; this is the
//! paint.
//!
//! Nothing had drawn it: the five topics went on the ring as ordinary records
//! and [`message::frame_of`] had no answer for them, so each one fell through
//! to the tip painter and came up as a plain notice with one string of a
//! five-paragraph group. `docs/decisions.md` C231.
//!
//! Two differences from the original are on purpose and neither is visible:
//!
//! * **the OK button is drawn last**, not before the text. The arm draws it
//!   second; the corner it draws in is `(x + w - 0x30, y + h - 0x30)`, which no
//!   paragraph reaches, so the order cannot show. Ours comes from
//!   [`super::MessageScreen::draw`]'s one call for every arm.
//! * **the heading is `Ui_DrawCentred` in `&g_fontHeading`** and so is
//!   [`Pen::heading_centred`], not [`Pen::eng_centred`] — that one is the body
//!   face, and this call site names the other.

use l2_view::Canvas;

use crate::message::{self, help};
use crate::screen::Ctx;
use crate::shell::{font, Pen};

/// The heading, then `paragraphs(group)` wrapped paragraphs stepping down by
/// their own line count plus [`help::PARAGRAPH_GAP`].
///
/// [`Pen::body_wrapped`] returns the height it painted — sixteen pixels a line,
/// which is what `FUN_0040328E` adds to `DAT_005CD4F8` itself — so the
/// accumulator here is the original's to the pixel.
pub(super) fn draw_help(
    pen: &Pen,
    ctx: &Ctx,
    canvas: &mut Canvas,
    record: &message::Record,
    f: message::Frame,
) {
    let shell = &ctx.assets.shell;
    let (hx, hy, hw) = help::heading(f);
    pen.heading_centred(canvas, hx, hy, hw, &help::words(shell, record.group, 0), font::TEXT);

    let (x, top, width) = help::body(f);
    let mut at = top;
    for i in 1..=help::paragraphs(record.group) {
        let words = help::words(shell, record.group, i);
        at += pen.body_wrapped(canvas, x, at, width, &words, font::TEXT) + help::PARAGRAPH_GAP;
    }
}
