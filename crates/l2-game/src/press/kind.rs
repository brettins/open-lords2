#![allow(unused_imports)]
use super::*;
use super::press::*;
use crate::input::{Event, Rect};

/// **The kind byte at `+0x0F` of the original's 24-byte input record, as a
/// type.**
///
/// This is the whole of what decides press, release, hold or repeat, and until
/// it existed here a screen answered a gesture by hand-rolling its own
/// press/release bookkeeping — which is how `docs/arms.json` came to mark
/// nineteen arms `reproduced` under a kind none of them had. A screen
/// now **declares** the kind, in a [`Widget`] table, and [`Press::event`]
/// decides when the handler runs.
///
/// The five are the original's own: `Widget_Test`
/// (`0x0040DA1E`) tests `+0x0F` against 4 and 5 and ignores every other value,
/// `Hotspot_Test` (`0x0040E3EE`) against 1, 3 and 2 in that order. **The
/// numbers do not overlap between the two testers** — `Hotspot_Test`'s 3 is a
/// release and `Widget_Test` has no 3 that fires at all — so the tester is part
/// of the question, which is what [`Kind::from_record`] takes.
///
/// `docs/input.md` is the model in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `Hotspot_Test` kind **1** — fires on the down edge, draws nothing.
    /// The majority of the interface.
    ///
    /// It reads `g_mouseLeftPressed` **and not** `g_mouseLeftDoubleClick`, so
    /// the second click of a double click does not fire it: Windows sends
    /// `WM_LBUTTONDBLCLK`
    /// behaviour, not an oversight, and [`Press::event`] keeps it.
    Press,
    /// `Hotspot_Test` kind **2** — the down edge, then a **flat**
    /// [`HELD_PULSE_MS`] pulse for as long as the button stays down on it.
    ///
    /// `if (kind != 2 || !(down || pressed || doubleClick)) skip;` then
    /// `if (pressed || doubleClick) fire; else if (DAT_0057D3C8) fire;`.
    Held,
    /// `Hotspot_Test` kind **3** — fires on `g_mouseLeftReleased`, the **up**
    /// edge.
    ///
/// It carries no memory of a press: the tester hit-tests the box and
    /// reads the released flag,
    /// whether or not the press that preceded it happened there.
    Release,
    /// `Widget_Test` kind **4** — the down edge, the pressed picture for
    /// [`PRESS_FRAMES`] frames, and an **accelerating** auto-repeat off
    /// [`REPEAT_GATE`].
    ///
    /// Every `+`/`−`, every `<`/`>`, every up/down arrow in the game.
    Repeat,
    /// `Widget_Test` kind **5** — the down edge puts the pressed picture up and
    /// the handler runs [`DELAYED_FRAMES`] frames **later**.
    ///
    /// Every yes/no gauntlet, every options checkbox, diplomacy's send. *"The
    /// game waited on mouse-up, and the gauntlet would go down slightly when
    /// clicked."*
    Delayed,
}

impl Kind {
    /// **The word `docs/arms.json` files this kind under**, so the marker beside
    /// an arm and the type the code answers it with cannot drift apart by
    /// somebody editing one of them.
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

    /// **Does a widget of this kind show the pressed picture?**
    ///
    /// Only the two `Widget_Test` kinds: `Widget_Draw` (`0x0040CFD2`) adds one
    /// to the frame at `+0x04` for kinds 4 and 5 and for nothing else, and
    /// `Hotspot_Test`'s records are never drawn at all.
    pub const fn has_pressed_frame(self) -> bool {
        matches!(self, Kind::Repeat | Kind::Delayed)
    }
}

/// **One record of a screen's input table** — a rectangle and the kind of
/// gesture it answers.
///
/// The original's record carries the geometry, the sprite frame, the handler
/// pointer, three counters and the kind, in 24 bytes. Ours carries the geometry
/// and the kind: the handler is the arm of the `match` the screen writes around
/// the index this returns, the counters live in [`Press`] — indexed by the same
/// index, one press timer per record — and the frame belongs to the painter.
///
/// **The hit box is a square in the original and a rectangle here.**
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

/// Does the button fire on this step of the hold?
///
/// `step` is the counter at `+0x0E` **after** its increment.
/// original tests it.
pub fn fires_on_step(step: u8) -> bool {
    if step >= REPEAT_CLAMP + 1 {
        // The clamp branch: `rec[0x0E] = 0x2F` and fall straight through to the
        // call. It does not look at the table.
        return true;
    }
    if step < REPEAT_FIRST_STEP {
        return false;
    }
    REPEAT_GATE[step as usize] != 0
}

