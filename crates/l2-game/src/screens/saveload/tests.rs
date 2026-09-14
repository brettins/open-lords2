#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::screen::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_screens_are_one_painter_with_a_mode_flag() {
        assert_eq!(Mode::Load.screen_id(), 0x35);
        assert_eq!(Mode::Save.screen_id(), 0x36);
        // The painter's argument *is* the string index: `Eng_DrawString(40,
        // saving, ...)`.
        assert_eq!(Mode::Load.heading_index(), 0);
        assert_eq!(Mode::Save.heading_index(), 1);
        assert_eq!(Mode::Load.working_index(), 2);
        assert_eq!(Mode::Save.working_index(), 3);
    }

    #[test]
    fn the_thirty_rows_fill_the_rectangle_the_painter_reserves_for_them() {
        // `Ui_DrawBoxInterior(box.x + 0x1E, box.y + 0x6A, 0x15, 10)` — 21 × 10
        // cells at (46, 250), so 336 × 160 covering x 46 … 381, y 250 … 409.
        let interior = Rect::new(INTERIOR.0, INTERIOR.1, INTERIOR.2, INTERIOR.3);
        let first = SaveLoadScreen::row_rect(0);
        let last = SaveLoadScreen::row_rect(PAGE - 1);
        assert_eq!(first.x - interior.x, 2, "the first name is two pixels into the interior");
        assert_eq!(first.y - interior.y, 2);
        assert_eq!(
            (last.y - first.y) / ROW_H,
            ROWS as i32 - 1,
            "ten rows, and the last one is the tenth"
        );
        // The tenth row's *text* sits inside; its 16-pixel step overhangs the
        // interior's last two pixels, which is the painter's own arithmetic —
        // the box starts at 250 and the first baseline at 252 — and not a slip
        // here. The body font is 14 pixels tall.
        assert!(last.y + 14 <= interior.y + interior.h, "the tenth name at {} spills", last.y);
        for i in 0..PAGE {
            let r = SaveLoadScreen::row_rect(i);
            assert!(r.x >= interior.x, "row {i} starts left of the list: {}", r.x);
            assert!(r.x + r.w <= interior.x + interior.w, "row {i} runs past the list");
        }
        assert_eq!(PAGE, 30, "the loop breaks after thirty names");
    }

    #[test]
    fn the_rows_run_across_before_they_run_down() {
        // The painter steps x by 0x78 twice and only then resets and steps y.
        assert_eq!(SaveLoadScreen::row_rect(0).y, SaveLoadScreen::row_rect(2).y);
        assert_eq!(SaveLoadScreen::row_rect(1).x - SaveLoadScreen::row_rect(0).x, COL_W);
        assert_eq!(SaveLoadScreen::row_rect(3).x, SaveLoadScreen::row_rect(0).x);
        assert_eq!(SaveLoadScreen::row_rect(3).y - SaveLoadScreen::row_rect(0).y, ROW_H);
    }

    #[test]
    fn every_rectangle_this_screen_draws_is_inside_the_box() {
        let box_r = Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16);
        for (x, y, w, h) in INSETS {
            assert!(x >= box_r.x && y >= box_r.y, "inset ({x}, {y}) is outside the window");
            assert!(x + w <= box_r.x + box_r.w, "inset ({x}, {y}) is {} wide", x + w);
            assert!(y + h <= box_r.y + box_r.h, "inset ({x}, {y}) is {} tall", y + h);
        }
        // The widgets, on the reading this module argues for. Absolutely they
        // would be at y = 64 and y = 144, above this window entirely, which is
        // the argument.
        for w in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN] {
            let r = widget_rect(w);
            assert!(
                r.x >= box_r.x
                    && r.y >= box_r.y
                    && r.x + r.w <= box_r.x + box_r.w
                    && r.y + r.h <= box_r.y + box_r.h,
                "widget at ({}, {}) is outside the box it belongs to",
                r.x,
                r.y
            );
        }
    }

    #[test]
    fn the_confirm_and_cancel_buttons_do_not_overlap_anything_clickable() {
        for w in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN] {
            let r = widget_rect(w);
            for i in 0..PAGE {
                let row = SaveLoadScreen::row_rect(i);
                let overlaps = r.x < row.x + row.w
                    && row.x < r.x + r.w
                    && r.y < row.y + row.h
                    && row.y < r.y + r.h;
                assert!(!overlaps, "widget ({}, {}) sits on list row {i}", r.x, r.y);
            }
        }
    }

    #[test]
    fn scrolling_stops_at_both_ends() {
        let mut s = SaveLoadScreen::new(Mode::Load);
        s.entries = (0..40)
            .map(|i| Entry {
                name: format!("save{i:02}"),
                path: std::path::PathBuf::from(format!("save{i:02}.l2sav")),
                bytes: 0,
            })
            .collect();
        s.scroll(-9);
        assert_eq!(s.top, 0, "a list cannot scroll above its first row");
        for _ in 0..20 {
            s.scroll(SCROLL_STEP as i32);
        }
        assert_eq!(s.top, s.max_top());
        assert!(s.top + PAGE >= s.entries.len(), "the last name must be reachable");
        assert_eq!(s.max_top() % SCROLL_STEP, 0, "the top is always a whole row of three");

        // A list that fits on the page does not scroll at all.
        s.entries.truncate(PAGE);
        s.top = 0;
        s.scroll(SCROLL_STEP as i32);
        assert_eq!(s.top, 0);
    }
}

