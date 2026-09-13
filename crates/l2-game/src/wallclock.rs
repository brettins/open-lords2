//! **The time, in MST, on the title screen — and it is OURS.**
//!
//! # The original draws no clock here — **[V]**, read whole
//!
//! `FUN_0041EA14` is page 1's painter, 143 bytes: `FUN_00409346(Panels2, 0xA0,
//! 10, 0x14, 0xF)`, `Ui_DrawCentred(11, 0, …, &g_fontHeading, 0x3F)`,
//! `Ui_DrawCentred(11, 1, …, &g_fontBody, 0x3F)`, then `FUN_0041EAA3` for the
//! four menu items — a recess, a `Gfx_MarkSpriteDirty` and a caption each.
//! That is the whole page. No clock, and no time call on the path.
//!
//! Nor anywhere else on a screen. **The binary's one calendar clock is
//! `Log_Timestamp` (`0x004B049B`)** — `time` → `localtime` → `asctime` into the
//! 40-byte `g_logTimestamp` — and its only two readers are in `Log_Write`'s
//! buffer and file path (`0x004A…`), so what the game does with the time of day
//! is write it into a log file. Its `timeGetTime` calls are millisecond tick
//! counts used for pacing (`crate::clock` describes the film's), never a clock
//! face.
//!
//! So **this is a deliberate divergence, not a reproduction.** Nobody should
//! "fix" it toward the binary, and nobody should cite it as an arm or a draw we
//! reproduce. It is marked the way every other invention of ours is — `ours/`
//! in the inventories, and this paragraph beside the code. A player asked for
//! it: *"can the build have time in MST in the title screen"*.
//! `docs/decisions.md` correction CNEW-title-mst-clock.
//!
//! # It is MST, and MST is not "the local zone"
//!
//! Fixed **UTC−7**, all year. Mountain *Daylight* Time is UTC−6 and is a
//! different zone with a different name; reading the host's zone would give a
//! third answer again on a machine that is not in Denver. [`MST_OFFSET_SECS`]
//! is the whole conversion and there is no daylight rule anywhere in this file
//! — which is the point, not an omission.
//!
//! # How it stays out of the simulation
//!
//! `docs/netcode.md` D-5 — *no wall clock, no scheduler* — binds everything
//! under the renderer. So this module is arithmetic on a reading it is
//! **handed**: nothing here calls `SystemTime::now()`, exactly as
//! [`crate::clock::Ticker`] takes its monotonic nanoseconds from `main.rs`
//! rather than reading them (`docs/decisions.md` C193).
//!
//! The reading arrives through `Assets::wall_clock`, which `main.rs` samples
//! and which is the same channel `Assets::quirks` already uses — the shell
//! projects into the asset bag once per tick and the painters read it. It
//! reaches no [`crate::Game`], no `Kingdom`, no digest and no save: `Assets` is
//! not part of any of them, and `crates/l2-game/tests/setup.rs`
//! `the_clock_cannot_reach_the_simulation` asserts the digest is the same under
//! two different readings.
//!
//! And every function here is pure, so the screen's own test injects a fixed
//! instant and needs no clock to pass.

use l2_view::Canvas;

use crate::shell::{font, Pen};

/// **MST is UTC−7 and stays UTC−7.** See the module header: the daylight zone
/// is MDT, which is a different thing with a different name, and the player
/// asked for MST.
pub const MST_OFFSET_SECS: i64 = -7 * 3600;

/// The gap kept at the right edge and below the line — the same 4 and 2
/// [`crate::build_id`] keeps at the left, so the two stamps sit on one baseline
/// at opposite corners.
const MARGIN_X: i32 = 4;
const MARGIN_Y: i32 = 2;

/// **`HH:MM`, 24-hour, in MST**, from seconds since the Unix epoch.
///
/// `rem_euclid` rather than `%`: a reading before 1970 is not something a game
/// will ever see, but a negative remainder would print `-7:00` rather than
/// wrapping, and a formatter that is only correct for half its domain is a
/// formatter somebody eventually feeds the other half.
pub fn hhmm(unix_secs: i64) -> String {
    let day = (unix_secs + MST_OFFSET_SECS).rem_euclid(86_400);
    format!("{:02}:{:02}", day / 3600, day % 3600 / 60)
}

/// Which minute a reading falls in, for the screen's *has the picture changed*
/// test. The clock is drawn to the minute, so a repaint is owed once a minute
/// and not sixty times a second.
pub fn minute(unix_secs: i64) -> i64 {
    (unix_secs + MST_OFFSET_SECS).div_euclid(60)
}

/// Where the line starts, right-aligned against the canvas.
pub fn left_edge(pen: &Pen, s: &str) -> i32 {
    let w = match &pen.assets.body {
        Some(f) => f.width(s),
        None => l2_view::text::width(s),
    };
    l2_view::canvas::WIDTH as i32 - MARGIN_X - w
}

/// And its top row, measured off the face that will actually draw it —
/// [`crate::build_id::top_edge`]'s rule, for its reason: a literal chosen
/// against one font clips the other, and that shipped once already.
pub fn top_edge(pen: &Pen, s: &str) -> i32 {
    let h = match &pen.assets.body {
        Some(f) => f.height(s),
        None => l2_view::text::GLYPH_H,
    };
    l2_view::canvas::HEIGHT as i32 - MARGIN_Y - h
}

/// **Draw it — bottom-right, in the page's own body face and its own
/// [`font::TEXT`].**
///
/// # Why there, and why in that face
///
/// *Where*: page 1's window is `FUN_00409346(Panels2, 0xA0, 10, 0x14, 0xF)`
/// — 16-pixel cells, so x 160…480 by y 10…250 — and its lowest item is
/// [`crate::screens::setup::item_rect`] 3, y 199…223. Nothing the original's
/// page-1 painter draws reaches below y 250, and the bottom-left corner is
/// already [`crate::build_id`]'s. The bottom-right corner is the free one, and
/// putting ours opposite ours keeps every invention on this page in the same
/// band of the picture.
///
/// *Which face*: the body font at [`font::TEXT`] (`0x3F`), flat, which is what
/// the subtitle and all four menu captions on this page are drawn in — the
/// caller hands us the page's own flat pen, so the colour and the emboss are
/// the page's rather than a style invented here. It is deliberately **not**
/// [`crate::build_id`]'s plain `Fntl2_9.pl8`: that line is seven hex characters
/// compared one at a time, and this is four digits read as a shape, like every
/// other word on the screen.
pub fn draw(canvas: &mut Canvas, pen: &Pen, unix_secs: i64) {
    let s = hhmm(unix_secs);
    pen.body(canvas, left_edge(pen, &s), top_edge(pen, &s), &s, font::TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The epoch itself, and the offset is the whole of the conversion.
    #[test]
    fn midnight_utc_is_five_in_the_afternoon_the_day_before() {
        assert_eq!(hhmm(0), "17:00");
        assert_eq!(hhmm(7 * 3600), "00:00");
        assert_eq!(hhmm(7 * 3600 + 86_399), "23:59");
        assert_eq!(hhmm(7 * 3600 + 86_400), "00:00", "and it wraps at midnight");
    }

    /// **The ablation, as an assertion: there is no daylight shift.**
    ///
    /// Two instants six months apart — 15 January and 15 July 2026, both
    /// 19:00:00 UTC — must read the same clock face. A `chrono`-style local
    /// zone, or any conditional on the date, gives 12:00 in January and 13:00
    /// in July; MST gives 12:00 for both because MST *is* UTC−7 and the zone
    /// that shifts is called MDT.
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

    /// A repaint is owed on the minute boundary and at no other time.
    #[test]
    fn the_minute_changes_once_a_minute() {
        let t = 1_768_503_600;
        assert_eq!(minute(t), minute(t + 59));
        assert_ne!(minute(t), minute(t + 60));
        assert_eq!(minute(t + 60) - minute(t), 1);
    }

    /// Nothing in this module reads a clock — the property `docs/netcode.md`
    /// D-5 asks for, stated where it can be checked: the same reading formats
    /// the same way whenever it is asked.
    #[test]
    fn the_same_reading_always_formats_the_same_way() {
        let t = 1_768_503_600;
        assert_eq!(hhmm(t), hhmm(t));
        assert_eq!(hhmm(t), "12:00");
    }
}
