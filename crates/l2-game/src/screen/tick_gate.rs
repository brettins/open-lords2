//! `FUN_004BB978` (`0x004BB978`), the ticks-due count `Battle_Frame` runs
//! `Turn_Tick` + `Units_Tick` (or the battle's passes) that many times for.
//! [V] `004b0000.c:7641`:
//!
//! ```c
//! if ((&DAT_004e31b0)[g_screenId] == '\x02') { if (g_multiplayer == 0) return 0; }
//! else if ((&DAT_004e31b0)[g_screenId] != '\0') return 0;
//! if (g_messageGroup != 0) { if (g_multiplayer == 0) return 0;
//!                            if ((&DAT_004e3240)[g_messageCategory] != '\0') return 0; }
//! ```

/// `DAT_004E31B0`, indexed by `g_screenId`; nonzero stops the tick. [V] read
/// out of `.data`; it holds no `2`, so the single-player-only arm is dead.
pub const PAUSES: [u8; 0x46] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x00
    0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, // 0x10
    0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, // 0x20
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x30
    1, 1, 1, 1, 1, 1, // 0x40
];

/// Whether this frame's ticks are withheld. `byte` is the top screen's
/// `g_screenId` (the message scroll is not a screen there); `None`, a screen
/// the original does not have, withholds nothing [I].
///
/// [D] netcode: a network game winds through `net::frame` whatever is on
/// screen — one peer's screen cannot pause the world — so the original's
/// multiplayer arms (the table's `1`s, `DAT_004E3240`) have no counterpart.
pub fn withheld(byte: Option<u8>, message_open: bool, multiplayer: bool) -> bool {
    if multiplayer {
        return false;
    }
    byte.is_some_and(|b| PAUSES.get(b as usize).is_some_and(|&p| p != 0)) || message_open
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DAT_004E31B0: 0x1E (confirm) 0, 0x12/0x13 battle prompt/result 1, 0x32 menu 1.
    #[test]
    fn the_confirm_box_winds_and_the_menu_does_not() {
        assert!(!withheld(Some(0x1E), false, false), "0x1E is 0: the turn runs under Ui_OpenConfirm");
        assert!(withheld(Some(0x12), false, false) && withheld(Some(0x13), false, false));
        assert!(withheld(Some(0x32), false, false));
        assert!(!withheld(Some(0x32), true, true), "[D] netcode: no screen pauses a network game");
    }
}
