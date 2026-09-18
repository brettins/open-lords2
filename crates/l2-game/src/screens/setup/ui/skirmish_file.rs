#![allow(unused_imports)]
use super::*;

/// **Page 13's own hit tests** — `Screen_HandleInput`'s `g_setupPage == 0xD`
/// arm is `Widget_Test(0, 0, &g_skirmishFileWidgets, 2)` (`004b0000.c:7466`)
/// and `Screen_FrameInput` then calls `FUN_00434174`, which tests the pointer
/// itself.
///
/// The widget table is **`g_skirmishFileWidgets` (`0x004DDDD8`)**, two records
/// (`node tools/oracle/widgets.js widgets 4ddda8 6`, records 2 and 3):
///
/// ```text
///   x    y  frame size  handler          kind  hotspot  arg
/// 480  256     35   24  SaveLoad_Scroll     4       -1    2
/// 480  288     37   24  SaveLoad_Scroll     4        1    2
/// ```
///
/// Frames 35 and 37 are the scroll pair; `Widget_Test` (`0x0040DA1E`) reads
/// **`+0x06` as the side of a square**, so both boxes are 24 by 24 — not
/// `(x2, y2)`, which is what page 12's *hotspot* table at `0x004DCF50` holds.
/// That is why this pair read as nonsense until the two layouts were told
/// apart, and why `docs/arms.json` said the record was "not in any table read
/// so far".
pub const FILE_SCROLL: [(i32, i32, i32); 2] = [(480, 256, -1), (480, 288, 1)];

/// `Widget_Test`'s kind-4 box: `x .. x + table[3]`, `y .. y + table[3]`.
pub const WIDGET_SIDE: i32 = 24;

/// The two records' `+0x04`; `Widget_Draw` (`0x0040CFD2`) adds one while the
/// press timer at `+0x0D` runs, so 36 and 38 are the pressed frames.
pub const SCROLL_FRAME: [usize; 2] = [35, 37];

/// `FUN_00414E06`'s ten rows and `FUN_00434174`'s ten-row box, `0x80 ≤ x ≤
/// 0x1DF`, `0xB0 ≤ y ≤ 0x14F` (`00430000.c:1373-1379`).
impl SetupScreen {
    pub(super) fn skirmish_file_arms(&self, v: &mut Vec<(Rect, Action)>) {
        // The arrows come first because `Screen_HandleInput` runs before
        // `Screen_FrameInput`'s own arm and returns 1 when it hits.
        for (x, y, d) in FILE_SCROLL {
            v.push((
                Rect::new(x, y, WIDGET_SIDE, WIDGET_SIDE),
                Action::Skirmish(SkirmishArm::FileScroll(d)),
            ));
        }
        // **`FUN_00434174`** tests the pointer itself, not a widget record.
        // The row is `DAT_004EA1A0 + (y - 0xB0) / 16` (`00430000.c:1379`) —
        // the scroll base, not the slot.
        for r in 0..SKIRMISH_FILE_ROWS {
            v.push((
                Rect::new(0x80, 0xB0 + r as i32 * 0x10, 0x160, 0x10),
                Action::Skirmish(SkirmishArm::File(r)),
            ));
        }
        // Everything outside the ten rows is `SaveLoad_Cancel`
        // (`0x00434308`), the *OK* cross, which is the page's one way back.
        v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
    }

    /// `Screen_DrawWidgets`' page-13 arm — `FUN_00414E06(DAT_004E65DC)` and
    /// then `Widget_Draw(0, 0, &g_skirmishFileWidgets, 2)`
    /// (`004b0000.c:7086-7087`), so the arrows go over the list.
    pub(super) fn paint_skirmish_file_arrows(&self, canvas: &mut Canvas, pen: &Pen) {
        for (i, (x, y, _)) in FILE_SCROLL.iter().enumerate() {
            if !pen.system_frame(canvas, SCROLL_FRAME[i], *x, *y) {
                shell::button_recess(canvas, *x, *y, WIDGET_SIDE, WIDGET_SIDE);
            }
        }
    }
}
