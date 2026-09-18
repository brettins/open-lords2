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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pointer {
    Arrow = 0,
    Question = 2,
    Cross = 4,
    CrossTarget = 5,
    Ring = 6,
    Peasant = 12,
    ArrowAlt = 13,
    Scythe = 14,
}

impl Pointer {
    pub fn kind(self) -> u8 {
        self as u8
    }

    /// The `RT_GROUP_CURSOR` id `App_InitWindow` (`0x004B2258`) loaded this
    /// kind's `HCURSOR` from — the picture, in the player's own `Lords2.exe`.
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

pub const RESOURCES: [u16; 7] = [102, 103, 104, 105, 110, 111, 113];

/// **`g_cursorByScreen` (`0x004E3098`)** — the kind for a `g_screenId`.
pub fn by_screen(screen: u8) -> Pointer {
    match screen {
        0x02 => Pointer::Question,
        0x06 => Pointer::Peasant,
        0x07 => Pointer::ArrowAlt,
        0x0E => Pointer::Question,
        // `Map_BeginMoveSelection` (`0x0043723A`), the only writer of 0x10:
        0x10 => Pointer::Scythe,
        _ => Pointer::Arrow,
    }
}

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
