//! # The original draws no clock here — **[V]**, read whole
//!
//! `FUN_0041EA14` is page 1's painter, 143 bytes: `FUN_00409346(Panels2, 0xA0,
//! 10, 0x14, 0xF)`, `Ui_DrawCentred(11, 0, …, &g_fontHeading, 0x3F)`,
//! `Ui_DrawCentred(11, 1, …, &g_fontBody, 0x3F)`, then `FUN_0041EAA3` for the
//! four menu items — a recess, a `Gfx_MarkSpriteDirty` and a caption each.
//!
//! Nor anywhere else on a screen. **The binary's one calendar clock is
//! `Log_Timestamp` (`0x004B049B`)** — `time` → `localtime` → `asctime` into the
//! 40-byte `g_logTimestamp` — and its only two readers are in `Log_Write`'s
//! buffer and file path (`0x004A…`), so what the game does with the time of day
//! is write it into a log file. Its `timeGetTime` calls are millisecond tick
//! counts used for pacing (`crate::clock` describes the film's), never a clock
//! face.
//!
//! `docs/decisions.md` correction C206.

use l2_view::Canvas;

use crate::shell::{font, Pen};

pub const MST_OFFSET_SECS: i64 = -7 * 3600;

const MARGIN_X: i32 = 4;
const MARGIN_Y: i32 = 2;

pub fn hhmm(unix_secs: i64) -> String {
    let day = (unix_secs + MST_OFFSET_SECS).rem_euclid(86_400);
    format!("{:02}:{:02}", day / 3600, day % 3600 / 60)
}

pub fn minute(unix_secs: i64) -> i64 {
    (unix_secs + MST_OFFSET_SECS).div_euclid(60)
}

pub fn left_edge(pen: &Pen, s: &str) -> i32 {
    let w = match &pen.assets.body {
        Some(f) => f.width(s),
        None => l2_view::text::width(s),
    };
    l2_view::canvas::WIDTH as i32 - MARGIN_X - w
}

pub fn top_edge(pen: &Pen, s: &str) -> i32 {
    let h = match &pen.assets.body {
        Some(f) => f.height(s),
        None => l2_view::text::GLYPH_H,
    };
    l2_view::canvas::HEIGHT as i32 - MARGIN_Y - h
}

/// *Where*: page 1's window is `FUN_00409346(Panels2, 0xA0, 10, 0x14, 0xF)`
/// — 16-pixel cells, so x 160…480 by y 10…250 — and its lowest item is
/// [`crate::screens::setup::item_rect`] 3, y 199…223. Nothing the original's
/// page-1 painter draws reaches below y 250, and the bottom-left corner is
/// already [`crate::build_id`]'s. The bottom-right corner is the free one, and
/// putting ours opposite ours keeps every invention on this page in the same
/// band of the picture.
pub fn draw(canvas: &mut Canvas, pen: &Pen, unix_secs: i64) {
    let s = hhmm(unix_secs);
    pen.body(canvas, left_edge(pen, &s), top_edge(pen, &s), &s, font::TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_utc_is_five_in_the_afternoon_the_day_before() {
        assert_eq!(hhmm(0), "17:00");
        assert_eq!(hhmm(7 * 3600), "00:00");
        assert_eq!(hhmm(7 * 3600 + 86_399), "23:59");
        assert_eq!(hhmm(7 * 3600 + 86_400), "00:00", "and it wraps at midnight");
    }

    /// **The ablation, as an assertion: **
    #[test]
    fn mst_does_not_shift_in_summer() {
        let january = 1_768_503_600; // 2026-01-15T19:00:00Z
        let july = 1_784_142_000; // 2026-07-15T19:00:00Z
        assert_eq!(hhmm(january), "12:00");
        assert_eq!(hhmm(july), "12:00");
        assert_eq!(
            hhmm(january),
            hhmm(july),
            "MST is a fixed UTC-7 offset; a difference here means a daylight rule or the \
             host's own zone has got in, and the player asked for MST"
        );
    }

    #[test]
    fn the_minute_changes_once_a_minute() {
        let t = 1_768_503_600;
        assert_eq!(minute(t), minute(t + 59));
        assert_ne!(minute(t), minute(t + 60));
        assert_eq!(minute(t + 60) - minute(t), 1);
    }

    #[test]
    fn the_same_reading_always_formats_the_same_way() {
        let t = 1_768_503_600;
        assert_eq!(hhmm(t), hhmm(t));
        assert_eq!(hhmm(t), "12:00");
    }
}
