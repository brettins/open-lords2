//! **The shell table, and it is empty.**
//!
//! # What a shell was
//!
//! A row here named a screen's `g_screenId`, its painter's address, the `.pl8`
//! that painter loads, the window it opens and the `L2.eng` group it draws —
//! every one read out of the painter, none invented — and one generic painter
//! walked the table. What appeared on screen was the original's artwork with
//! the original's words on it in the original's rectangles, and no logic behind
//! any of it. A player looking at the build called it *"placeholder shit
//! everywhere"*, and he was looking at exactly this.
//!
//! **A screen left the table when there was real state behind it, and they all
//! have.** [`SHELLS`] is empty, [`find`] answers `None` for every id, and the
//! generic painter and its `ScreenId::Shell` are gone rather than left standing
//! with nothing to draw.
//!
//! # Why the file stays
//!
//! Two reasons, and neither is sentiment.
//!
//! **The history is the audit.** Every row that ever left carried a claim, and
//! four of them were *wrong* in ways that cost real time: `0x17` was filed as
//! *"Hire mercenaries"* while its painter was `Screen_RaiseArmy` and there is
//! no mercenaries screen at all; the armoury's row cited `L2.eng` group 16
//! where the truth was 69; `0x0F` was a live duplicate of a screen that had
//! already graduated; and `0x04`'s row said its painter draws no `Ui_DrawBox`
//! when *both* of its halves open with one. Those corrections are recorded on
//! [`SHELLS`] itself, and a file that deleted itself would delete them.
//!
//! **The two tests are the check.** [`tests::the_table_is_empty`] is what keeps
//! it empty — a new shell is now a decision somebody has to argue for rather
//! than a row somebody adds — and
//! [`tests::find_answers_for_every_id_that_ever_sat_here`] names them all, so a
//! screen cannot quietly regress into one.
//!
//! # The rule the table taught, which is worth more than the table
//!
//! **A row of five fields cannot say what a painter does.** Every graduation
//! found something the row could not hold: that `0x0A` is where an army is
//! *created* rather than a shop; that `0x09` draws four of its five lines in
//! the 22-pixel font and the fifth is a button caption, not a line; that `0x2E`
//! is seven *columns* by three rows and not seven rows; that `0x04`'s eleven
//! layouts all pin their bottom edge at y 464. None of that fits in a table and
//! all of it fits in a module header — which is why every graduated screen
//! carries its painter as a literal listing and this file carries only the
//! history.

use crate::screen::ScreenId;

/// One line of `L2.eng` a painter drew at a fixed place: `(index, x, y)`.
pub type Line = (usize, i32, i32);

/// A screen we could draw and could not yet drive. **Nothing is one any more.**
pub struct Shell {
    /// `g_screenId`, the byte `Screen_Draw` switches on.
    pub id: u8,
    /// Its painter, for anyone going back to the binary.
    pub painter: u32,
    /// What this document called it.
    pub name: &'static str,
    /// The full-screen background, if the painter loads one.
    pub background: Option<&'static str>,
    /// The `.256` it sets with it.
    pub palette: Option<&'static str>,
    /// `Ui_DrawBox`/`FUN_004093E0`: `(x, y, cols, rows, borderSet)` in cells of
    /// 16 pixels. `FUN_004093E0` is border set 1, `Ui_DrawBox` is set 0.
    pub window: Option<(i32, i32, i32, i32, usize)>,
    /// The `L2.eng` group this painter draws.
    pub group: usize,
    /// The heading, in the 22-pixel font.
    pub heading: Option<Line>,
    /// The body lines, in the 14-pixel font.
    pub lines: &'static [Line],
    /// `Ui_OkButton(x, y, mode)`.
    pub ok: Option<(i32, i32, usize)>,
    /// Whether this drew over what was underneath rather than replacing it.
    pub overlay: bool,
    /// What the painter did that the shell did not.
    pub unfinished: &'static str,
}

/// **Empty.** Every screen that was ever here is built; the list below is the
/// record of what each row claimed and what was wrong with it.
///
/// * `0x04` **the map information panel** → `screens/info.rs`. The row said
///   *"`FUN_0041B032` draws no `Ui_DrawBox`"* — **false**, both halves open
///   with one, at `0x0041B1D9` and `0x0041C1B0`, and `window: None` followed
///   from that. What it had *right* was refusing to place a line at a runtime
///   `DAT_00553D2C * 16 + 74`; the arithmetic closes now — top + height is 464
///   for all eleven layouts — and the answer is y 106 or y 362.
/// * `0x08` **the merchant** → `screens/merchant.rs`. The row said *"the price
///   grid (`mercgrid.pl8`) and the eight commodities"*; `mercgrid.pl8` is not
///   drawn at all — it is the hit test, an 80 × 60 byte map of good ids — and
///   there are **twelve** wares on the stall. `0x0C` went with it.
/// * `0x09` **the court** → `screens/court.rs`. Group 70 was right. The *fonts*
///   were wrong: indices 0, 2, 3 and 4 are the 22-pixel heading font, not the
///   body font the `lines` field documents, and index 6 is a **button caption**
///   rather than a line at all.
/// * `0x0A` **the armoury** → `screens/armoury.rs`, and it took `0x0D` with it.
///   The group was **69**, not 16 — 16 is the twelve mercenary nationalities
///   and the painter never touches it. Filing it wrong made it look like a
///   mercenary panel with nothing behind it rather than the screen the whole
///   levy is confirmed on.
/// * `0x0B` **diplomacy** → `screens/diplomacy.rs`. Group 72 was right; the
///   name *"The other lords"* named the left-hand column and hid the menu, and
///   group 72 index 0 is literally *"Diplomacy."* `heading: None` was wrong —
///   the painter draws the selected lord's name at (208, 61).
/// * `0x0F` **the job popup** — **never a shell at all.** Name, painter and
///   group were all correct *and already claimed by `screens/job.rs`*, which
///   had graduated. Two index entries for one screen of the original.
///   Everything in the row that was not a copy was wrong: the window was
///   admitted invented and the real one is `Ui_DrawBox(0x30, 0x60, 0x19,
///   g_jobPanelRows[job])`, a *variable* height, and the heading index is the
///   **job**, not a constant 1. Nothing caught it, because the only check
///   dedupped *within* this table.
/// * `0x11` **army division** → `screens/divide.rs`.
/// * `0x17` **the raise-army screen** → `screens/army.rs`, *and its name was
///   the finding.* Filed as *"Hire mercenaries"* while `docs/symbols.json`
///   called its painter `Screen_RaiseArmy`; there is no mercenaries screen in
///   the game, the offer is a block on this one, and this is the only door to
///   `Army_Create` a player has. `docs/decisions.md` C45.
/// * `0x18` **send supplies** → `screens/supplies.rs`. The only row whose every
///   field checked out, `unfinished` included.
/// * `0x1B` **castle building** → `screens/castle.rs`.
/// * `0x25` **About** → `screens/about.rs`. `unfinished: "nothing — this is the
///   whole screen"` read like an apology and was a **measurement**: the painter
///   is 171 bytes and there is nothing else in it. It graduated by acquiring
///   input and a test, not paint.
/// * `0x2E` **Battle Master ratings** → `screens/ratings.rs`. Everything right
///   except the shape: *"the seven rating rows per player"* is **seven columns
///   by three rows**, twice over, and the columns are troop types.
/// * `0x31`, `0x39`, `0x42`, `0x43` **the four options panels** →
///   `screens/options.rs`.
/// * `0x35`, `0x36` **load and save** → `screens/saveload.rs`.
pub const SHELLS: &[Shell] = &[];

/// The shell for a screen id — **always `None`**, and kept so that the claim is
/// testable rather than assumed.
pub fn find(id: u8) -> Option<&'static Shell> {
    SHELLS.iter().find(|s| s.id == id)
}

/// Every `g_screenId` that ever had a row here, with the module that has it
/// now. [`tests::find_answers_for_every_id_that_ever_sat_here`] walks it.
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

/// The [`ScreenId`] a graduated id resolves to, for the ids whose screen needs
/// no argument. `None` means the screen takes a county or a unit that this
/// table cannot supply, and its module is named in [`GRADUATED`] instead.
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

    /// **The check that keeps it empty.**
    ///
    /// Not `is_empty()` as a formality. The predicate this *means* is *"no
    /// screen in this engine is artwork with nothing behind it"*, and this
    /// table is the only place that could ever be expressed: a row added here
    /// is a screen somebody decided to ship as a picture, and that should cost
    /// an argument rather than a diff nobody reads.
    ///
    /// It replaces `every_shell_has_a_distinct_screen_id_and_a_painter`, which
    /// dedupped *within* the table and therefore could not see `0x0F` sitting
    /// here beside the `screens/job.rs` that already owned it.
    #[test]
    fn the_table_is_empty() {
        assert!(
            SHELLS.is_empty(),
            "{} screen(s) are back to being artwork with nothing behind them: {:?}",
            SHELLS.len(),
            SHELLS.iter().map(|s| s.name).collect::<Vec<_>>(),
        );
    }

    /// Every id that ever sat here is somebody's module now, and none of them
    /// is a shell again.
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

    /// The four ids whose screen needs no argument resolve to a **distinct**
    /// `ScreenId` each — the other half of the `0x0F` failure, where two
    /// entries claimed one screen of the original.
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
