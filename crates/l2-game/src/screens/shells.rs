//! A row here named a screen's `g_screenId`, its painter's address, the `.pl8`
//! that painter loads, the window it opens and the `L2.eng` group it draws —
//! every one read out of the painter, none invented — and one generic painter
//! walked the table. What appeared on screen was the original's artwork with
//! the original's words on it in the original's rectangles, and no logic behind
//! any of it. A player looking at the build called it *"placeholder shit
//! everywhere"*, and he was looking at exactly this.
//!
//! **The history is the audit.** Every row that ever left carried a claim, and
//! four of them were *wrong* in ways that cost real time: `0x17` was filed as
//! *"Hire mercenaries"* while its painter was `Screen_RaiseArmy` and there is
//! no mercenaries screen at all; the armoury's row cited `L2.eng` group 16
//! where the truth was 69; `0x0F` was a live duplicate of a screen that had
//! already graduated; and `0x04`'s row said its painter draws no `Ui_DrawBox`
//! when *both* of its halves open with one. Those corrections are recorded on
//! [`SHELLS`] itself, and a file that deleted itself would delete them.

use crate::screen::ScreenId;

/// One line of `L2.eng` a painter drew at a fixed place: `(index, x, y)`.
pub type Line = (usize, i32, i32);

pub struct Shell {
    pub id: u8,
    pub painter: u32,
    pub name: &'static str,
    pub background: Option<&'static str>,
    pub palette: Option<&'static str>,
    /// `Ui_DrawBox`/`FUN_004093E0`: `(x, y, cols, rows, borderSet)` in cells of
    /// 16 pixels. `FUN_004093E0` is border set 1, `Ui_DrawBox` is set 0.
    pub window: Option<(i32, i32, i32, i32, usize)>,
    /// The `L2.eng` group this painter draws.
    pub group: usize,
    pub heading: Option<Line>,
    pub lines: &'static [Line],
    pub ok: Option<(i32, i32, usize)>,
    pub overlay: bool,
    pub unfinished: &'static str,
}

/// * `0x04` **the map information panel** → `screens/info.rs`. The row said
///   *"`FUN_0041B032` draws no `Ui_DrawBox`"* — **false**, both halves open
///   with one, at `0x0041B1D9` and `0x0041C1B0`, and `window: None` followed
///   from that. What it had *right* was refusing to place a line at a runtime
///   `DAT_00553D2C * 16 + 74`; the arithmetic closes now — top + height is 464
/// for all eleven layouts — and the answer is y 106 or y 362.
///.
/// * `0x0B` **diplomacy** → `screens/diplomacy.rs`. Group 72 was right; the
///   name *"The other lords"* named the left-hand column and hid the menu, and
///   group 72 index 0 is literally *"Diplomacy."* `heading: None` was wrong —
///   the painter draws the selected lord's name at (208, 61).
///
/// * `0x17` **the raise-army screen** → `screens/army.rs`, *and its name was
///   the finding.* Filed as *"Hire mercenaries"* while `docs/symbols.json`
/// called its painter `Screen_RaiseArmy`;
///   the game, the offer is a block on this one, and this is the only door to
///   `Army_Create` a player has. `docs/decisions.md` C45.
pub const SHELLS: &[Shell] = &[];

pub fn find(id: u8) -> Option<&'static Shell> {
    SHELLS.iter().find(|s| s.id == id)
}

pub const GRADUATED: &[(u8, &str)] = &[
    (0x04, "screens/info.rs"),
    (0x08, "screens/merchant.rs"),
    (0x09, "screens/court.rs"),
    (0x0A, "screens/armoury.rs"),
    (0x0B, "screens/diplomacy.rs"),
    (0x0C, "screens/merchant.rs"),
    (0x0D, "screens/armoury.rs"),
    (0x0F, "screens/job.rs"),
    (0x11, "screens/divide.rs"),
    (0x17, "screens/army.rs"),
    (0x18, "screens/supplies.rs"),
    (0x1B, "screens/castle.rs"),
    (0x1D, "screens/siege.rs"),
    (0x25, "screens/about.rs"),
    (0x2E, "screens/ratings.rs"),
    (0x31, "screens/options.rs"),
    (0x35, "screens/saveload.rs"),
    (0x36, "screens/saveload.rs"),
    (0x39, "screens/options.rs"),
    (0x42, "screens/options.rs"),
    (0x43, "screens/options.rs"),
];

pub fn screen_for(id: u8) -> Option<ScreenId> {
    match id {
        0x09 => Some(ScreenId::Court),
        0x0B => Some(ScreenId::Diplomacy),
        0x25 => Some(ScreenId::About),
        0x2E => Some(ScreenId::Ratings),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_empty() {
        assert!(
            SHELLS.is_empty(),
            "{} screen(s) are back to being artwork with nothing behind them: {:?}",
            SHELLS.len(),
            SHELLS.iter().map(|s| s.name).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn find_answers_for_every_id_that_ever_sat_here() {
        for &(id, module) in GRADUATED {
            assert!(
                find(id).is_none(),
                "0x{id:02X} is a shell again, and {module} is supposed to have it",
            );
            assert!(!module.is_empty());
        }
        let mut ids: Vec<u8> = GRADUATED.iter().map(|&(id, _)| id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "an id is listed twice");
    }

    #[test]
    fn no_two_graduated_ids_resolve_to_one_screen() {
        let mut seen: Vec<ScreenId> =
            GRADUATED.iter().filter_map(|&(id, _)| screen_for(id)).collect();
        let before = seen.len();
        seen.dedup_by(|a, b| a == b);
        assert_eq!(seen.len(), before, "two ids resolve to the same screen");
        assert_eq!(before, 4, "four ids need no argument");
    }
}
