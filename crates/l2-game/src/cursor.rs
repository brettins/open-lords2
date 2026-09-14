//! **The mouse pointer.**
//!
//! `Cursor_Set` (`0x004B1CF3`) is the binary's only caller of `SetCursor`, and
//! its only caller is `Battle_Frame` (`0x004B99C0`) — the per-frame function —
//! so the pointer is re-chosen from scratch every frame, two ways:
//!
//! * `0x28 <= g_screenId < 0x2B`, the battlefield: the hover ladder, which is
//!   [`crate::battlefield::LiveBattle::cursor`];
//! * anything else: `Cursor_Set(g_cursorByScreen[g_screenId])`, a lookup in a
//!   table of 64 dwords at `0x004E3098` of which **five** rows are non-zero.
//!
//! `docs/screens.md` §9. The pictures are seven `RT_GROUP_CURSOR` resources
//! inside `Lords2.exe`; we do not read them yet, and the shell maps a kind onto
//! the nearest system cursor — `crates/l2-game/src/mod.rs`.

/// The eight `HCURSOR`s `Cursor_Set` switches on, **by its kind number**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pointer {
    /// 0 — `g_cursorArrow`, the plain arrow.
    Arrow = 0,
    /// 2 — `g_cursorQuestion`, an arrow corner with a question mark.
    Question = 2,
    /// 4 — `g_cursorCross`, a hollow serifed cross.
    Cross = 4,
    /// 5 — `g_cursorCrossTarget`, the cross with a filled diamond.
    CrossTarget = 5,
    /// 6 — `g_cursorRing`, an empty ring.
    Ring = 6,
    /// 12 — `g_cursorPeasant`, an arrow corner with a human figure.
    Peasant = 12,
    /// 13 — `g_cursorArrowAlt`, resource 105 loaded a second time: the same
    /// picture as [`Pointer::Arrow`].
    ArrowAlt = 13,
    /// 14 — `g_cursorScythe`, an arrow corner with a scythe.
    Scythe = 14,
}

impl Pointer {
    /// The kind `Cursor_Set` was handed.
    pub fn kind(self) -> u8 {
        self as u8
    }
}

/// **`g_cursorByScreen` (`0x004E3098`)** — the kind for a `g_screenId`.
///
/// Five non-zero rows, read out of the image; every other screen in the game
/// gets 0, the plain arrow. Two of the five are dead: no `mov byte ptr
/// [g_screenId], imm` in `.text` writes 7 or 0x0E. They are kept because the
/// table is the evidence.
pub fn by_screen(screen: u8) -> Pointer {
    match screen {
        // The village, idle. This is the question-mark pointer a player
        // reported missing, and the table gives it to 0x02 alone: 0x05, the
        // rubber band, falls through to the arrow.
        0x02 => Pointer::Question,
        // The village, carrying a selection — the peasants in your hand.
        0x06 => Pointer::Peasant,
        0x07 => Pointer::ArrowAlt,
        0x0E => Pointer::Question,
        // `Map_BeginMoveSelection` (`0x0043723A`), the only writer of 0x10:
        // choosing where an army marches.
        0x10 => Pointer::Scythe,
        _ => Pointer::Arrow,
    }
}

/// The battlefield ladder's four leaves as `Cursor_Set` kinds.
impl From<crate::battlefield::Cursor> for Pointer {
    fn from(c: crate::battlefield::Cursor) -> Pointer {
        use crate::battlefield::Cursor as C;
        match c {
            C::Arrow => Pointer::Arrow,
            C::Move => Pointer::Cross,
            C::Attack => Pointer::CrossTarget,
            C::Select => Pointer::Ring,
        }
    }
}
