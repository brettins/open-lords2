#![allow(unused_imports)]
use super::*;
use super::press::*;
use crate::input::{Event, Rect};

/// **The kind byte at `+0x0F` of the original's 24-byte input record, as a
/// type.**
///
/// The five are the original's own: `Widget_Test`
/// (`0x0040DA1E`) tests `+0x0F` against 4 and 5 and ignores every other value,
/// `Hotspot_Test` (`0x0040E3EE`) against 1, 3 and 2 in that order. **The
/// numbers do not overlap between the two testers** — `Hotspot_Test`'s 3 is a
/// release and `Widget_Test` has no 3 that fires at all — so the tester is part
/// of the question, which is what [`Kind::from_record`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Press,
    /// `if (kind != 2 || !(down || pressed || doubleClick)) skip;` then
    /// `if (pressed || doubleClick) fire; else if (DAT_0057D3C8) fire;`.
    Held,
    Release,
    Repeat,
    Delayed,
}

impl Kind {
    pub const fn gesture(self) -> &'static str {
        match self {
            Kind::Press => "left-press",
            Kind::Held => "left-press-held",
            Kind::Release => "left-release",
            Kind::Repeat => "left-press-repeat",
            Kind::Delayed => "left-press-delayed",
        }
    }

    /// The kind a record's `+0x0F` byte means. `widget` says which tester walks
    /// the table, because the two use the same record and different numbers.
    pub const fn from_record(widget: bool, byte: u8) -> Option<Kind> {
        Some(match (widget, byte) {
            (false, 1) => Kind::Press,
            (false, 2) => Kind::Held,
            (false, 3) => Kind::Release,
            (true, 4) => Kind::Repeat,
            (true, 5) => Kind::Delayed,
            _ => return None,
        })
    }

    /// Only the two `Widget_Test` kinds: `Widget_Draw` (`0x0040CFD2`) adds one
    /// to the frame at `+0x04` for kinds 4 and 5 and for nothing else, and
    /// `Hotspot_Test`'s records are never drawn at all.
    pub const fn has_pressed_frame(self) -> bool {
        matches!(self, Kind::Repeat | Kind::Delayed)
    }
}

/// `Widget_Test` reads `+0x06` as the side of a square — it uses `table[3]` for
/// both axes, so every widget record's width equals its height — and
/// `Hotspot_Test` reads the same four shorts as `{x0, y0, x1, y1}`. A [`Rect`]
/// expresses both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Widget {
    pub rect: Rect,
    pub kind: Kind,
}

impl Widget {
    pub const fn new(rect: Rect, kind: Kind) -> Widget {
        Widget { rect, kind }
    }
}

/// `step` is the counter at `+0x0E` **after** its increment.
pub fn fires_on_step(step: u8) -> bool {
    if step >= REPEAT_CLAMP + 1 {
        return true;
    }
    if step < REPEAT_FIRST_STEP {
        return false;
    }
    REPEAT_GATE[step as usize] != 0
}

