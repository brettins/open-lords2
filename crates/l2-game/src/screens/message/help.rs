//! **Category `0x13` — the help window**, `Msg_DrawWindow`'s
//! (`0x0047309E`) table-driven arm. The numbers, the reading of the arm and
//! what the three of them do are in [`crate::message::help`]; this is the
//! paint.
//!
//! Nothing had drawn it: the five topics went on the ring as ordinary records
//! and [`message::frame_of`] had no answer for them, so each one fell through
//! to the tip painter and came up as a plain notice with one string of a
//! five-paragraph group. `docs/decisions.md` C231.

use l2_view::Canvas;

use crate::message::{self, help};
use crate::screen::Ctx;
use crate::shell::{font, Pen};

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
