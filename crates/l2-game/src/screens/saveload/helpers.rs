#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::tests::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// **`g_saveLoadWidgets` as a table, and all four records are kind 4** —
/// `node tools/oracle/kinds.js` files `FUN_004342F3`, `SaveLoad_Cancel` and
/// `SaveLoad_Scroll` under `widget 4`, read out of `+0x0F` of `0x004DDD78` …
/// `0x004DDDC0`. `[V]`
///
/// So the thumb up **acts on the press**, clicks, and shows `base + 1` — and a
/// player who says it *"is still on mousedown instead of mouseup"* is feeling
/// something real that is not the gesture: the handler only arms a latch, and
/// the save happens 150 frames later ([`WORK_FRAMES`]). This screen used to
/// answer a raw click, so it did neither the click nor the wait.
///
/// Each `arm!` is the marker and the kind. The two scroll arrows are one
/// handler, `SaveLoad_Scroll`, told apart by the hotspot id, so they share one.
pub(super) fn widgets() -> [Widget; 4] {
    const SCROLL: crate::press::Kind = crate::arm!("0x00434346/saveload-scroll", Repeat);
    [
        Widget::new(widget_rect(CONFIRM), crate::arm!("0x004342F3/saveload-confirm", Repeat)),
        Widget::new(widget_rect(CANCEL), crate::arm!("0x00434308/saveload-cancel", Repeat)),
        Widget::new(widget_rect(SCROLL_UP), SCROLL),
        Widget::new(widget_rect(SCROLL_DOWN), SCROLL),
    ]
}

/// Where the save directory is written: inside the box's bottom edge.
pub(super) const DIR_LINE: (i32, i32) = (BOX_X + 4, BOX_Y + BOX_ROWS * 16 - 12);

/// Everything we put on this screen that the original does not say is prefixed,
///
pub(super) fn ours(detail: &str) -> String {
    format!("OURS: {detail}")
}

/// The directory line, **cut from the left** when it is wider than the box.
///
/// Ours, like the line itself — the original has no directory to show. It was
/// drawn whole, and a directory longer than about 67 characters ran straight
/// out of the side of the box: a `LORDS2_SAVES` on a deep path, or the default
/// under a profile whose user name is longer than 24. Nobody saw it because
/// the only test of the box's edges ran with a short temporary directory, and
/// it went red the day that directory got longer. The left is what goes
/// because the right is the part that says which directory this is.
pub(super) fn directory_line(dir: &str) -> String {
    let room = BOX_X + BOX_COLS * 16 - 4 - DIR_LINE.0;
    let whole = ours(dir);
    if text::width(&whole) <= room {
        return whole;
    }
    let chars: Vec<char> = dir.chars().collect();
    let fixed = ours("...").chars().count() as i32;
    let keep = ((room + 1) / text::ADVANCE - fixed).max(0) as usize;
    let tail: String = chars[chars.len().saturating_sub(keep)..].iter().collect();
    ours(&format!("...{tail}"))
}

/// **`FUN_00403CF4(x, y, w, h, colour)`** — four `FUN_00403A8F` lines, **all in
/// the caller's one colour**,
/// body:
///
/// ```c
/// FUN_00403A8F(x,         y,         x + w - 1, y,         c);   /* top    */
/// FUN_00403A8F(x,         y + h - 1, x + w - 1, y + h - 1, c);   /* bottom */
/// FUN_00403A8F(x,         y,         x,         y + h - 1, c);   /* left   */
/// FUN_00403A8F(x + w - 1, y,         x + w - 1, y + h - 1, c);   /* right  */
/// ```
///
/// It is **not** `Ui_DrawInsetRect` (`0x00403DEB`), which takes four arguments
/// and lights `0x10` / `0x1F` on opposite corners; `shell::inset_rect` is that
/// one and three other screens use it. This module drew the save box's four
/// rectangles through it until the painter's call was read
/// name.
pub fn rect_outline(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, colour: u8) {
    canvas.fill_rect(x, y, w, 1, colour);
    canvas.fill_rect(x, y + h - 1, w, 1, colour);
    canvas.fill_rect(x, y, 1, h, colour);
    canvas.fill_rect(x + w - 1, y, 1, h, colour);
}

