#![allow(unused_imports)]
use super::*;

use super::*;
use super::handlers::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

#[allow(clippy::wrong_self_convention)]
impl World<'_> {
    pub(crate) fn unit_pos(&self, u: usize) -> (i16, i16) {
        let u = self.units.get(u);
        (u.x, u.y)
    }

    pub(crate) fn set_target(&mut self, cur: usize, x: i16, y: i16) {
        let u = self.units.get_mut(cur);
        u.target_x = x;
        u.target_y = y;
    }

    /// The three destination-writing orders all clear the withdrawing flag
    /// first. `Order_StepAwayFromUnit` is the one that sets it.
    fn clear_withdrawing(&mut self, cur: usize) {
        self.units.get_mut(cur).withdrawing = false;
    }

    fn side_index(&self, cur: usize) -> usize {
        if self.units.get(cur).side == 0 {
            0
        } else {
            1
        }
    }

    /// `Enemy_NearestUnit` (`0x0048EF45`): nearest **enemy unit** by Chebyshev
    /// distance between the two units' centres, within `max_dist`, holding at
    /// least `min_figures` figures. `0` when nothing qualifies.
    ///
    /// "Enemy" is *a different owner byte*, not a different side. No weighting
    /// of any kind — no troop type, no strength, no facing, no threat.
    pub fn nearest_enemy_unit(&self, cur: usize, max_dist: i32, min_figures: u8) -> usize {
        let mine = self.units.get(cur).owner;
        let (x, y) = self.unit_pos(cur);
        let mut best = 0usize;
        // 160000 in the original: larger than any distance on an 80-cell map.
        let mut best_dist = 160_000i32;
        for i in 1..=MAX_UNITS {
            let u = self.units.get(i);
            if !u.is_live() || u.owner == mine || u.figures < min_figures {
                continue;
            }
            let d = chebyshev(x, y, u.x, u.y);
            if d <= max_dist && d < best_dist {
                best = i;
                best_dist = d;
            }
        }
        best
    }

    // --- the action vocabulary ---------------------------------------------
    //
    // "There is no formation change, no facing order, no fire-at-will toggle,
    // no reserve, and no withdrawal from the field. The AI's entire vocabulary
    // is *go here*, *shoot that unit* and *everybody charge*."

    /// `Order_HoldPosition`: destination = own position.
    pub(crate) fn hold_position(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(cur);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::HoldPosition);
    }

    /// `Order_OntoUnit`: walk straight at another unit.
    pub(crate) fn onto_unit(&mut self, cur: usize, other: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(other);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::OntoUnit(other));
    }

    /// `Order_HalfwayToUnit`: halve the separation per axis.
    ///
    /// It does **nothing at all** unless one axis is separated by eight cells
    /// or more, and then moves only the axes separated by six or more. That,
    /// plus `Order_StopShortOfTarget` pulling a missile unit's destination back
    /// to `range/8 − 3`, is why AI archers converge on a standoff distance
    ///
    pub(crate) fn halfway_to_unit(&mut self, cur: usize, other: usize) {
        self.clear_withdrawing(cur);
        let (sx, sy) = self.unit_pos(cur);
        let (ox, oy) = self.unit_pos(other);
        let hx = ((ox as i32 - sx as i32).abs()) / 2;
        let hy = ((oy as i32 - sy as i32).abs()) / 2;
        if hx <= 3 && hy <= 3 {
            return;
        }
        let (mut tx, mut ty) = (sx, sy);
        if hx > 2 {
            tx = if sx < ox { sx + hx as i16 } else { sx - hx as i16 };
        }
        if hy > 2 {
            ty = if sy < oy { sy + hy as i16 } else { sy - hy as i16 };
        }
        self.set_target(cur, tx, ty);
        self.ai.record(cur, Action::HalfwayToUnit(other));
    }

    /// `Order_StepAwayFromUnit`: shift the **destination** two cells away.
    ///
/// The destination, so repeated withdrawals
    /// compound. Along the longer axis, and along both once an axis separation
    /// exceeds five. This is the whole of the AI's retreat behaviour.
    pub(crate) fn step_away_from_unit(&mut self, cur: usize, other: usize) {
        let (ox, oy) = self.unit_pos(other);
        let (sx, sy) = self.unit_pos(cur);
        let (adx, ady) = (
            (ox as i32 - sx as i32).abs(),
            (oy as i32 - sy as i32).abs(),
        );
        let (mut dx, mut dy) = (0i16, 0i16);
        if ady < adx {
            dx = if ox < sx { 2 } else { -2 };
        } else {
            dy = if oy < sy { 2 } else { -2 };
        }
        if adx > 5 {
            dx = if ox < sx { 2 } else { -2 };
        }
        if ady > 5 {
            dy = if oy < sy { 2 } else { -2 };
        }
        {
            let u = self.units.get_mut(cur);
            u.withdrawing = true;
            u.withdrawals = u.withdrawals.wrapping_add(1);
            u.target_x += dx;
            u.target_y += dy;
        }
        self.ai.record(cur, Action::StepAwayFromUnit(other));
    }

    /// `Order_StepTowardRallyPoint`: the mirror of the above, toward
    /// `rally_x/y`, **and it clears the request** — so the first missile unit
    /// to think answers the call and the rest do not.
    pub(crate) fn step_toward_rally_point(&mut self, cur: usize) {
        let (rx, ry) = (self.ai.rally_x, self.ai.rally_y);
        self.ai.rally_request = false;
        let (sx, sy) = self.unit_pos(cur);
        let (adx, ady) = ((rx - sx as i32).abs(), (ry - sy as i32).abs());
        let (mut dx, mut dy) = (0i16, 0i16);
        if ady < adx {
            dx = if rx < sx as i32 { -2 } else { 2 };
        } else {
            dy = if ry < sy as i32 { -2 } else { 2 };
        }
        if adx > 5 {
            dx = if rx < sx as i32 { -2 } else { 2 };
        }
        if ady > 5 {
            dy = if ry < sy as i32 { -2 } else { 2 };
        }
        {
            let u = self.units.get_mut(cur);
            u.target_x += dx;
            u.target_y += dy;
        }
        self.ai.record(cur, Action::StepTowardRallyPoint);
    }

    /// `Order_ToRallyWaypoint`: one of three waypoints for this side, in the
    /// group `Battle_Start` picked.
    pub(crate) fn to_rally_waypoint(&mut self, cur: usize, index: usize) {
        let side = self.side_index(cur);
        let (x, y) = self.field.rally[side][self.ai.rally_group][index];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToRallyWaypoint(index));
    }

    /// `BattleUnit_OrderToEnemyEnd`: a slot of the *opposing* side's
    /// deployment marker.
    pub(crate) fn to_enemy_end(&mut self, cur: usize, slot: usize) {
        self.clear_withdrawing(cur);
        let side = self.side_index(cur);
        let (x, y) = self.field.enemy_end[side][slot & 3];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToEnemyEnd(slot & 3));
    }

    /// `Order_ChargeNearest` (`0x0048C8AF`) — the most consequential action.
    ///
    /// It sets the unit's halted flag, which switches the every-500-frame
    /// reform off, and puts **every** figure of the unit into free pursuit.
    /// From then on each figure picks its own victim and ignores the unit's
    /// destination: *a charged unit stops being a formation*.
    pub(crate) fn charge_nearest(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        self.units.get_mut(cur).halted = true;
        for f in self.figures.iter_mut() {
            if f.is_alive() && f.unit as usize == cur {
                f.state = State::Chasing;
                f.target = None;
            }
        }
        self.ai.record(cur, Action::Charge);
    }

    /// `Order_ShootAtUnit` (`0x0048C974`): every figure of this unit takes a
    /// target within forty cells, restricted to `unit` when that is non-zero,
    /// and enters state 17.
    ///
    /// Returns false if **any** figure found nothing, which the siege missile
    /// handler reads as "that unit is out of reach". Note the odd exclusion the
    /// original carries and this reproduces: a shooter standing on surface 1
    /// will not take a target standing on surface 5.
    pub(crate) fn shoot_at_unit(&mut self, cur: usize, unit: usize) -> bool {
        let mut all_found = true;
        for f in 0..self.figures.len() {
            if !self.figures[f].is_alive() || self.figures[f].unit as usize != cur {
                continue;
            }
            let Some(target) = self.find_missile_target(f, 40, unit) else {
                all_found = false;
                continue;
            };
            let here = self.positions.get(f).copied().unwrap_or((0, 0));
            let there = self.positions.get(target).copied().unwrap_or((0, 0));
            let blocked = self.field.surface_at(here.0 as i32, here.1 as i32) == Some(1)
                && self.field.surface_at(there.0 as i32, there.1 as i32) == Some(5);
            if !blocked {
                self.figures[f].state = State::Shooting;
                self.figures[f].target = Some(target);
            }
        }
        self.ai.record(cur, Action::ShootAtUnit(unit));
        all_found
    }

    /// `Missile_FindTarget` (`0x004956CC`), reduced to what the order handlers
    /// need of it.
    ///
    /// Score is Manhattan distance capped at 160; a siege-engine target costs
    /// +35 and is invisible to a melee figure; and the *range* test is a square
    /// box even though the *score* is Manhattan. `restrict` limits the search
    /// to one unit when non-zero.
    fn find_missile_target(&self, shooter: usize, range: i32, restrict: usize) -> Option<usize> {
        let me = &self.figures[shooter];
        let mine = me.owner;
        let melee_only = !matches!(me.troop, Troop::Crossbowmen | Troop::Archers | Troop::Catapults);
        let &(sx, sy) = self.positions.get(shooter)?;
        let (sx, sy) = (sx as i32, sy as i32);
        let mut best: Option<(i32, usize)> = None;
        for (i, f) in self.figures.iter().enumerate() {
            if !f.is_alive() || f.owner == mine || f.owner == 0 {
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

    // --- the siege position vocabulary -------------------------------------

    /// `Order_ToCell`: a cell index straight into the destination.
    pub(crate) fn to_cell(&mut self, cur: usize, cell: usize) {
        let (x, y) = ((cell % DIM) as i16, (cell / DIM) as i16);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToCell);
    }

    /// `Order_ToFieldCorner`: `(6, 74)` or `(74, 74)` by castle layout, and a
    /// castle approach point when the layout is neither 1 nor 2.
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

    /// `Order_ToCastleApproach`: one of four approach points per index, by
    /// lane. Rams (category 7) always use the primary table; everyone else
    /// switches on the castle orientation flag.
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

    /// `Order_ToSiegeStaging`: the primary approach table below approach score
    /// 16 or above 399, the secondary five cells further in between, or a fixed
    /// offset from the castle when the orientation flag is set.
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

    /// `Order_ToBreachOrStaging`: below approach score 16 it looks for the
    /// nearest **moat** cell at radii 12…19 and walks onto it; between 16 and
    /// 400 it falls back on the secondary staging table; above 400 it does
    /// nothing at all.
    ///
    /// The name in `docs/battle-ai.md` reads it as a breach. The cell it
    /// searches for is surface **2**, which `docs/battle.md` §7 and the
    /// state-9 moat handler both give as water — so the early phase of a siege
    /// is an order to go and fill the moat in.
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

    /// `Order_ToCastleObjective`: one of the two cells the castle builder
    /// recorded.
    pub(crate) fn to_castle_objective(&mut self, cur: usize, mode: usize) {
        let cell = if mode == 1 || self.ai.ramparts_breached >= 1 || self.field.castle_index == 13 {
            self.field.castle_objective[0]
        } else {
            self.field.castle_objective[1]
        };
        self.to_cell(cur, cell);
        self.ai.record(cur, Action::ToCastleObjective(mode));
    }

    /// `Order_ToWallSlot`: one of sixteen positions in a group, **skipping
    /// empty entries and wrapping**, offset one cell west and two north of the
    /// recorded position.
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

    /// `Order_ToNearestWallCell`: the nearest surface-4 cell within forty.
    pub(crate) fn to_nearest_wall_cell(&mut self, cur: usize) {
        let (sx, sy) = self.unit_pos(cur);
        if let Some((x, y)) = self.nearest_surface(sx as i32, sy as i32, 40, 4, true) {
            self.set_target(cur, x as i16, y as i16);
            self.ai.record(cur, Action::ToNearestWallCell);
        } else {
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToWallBelowKeep`: the primary castle cell, four cells south, then
    /// the nearest surface-4 cell within five of *that*.
    pub(crate) fn to_wall_below_keep(&mut self, cur: usize) {
        let cell = self.field.castle_objective[0];
        let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32 + 4);
        if let Some((wx, wy)) = self.nearest_surface(x, y, 5, 4, true) {
            self.set_target(cur, wx as i16, wy as i16);
            self.ai.record(cur, Action::ToWallBelowKeep);
        } else {
            // The original writes the offset destination *before* the search
            // and only re-issues the order when the search succeeds, so a
            // failed search leaves the destination changed but unordered.
            self.ai.record(cur, Action::DoNothing);
        }
    }

    /// `Order_ToSurface5Near`: the nearest surface-5 cell within five of a
    /// given cell.
    ///
    /// **Reproduces an original bug.** `Siege_FindCellSurface5` measures its
    /// distance from the *clipped top-left corner of the search box*, not from
    /// the cell it was asked about — the local holding the query point is
    /// overwritten by the clipping arithmetic before the distance call. Its
    /// sibling `Siege_FindCellSurface4` saves the query point first and does
    /// not have the fault. Left as written: the original's choice of cell is
    /// the specification, and "fixing" it moves defenders somewhere else.
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

    /// `Order_ToWallNearPreferredTarget` / `Order_ToWallNearAvoidedTarget`.
    ///
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

    /// `Siege_FindCellSurface4` / `Siege_FindCellSurface5`: nearest cell of a
    /// given surface inside a square box, clipped to the map.
    ///
    /// `manhattan` picks the metric — the original uses Manhattan distance when
    /// its `maxElevation` argument is 4 and `min(|dx|, |dy|)` otherwise.
    pub(crate) fn nearest_surface(
        &self,
        x: i32,
        y: i32,
        radius: i32,
        surface: u8,
        manhattan: bool,
    ) -> Option<(i32, i32)> {
        if self.field.surface.is_empty() {
            return None;
        }
        let mut best: Option<(i32, i32, i32)> = None;
        for cy in (y - radius).max(0)..=(y + radius).min(DIM as i32 - 1) {
            for cx in (x - radius).max(0)..=(x + radius).min(DIM as i32 - 1) {
                if self.field.surface_at(cx, cy) != Some(surface) {
                    continue;
                }
                let (dx, dy) = ((cx - x).abs(), (cy - y).abs());
                let d = if manhattan { dx + dy } else { dx.min(dy) };
// `<=`: the original keeps the *last* equally
                // close cell it finds, which fixes the tie-break to scan order.
                if best.is_none_or(|(b, _, _)| d <= b) {
                    best = Some((d, cx, cy));
                }
            }
        }
        best.map(|(_, cx, cy)| (cx, cy))
    }

    /// The faulty sibling — see [`to_surface5_near`](Self::to_surface5_near).
    fn nearest_surface_from_box_corner(
        &self,
        x: i32,
        y: i32,
        radius: i32,
        surface: u8,
    ) -> Option<(i32, i32)> {
        if self.field.surface.is_empty() {
            return None;
        }
        let (ox, oy) = ((x - radius).max(0), (y - radius).max(0));
        let mut best: Option<(i32, i32, i32)> = None;
        for cy in oy..=(y + radius).min(DIM as i32 - 1) {
            for cx in ox..=(x + radius).min(DIM as i32 - 1) {
                if self.field.surface_at(cx, cy) != Some(surface) {
                    continue;
                }
                let d = (cx - ox).abs() + (cy - oy).abs();
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, cx, cy));
                }
            }
        }
        best.map(|(_, cx, cy)| (cx, cy))
    }

    /// `Siege_ClaimDefencePost` (`0x0048ED95`): a twenty-entry reservation
    /// table that stops two defending units posting to the same place. Returns
    /// the cell already reserved for this unit, or reserves the first free
    /// non-empty entry. `0` when the table is full.
    pub(crate) fn claim_defence_post(&mut self, cur: usize) -> usize {
        for i in 0..20 {
            if self.ai.defence_post_owner[i] as usize == cur && cur != 0 {
                return self.field.defence_posts[i];
            }
        }
        for i in 0..20 {
            if self.field.defence_posts[i] != 0 && self.ai.defence_post_owner[i] == 0 {
                self.ai.defence_post_owner[i] = cur as u16;
                return self.field.defence_posts[i];
            }
        }
        0
    }

    /// `Oil_IsOnHighWall`: elevation 2 or more on a cell of surface 6 or above.
    pub(crate) fn oil_is_on_high_wall(&self, cur: usize) -> bool {
        let (x, y) = self.unit_pos(cur);
        self.field.elevation_at(x as i32, y as i32) >= 2
            && self.field.surface_at(x as i32, y as i32).unwrap_or(0) >= 6
    }

    /// `Oil_FindPourTarget` → `Enemy_FindCluster`: nothing below elevation 2;
    /// otherwise the densest enemy cluster within six cells needing three
    /// enemies, or within four needing two once the unit is on the rampart
    /// proper. A genuine "wait until they bunch up under you" rule.
    pub(crate) fn oil_find_pour_target(&self, cur: usize) -> Option<(i16, i16)> {
        let (ux, uy) = self.unit_pos(cur);
        if self.field.elevation_at(ux as i32, uy as i32) < 2 {
            return None;
        }
        let on_rampart = self.field.surface_at(ux as i32, uy as i32).unwrap_or(0) >= 6;
        let (radius, needed) = if on_rampart { (4i32, 2usize) } else { (6i32, 3usize) };
        let mine = self.units.get(cur).owner;
        let mut best: Option<(usize, i32, i32)> = None;
        for cy in (uy as i32 - radius).max(0)..=(uy as i32 + radius).min(DIM as i32 - 1) {
            for cx in (ux as i32 - radius).max(0)..=(ux as i32 + radius).min(DIM as i32 - 1) {
                let mut count = 0usize;
                for (i, f) in self.figures.iter().enumerate() {
                    if !f.is_alive() || f.owner == mine || f.owner == 0 {
                        continue;
                    }
                    let Some(&(x, y)) = self.positions.get(i) else { continue };
                    if (x as i32 - cx).abs() <= 1 && (y as i32 - cy).abs() <= 1 {
                        count += 1;
                    }
                }
                if count >= needed && best.is_none_or(|(b, _, _)| count > b) {
                    best = Some((count, cx, cy));
                }
            }
        }
        best.map(|(_, x, y)| (x as i16, y as i16))
    }

    // --- the think gate ----------------------------------------------------

    /// The skeleton every handler opens with: count the think timer up, and run
    /// only when it passes its interval and (in 13 of the 17) no figure of the
    /// unit is in melee.
    pub(crate) fn may_think(&mut self, cur: usize, interval: i16, melee_gates: bool) -> bool {
        let u = self.units.get_mut(cur);
        u.think = u.think.saturating_add(1);
        if u.think < interval || (melee_gates && u.in_melee) {
            return false;
        }
        u.think = 0;
        true
    }

    /// The tail: `orders += 1`. Two handlers return before reaching it.
    pub(crate) fn advance_script(&mut self, cur: usize) {
        let u = self.units.get_mut(cur);
        u.orders = u.orders.wrapping_add(1);
    }

    /// The liveness test every handler makes on its remembered attacker:
    /// the grudge must be live *and* the attacker must still exist.
    pub(crate) fn live_attacker(&self, cur: usize) -> Option<usize> {
        let u = self.units.get(cur);
        if u.hit_memory == 0 {
            return None;
        }
        let a = u.last_attacker as usize;
        if self.units.owner_of(a) == 0 {
            return None;
        }
        Some(a)
    }
}

