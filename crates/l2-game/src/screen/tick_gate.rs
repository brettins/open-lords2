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

/// `FUN_0043AD25` (`0x0043AD25`), run by `Turn_Tick` phase 7 before `Season_Advance`:
/// `if (g_screenId == '"') { Smk_Skip(); … } g_screenId = 0;` [V] `00430000.c:6181`.
/// The film above `depth` (the campaign) it skips. Reached in a network game only:
/// `DAT_004E31B0[0x22]` is 1, so [`withheld`] stops a single-player turn under it [V].
pub fn season_skips_film(ids: &[super::ScreenId], depth: usize) -> Option<crate::movie::Film> {
    ids.get(depth + 1..)?.iter().rev().find_map(|id| match id {
        super::ScreenId::Movie(f) => Some(*f),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FUN_0043AD25 0x0043AD25: a film over the campaign is skipped; none, nothing.
    #[test]
    fn the_season_turn_skips_a_film_over_the_campaign() {
        use super::super::ScreenId;
        use crate::movie::Film;
        let ids = [ScreenId::Menu, ScreenId::Campaign, ScreenId::Movie(Film::Credits)];
        assert_eq!(season_skips_film(&ids, 1), Some(Film::Credits));
        assert_eq!(season_skips_film(&ids[..2], 1), None);
        assert!(withheld(Some(0x22), false, false), "DAT_004E31B0[0x22] = 1 in single player");
    }

    /// DAT_004E31B0: 0x1E (confirm) 0, 0x12/0x13 battle prompt/result 1, 0x32 menu 1.
    #[test]
    fn the_confirm_box_winds_and_the_menu_does_not() {
        assert!(!withheld(Some(0x1E), false, false), "0x1E is 0: the turn runs under Ui_OpenConfirm");
        assert!(withheld(Some(0x12), false, false) && withheld(Some(0x13), false, false));
        assert!(withheld(Some(0x32), false, false));
        assert!(!withheld(Some(0x32), true, true), "[D] netcode: no screen pauses a network game");
    }
}
