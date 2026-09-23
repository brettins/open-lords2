//! What a battle click changes in the runner, as data. Alone it applies at
//! once; in a network game it waits for its tick. `docs/netcode.md` §4.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleOp {
    Pick { lo: (u8, u8), hi: (u8, u8), commit: bool },
    Drop { figure: usize },
    Clear,
    Recall { figures: Vec<usize> },
    Order { cell: (u8, u8), target: Option<usize>, woodland: bool },
    Formation { unit: usize, formation: Formation },
    Pause,
    Charge,
    Sally,
    Autocalc,
    SkipOutcome,
}

impl LiveBattle {
    /// `true` when it applied, or was queued.
    pub(crate) fn act(&mut self, op: BattleOp) -> bool {
        if self.deferred {
            self.outbox.push(op);
            return true;
        }
        let me = self.owner;
        self.apply(me, op)
    }

    /// `realm` is the issuing seat's; a figure or unit of another realm is refused.
    pub fn apply(&mut self, realm: u8, op: BattleOp) -> bool {
        let owns = |r: &BattleRunner, f: usize| {
            r.fighters.get(f).is_some_and(|f| r.sim.figures[f.sim].owner == realm)
        };
        let regroup = match op {
            BattleOp::Pick { lo, hi, commit } => {
                self.runner.pick_box(realm, lo, hi, commit);
                commit
            }
            BattleOp::Drop { figure } => {
                if !owns(&self.runner, figure) {
                    return false;
                }
                self.runner.deselect_figure(figure);
                true
            }
            BattleOp::Clear => {
                self.runner.clear_selection(realm);
                if realm == self.owner {
                    self.current_unit = 0;
                }
                false
            }
            BattleOp::Recall { figures } => {
                self.runner.clear_selection(realm);
                // `select_figure` refuses another realm's man.
                for f in figures {
                    self.runner.select_figure(f, realm);
                }
                true
            }
            BattleOp::Order { cell, target, woodland } => {
                // `Battle_OrderClicked` and `BattleMap_Click` test `DAT_0053F238`.
                if self.paused || !self.runner.order_selected(realm, cell.0, cell.1, target, woodland) {
                    return false;
                }
                true
            }
            BattleOp::Formation { unit, formation } => {
                if unit == 0 {
                    return false;
                }
                self.runner.order_formation(unit, formation);
                false
            }
            BattleOp::Pause => {
                if self.choice_owner == 0 {
                    return false;
                }
                self.paused = !self.paused;
                false
            }
            BattleOp::Charge => {
                // Battle_ChargeButton 0x0043BD02 00430000.c:6857 `if ((g_battleChoiceOwner != 0) && (DAT_0055322c == 0))`
                if self.choice_owner == 0 || self.has_charged(realm) {
                    return false;
                }
                self.charged |= 1 << realm;
                self.runner.charge_all(realm);
                false
            }
            BattleOp::Sally => {
                if self.sallied || !self.runner.lower_drawbridge() {
                    return false;
                }
                self.sallied = true;
                false
            }
            BattleOp::Autocalc => {
                self.autocalc = true;
                false
            }
            BattleOp::SkipOutcome => {
                if self.mode != Mode::Outcome {
                    return false;
                }
                self.outcome_ticks = OUTCOME_FRAMES + 1;
                false
            }
        };
        if regroup {
            let unit = self.runner.regroup_selection(realm);
            if realm == self.owner {
                self.current_unit = unit;
            }
        }
        self.redraw = true;
        true
    }
}
