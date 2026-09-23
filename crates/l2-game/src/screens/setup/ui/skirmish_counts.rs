#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::{Skirmish, TroopsTable, STRENGTH_WEIGHT};

/// **The custom muster's `+`/`-` table** — `DAT_004DE000`, tested by
/// `Screen_HandleInput`'s `g_setupPage == 0xC` arm as
/// `Widget_Test(0, 0, &DAT_004DE000, DAT_0056D5C0)` (`004b0000.c:7473`), and
/// only while `DAT_0056899C == 1`.
///
/// `DAT_0056D5C0` is **0x1C for a field battle and 0x2C for a castle**
/// (`00420000.c:4602`, `00430000.c:8179` and `:8182`), which is the seven or
/// eleven rows the painter draws times four buttons:
///
/// ```text
///   x    y  frame size  handler        kind  hotspot
///  95  106+30r    1   16  FUN_0043D6A7    4     row     one more
/// 111  106+30r    3   16  FUN_0043D78B    4     row     one fewer
/// 134  106+30r    5   16  FUN_0043D882    4     row     as many as fit
/// 150  106+30r    7   16  FUN_0043D8B9    4     row     none at all
/// ```
///
/// `node tools/oracle/widgets.js widgets 4de000 44`. Records 0…13 and 28…35
/// are the left column, 14…27 and 36…43 the right; rows 7…10 are the four
/// siege engines and exist only in the castle table's 44.
pub const COUNT_X: [i32; 4] = [95, 111, 134, 150];
pub const COUNT_Y0: i32 = 106;
pub const COUNT_PITCH: i32 = 30;
pub const COUNT_SIDE: i32 = 16;
pub const COUNT_FRAME: [usize; 4] = [1, 3, 5, 7];

/// Which of the four columns was pressed, as `SkirmishArm::Count` carries it.
pub const ONE_MORE: usize = 0;
pub const ONE_FEWER: usize = 1;
pub const ALL_MORE: usize = 2;
pub const ALL_FEWER: usize = 3;

impl SetupScreen {
    pub(super) fn custom_count_arms(&self, v: &mut Vec<(Rect, Action)>) {
        if !self.skirmish.custom {
            return;
        }
        let rows = if self.skirmish.kind == 2 { STRENGTH_WEIGHT.len() } else { 7 };
        for row in 0..rows {
            let y = COUNT_Y0 + row as i32 * COUNT_PITCH;
            for col in 0..4 {
                v.push((
                    Rect::new(COUNT_X[col], y, COUNT_SIDE, COUNT_SIDE),
                    Action::Skirmish(SkirmishArm::Count(col, row)),
                ));
            }
        }
    }

    /// `Widget_Draw(0, 0, &DAT_004DE000, DAT_0056D5C0)` (`004b0000.c:7100`),
    /// drawn only under the same `DAT_0056899C == 1` guard.
    pub(crate) fn paint_custom_count_buttons(&self, canvas: &mut Canvas, pen: &Pen) {
        let rows = if self.skirmish.kind == 2 { STRENGTH_WEIGHT.len() } else { 7 };
        for row in 0..rows {
            let y = COUNT_Y0 + row as i32 * COUNT_PITCH;
            for col in 0..4 {
                if !pen.system_frame(canvas, COUNT_FRAME[col], COUNT_X[col], y) {
                    shell::button_recess(canvas, COUNT_X[col], y, COUNT_SIDE, COUNT_SIDE);
                }
            }
        }
    }

    pub(crate) fn custom_count(&mut self, col: usize, row: usize) {
        let table = std::mem::take(&mut self.troops);
        self.skirmish.count_arm(&table, col, row);
        self.troops = table;
    }
}

impl Skirmish {
    /// **`DAT_00521C90`** — `FUN_0042C476` (`0x0042C476`) is
    /// `DAT_0051FBBC - sum(count * weight)` over all **eleven** slots, and
    /// `DAT_0051FBBC` is the strength `Skirmish_FillArmies` gave the table
    /// row. So the allowance is the row's own strength, respent.
    pub fn spare_strength(&self, table: &TroopsTable) -> i32 {
        let row = table.counts(self.kind, self.row, self.side_of(true), self.difficulty[0]);
        let allowance: i32 =
            row.iter().zip(STRENGTH_WEIGHT).map(|(&c, w)| c as i32 * w).sum();
        let now = self.edited.unwrap_or(row);
        allowance - now.iter().zip(STRENGTH_WEIGHT).map(|(&c, w)| c as i32 * w).sum::<i32>()
    }

    /// **`FUN_0042C4E2` (`0x0042C4E2`, `00420000.c:4821-4837`)** — may one
    /// more of this slot be bought?
    ///
    /// ```c
    /// if (6 < slot) {
    ///   if (DAT_0053E91C != 2) return false;                 not a castle
    ///   if (DAT_0053EF5C == g_localPlayer && slot != 10) return false;
    ///   if (DAT_0053EF5C == DAT_0056D5CC && slot == 10) return false;
    ///   if (8 < count[slot]) return false;                   nine engines
    /// }
    /// return g_troopStrengthWeight[slot] <= DAT_00521C90;
    /// ```
    ///
    /// So the attacker may buy **only** slot 10 of the four engines and the
    /// defender may buy any of them **but** slot 10, each capped at nine.
    pub fn may_buy(&self, table: &TroopsTable, slot: usize) -> bool {
        let counts = self.counts_now(table);
        if slot > 6 {
            if self.kind != 2 {
                return false;
            }
            if self.local_defends != (slot == 10) {
                return false;
            }
            if counts[slot] > 8 {
                return false;
            }
        }
        STRENGTH_WEIGHT[slot] <= self.spare_strength(table)
    }

    fn counts_now(&self, table: &TroopsTable) -> [i16; 11] {
        self.edited
            .unwrap_or_else(|| table.counts(self.kind, self.row, self.side_of(true), self.difficulty[0]))
    }

    /// One press of one of the four columns. `FUN_0043D882` and
    /// `FUN_0043D8B9` are `while` loops over the other two
    /// (`00430000.c:7998-8003`, `:8017-8019`), so they are the same two arms
    /// run to exhaustion.
    pub fn count_arm(&mut self, table: &TroopsTable, col: usize, slot: usize) {
        match col {
            crate::screens::setup::ONE_MORE => {
                self.buy_one(table, slot);
            }
            crate::screens::setup::ONE_FEWER => {
                self.sell_one(table, slot);
            }
            crate::screens::setup::ALL_MORE => while self.buy_one(table, slot) {},
            _ => while self.sell_one(table, slot) {},
        }
    }

    /// `FUN_0043D6C3` (`00430000.c:7946`): `if (FUN_0042C4E2(slot)) count++`,
    /// then `menTotal` is resummed over the first seven.
    fn buy_one(&mut self, table: &TroopsTable, slot: usize) -> bool {
        if !self.may_buy(table, slot) {
            return false;
        }
        let mut counts = self.counts_now(table);
        counts[slot] += 1;
        self.edited = Some(counts);
        true
    }

    /// `FUN_0043D7A7` (`00430000.c:7975`): the only guard is `0 < count`.
    fn sell_one(&mut self, table: &TroopsTable, slot: usize) -> bool {
        let mut counts = self.counts_now(table);
        if counts[slot] <= 0 {
            return false;
        }
        counts[slot] -= 1;
        self.edited = Some(counts);
        true
    }
}
