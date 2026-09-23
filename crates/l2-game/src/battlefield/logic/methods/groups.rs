#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::commands::*;
use super::field::*;
use super::super::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

impl LiveBattle {
    /// `FUN_0043C885` (`0x0043C885`) — **Ctrl and a digit stores the
    /// selection**.
    ///
    /// // arm: 0x0043C885/store-group key
    pub fn store_group(&mut self, digit: u8) -> bool {
        let Some(slot) = group_slot(digit) else { return false };
        self.groups[slot] = Some(self.runner.selected_fighters(self.owner));
        true
    }

    /// `FUN_0043C910` (`0x0043C910`) — **a digit recalls it, and moves the
    /// camera to it**.
    ///
    /// // arm: 0x0043C910/recall-group key
    pub fn recall_group(&mut self, digit: u8) -> bool {
        let Some(slot) = group_slot(digit) else { return false };
        let Some(members) = self.groups[slot].clone() else { return false };
        self.act_then(BattleOp::Recall { figures: members }, Follow::Look);
        if !self.deferred {
            self.look_at_selection();
        }
        self.redraw = true;
        true
    }

    pub(crate) fn look_at_selection(&mut self) {
        if let Some(&first) = self.runner.selected_fighters(self.owner).first() {
            let f = &self.runner.fighters[first];
            self.look_at((f.x, f.y));
        }
    }

    /// `FUN_0043C77A` (`0x0043C77A`) — **`H` and `V`**.
    ///
    /// Line and column. The original reads `DAT_0053E984` straight — it does not
    /// regroup first — so pressing `H` after men have died reforms whatever unit
    /// the selection last became, which may no longer be the whole selection.
    ///
    /// **It acts while the battle is paused.** `[V]`, from both of its guards:
    ///
    /// the window procedure's `WM_CHAR` arm asks `g_battlePhase == 2 &&
    /// DAT_0057A0CC == 0`, and `FUN_0043C77A` itself asks `DAT_00553C6C == 0 &&
    /// g_appPhase == 3`. Neither is the pause word, `DAT_0053F238`, which
    /// `Battle_OrderClicked` and `BattleMap_Click` do test — so a click cannot
    /// order a paused battle and `H` can, and a player can set a line or a
    /// column before the fighting starts. This used to refuse while paused, a
    /// guard of ours copied from the click.
    ///
    /// // arm: 0x0043C77A/formation key
    pub fn key_formation(&mut self, formation: Formation) -> bool {
        // `[V]`.
        //
        // sfx: Battle_FormationKey#1
        self.cry(cry::ORDERED);
        // Not a guard of the original's: it orders `DAT_0053E984` whatever it
        // holds. Unit 0 is no unit, and `BattleRunner::order_formation` refuses
        // it too; this only keeps "nothing was turned" out of the redraw.
        if self.current_unit == 0 {
            return false;
        }
        self.act(BattleOp::Formation { unit: self.current_unit, formation });
        self.redraw = true;
        true
    }


    /// `Battle_CheckOutcome` (`0x00477DFC`) raises `0x2B` and then counts
    /// `DAT_00568470` up to 5000 before it returns to the campaign.
    ///
    /// // arm: 0x00477DFC/outcome-timer timer
    pub fn tick_outcome(&mut self) -> bool {
        if self.mode != Mode::Outcome {
            return false;
        }
        self.outcome_ticks += 1;
        self.outcome_ticks > OUTCOME_FRAMES
    }

    /// // arm: 0x0042FF10/skip-outcome right-release
    pub fn skip_outcome(&mut self) -> bool {
        if self.mode != Mode::Outcome {
            return false;
        }
        self.act(BattleOp::SkipOutcome);
        true
    }

    pub fn tick(&mut self) {
        if self.mode == Mode::Outcome {
            self.tick_outcome();
            return;
        }
        if self.paused {
            return;
        }
        self.runner.step();
        self.redraw = true;
        if self.conclusion.is_none() {
            if let Some(c) = self.runner.conclusion() {
                self.conclusion = Some(c);
                self.mode = Mode::Outcome;
                self.outcome_ticks = 0;
            }
        }
    }

    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }
}



