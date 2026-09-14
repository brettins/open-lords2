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
//! `docs/screens.md` §9. The pictures are the seven `RT_GROUP_CURSOR`
//! resources `App_InitWindow` (`0x004B2258`) loads out of `Lords2.exe`:
//! `l2_formats::cursors` reads them and the shell draws them at the canvas's
//! own whole scale, falling back to the nearest system cursor when the
//! executable is absent — `crates/l2-game/src/main.rs`, `App::apply_pointer`.

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

    /// The `RT_GROUP_CURSOR` id `App_InitWindow` (`0x004B2258`) loaded this
    /// kind's `HCURSOR` from — the picture, in the player's own `Lords2.exe`.
    /// 105 twice, into `g_cursorArrow` and `g_cursorArrowAlt`.
    pub fn resource(self) -> u16 {
        match self {
            Pointer::Arrow | Pointer::ArrowAlt => 105,
            Pointer::Question => 110,
            Pointer::Cross => 102,
            Pointer::CrossTarget => 103,
            Pointer::Ring => 104,
            Pointer::Peasant => 111,
            Pointer::Scythe => 113,
        }
    }
}

/// The seven `RT_GROUP_CURSOR` ids `App_InitWindow` loads, in id order.
pub const RESOURCES: [u16; 7] = [102, 103, 104, 105, 110, 111, 113];

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
