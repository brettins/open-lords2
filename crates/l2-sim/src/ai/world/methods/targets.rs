#![allow(unused_imports)]
use super::*;
use super::moves::*;
use super::surfaces::*;
use super::super::*;
use super::super::*;
use super::super::handlers::*;
use super::super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

impl World<'_> {
    /// `Missile_FindTarget` (`0x004956CC`), reduced to what the order handlers
    /// need of it.
    pub(crate) fn find_missile_target(&self, shooter: usize, range: i32, restrict: usize) -> Option<usize> {
        let me = &self.figures[shooter];
        let mine = me.owner;
        let melee_only = !matches!(me.troop, Troop::Crossbowmen | Troop::Archers | Troop::Catapults);
        let &(sx, sy) = self.positions.get(shooter)?;
        let (sx, sy) = (sx as i32, sy as i32);
        let mut best: Option<(i32, usize)> = None;
        for (i, f) in self.figures.iter().enumerate() {
            // Missile_FindTarget 0x004956CC: owner != 0, other owner, `state != 2` [V]
            if f.state == State::Dead || f.owner == mine || f.owner == 0 {
                continue;
            }
            if restrict != 0 && f.unit as usize != restrict {
                continue;
            }
            if f.troop.is_siege() && melee_only {
                continue;
            }
            let Some(&(x, y)) = self.positions.get(i) else { continue };
            let (dx, dy) = ((x as i32 - sx).abs(), (y as i32 - sy).abs());
            if dx > range || dy > range {
                continue;
            }
            let mut score = (dx + dy).min(160);
            if f.troop.is_siege() {
                score += 35;
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, i));
            }
        }
        best.map(|(_, i)| i)
    }


    pub(crate) fn to_cell(&mut self, cur: usize, cell: usize) {
        let (x, y) = ((cell % DIM) as i16, (cell / DIM) as i16);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToCell);
    }

    pub(crate) fn to_field_corner(&mut self, cur: usize, index: usize) {
        self.clear_withdrawing(cur);
        match self.field.layout {
            1 => {
                self.set_target(cur, 6, 74);
                self.ai.record(cur, Action::ToFieldCorner);
            }
            2 => {
                self.set_target(cur, 74, 74);
                self.ai.record(cur, Action::ToFieldCorner);
            }
            _ => self.to_castle_approach(cur, index),
        }
    }

    pub(crate) fn to_castle_approach(&mut self, cur: usize, index: usize) {
        self.clear_withdrawing(cur);
        let category = self.units.get(cur).category;
        let lane = self.ai.approach_lane & 3;
        let (x, y) = if category == 7 || self.field.orientation == 0 {
            self.field.castle_approach[index.min(5)][lane]
        } else if category == 1 {
            (self.field.castle_ref.0 + 10, self.field.castle_ref.1 + 10)
        } else {
            (self.field.castle_ref.0 + 2, self.field.castle_ref.1 + 15)
        };
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToCastleApproach(index));
    }

    pub(crate) fn to_siege_staging(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        let lane = self.ai.approach_lane & 3;
        let (x, y) = if self.field.orientation == 0 {
            if self.ai.approach_score < 16 || self.ai.approach_score > 399 {
                self.field.castle_approach[0][lane]
            } else {
                let p = self.field.staging[lane];
                (p.0, p.1 + 5)
            }
        } else {
            (self.field.castle_ref.0 + 2, self.field.castle_ref.1 + 12)
        };
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToSiegeStaging);
    }

    pub(crate) fn to_breach_or_staging(&mut self, cur: usize) {
        if self.ai.approach_score >= 401 {
            self.ai.record(cur, Action::DoNothing);
            return;
        }
        if self.ai.approach_score < 16 {
            let (sx, sy) = self.unit_pos(cur);
            for radius in 12..20 {
                if let Some((x, y)) = self.nearest_surface(sx as i32, sy as i32, radius, 2, false) {
                    self.set_target(cur, x as i16, y as i16);
                    self.ai.record(cur, Action::ToMoatCell);
                    return;
                }
            }
            self.ai.record(cur, Action::DoNothing);
            return;
        }
        let (x, y) = self.field.staging[self.ai.approach_lane & 3];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToSiegeStaging);
    }

    pub(crate) fn to_castle_objective(&mut self, cur: usize, mode: usize) {
        let cell = if mode == 1 || self.ai.ramparts_breached >= 1 || self.field.castle_index == 13 {
            self.field.castle_objective[0]
        } else {
            self.field.castle_objective[1]
        };
        self.to_cell(cur, cell);
        self.ai.record(cur, Action::ToCastleObjective(mode));
    }

    pub(crate) fn to_wall_slot(&mut self, cur: usize, group: usize, slot: usize) {
        self.clear_withdrawing(cur);
        let mut slot = slot % 16;
        let mut guard = 17;
        while guard > 0 {
            let (x, y) = self.field.wall_slot[group][slot];
            if x != 0 && y != 0 {
                break;
            }
            slot = (slot + 1) % 16;
            guard -= 1;
        }
        let (x, y) = self.field.wall_slot[group][slot];
        self.set_target(cur, x - 1, y - 2);
        self.ai.record(cur, Action::ToWallSlot(group, slot));
    }

    pub(crate) fn to_nearest_wall_cell(&mut self, cur: usize) {
        let (sx, sy) = self.unit_pos(cur);
        if let Some((x, y)) = self.nearest_surface(sx as i32, sy as i32, 40, 4, true) {
            self.set_target(cur, x as i16, y as i16);
            self.ai.record(cur, Action::ToNearestWallCell);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    pub(crate) fn to_wall_below_keep(&mut self, cur: usize) {
        let cell = self.field.castle_objective[0];
        let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32 + 4);
        if let Some((wx, wy)) = self.nearest_surface(x, y, 5, 4, true) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, Action::ToWallBelowKeep);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    pub(crate) fn to_surface5_near(&mut self, cur: usize, cell: usize) {
        let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32);
        if let Some((wx, wy)) = self.nearest_surface_from_box_corner(x, y, 5, 5) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, Action::ToSurface5Near);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToSecondTarget`: the unit's stored second destination. We have no
    /// second destination pair, so this holds the destination it already has —
    /// which is what copying `+0x26/+0x28` onto `+0x22/+0x24` does on a unit
    /// that has never been given one.
    pub(crate) fn to_second_target(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        self.ai.record(cur, Action::ToSecondTarget);
    }

    /// The two weighted variants of `Missile_FindTarget` choose *where to
    /// stand*, not what to shoot: one halves the score of an enemy carrying a
    /// missile weapon and the other doubles it, and the chosen enemy's nearest
    /// wall cell becomes the destination. **[I]** on the wall step — the
    /// weighting is read from the binary, the "then stand beside him" is how
    /// the callers use the result.
    pub(crate) fn to_wall_near_target(&mut self, cur: usize, prefer_shooters: bool) {
        let (sx, sy) = self.unit_pos(cur);
        let mine = self.units.get(cur).owner;
        let mut best: Option<(i32, usize)> = None;
        for (i, f) in self.figures.iter().enumerate() {
            if !f.is_alive() || f.owner == mine || f.owner == 0 {
                continue;
            }
            let Some(&(x, y)) = self.positions.get(i) else { continue };
            let mut score =
                ((x as i32 - sx as i32).abs() + (y as i32 - sy as i32).abs()).min(160);
            if f.state == State::FillingMoat {
                score /= 4;
            }
            let shooter = matches!(f.troop, Troop::Crossbowmen | Troop::Archers);
            if shooter {
                if prefer_shooters {
                    score /= 2;
                } else {
                    score *= 2;
                }
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, i));
            }
        }
        let action = if prefer_shooters {
            Action::ToWallNearPreferredTarget
        } else {
            Action::ToWallNearAvoidedTarget
        };
        let Some((_, target)) = best else {
            self.ai.record(cur, Action::DoNothing);
            return;
        };
        let (tx, ty) = self.positions[target];
        if let Some((wx, wy)) = self.nearest_surface(tx as i32, ty as i32, 5, 4, true) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, action);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

}
