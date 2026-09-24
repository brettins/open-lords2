#![allow(unused_imports)]
use super::*;
use super::targets::*;
use super::surfaces::*;
use super::super::*;
use super::super::*;
use super::super::handlers::*;
use super::super::tests::*;
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

    /// `BattleUnit_Order` (`0x00479E90`) as every AI order routine calls it,
    /// `(cur, targX, targY, 0, 0, 0)`: the figures move at the unit's reform,
    /// not here. `[V]` 00470000.c BattleUnit_Order; Battle_UpdateAllUnits 0x00489401
    pub(crate) fn set_target(&mut self, cur: usize, x: i16, y: i16) {
        // `if (bVar1 == 2 && humanControlled == 0) field_0x13 = 0` [V]
        let surface2 = self.field.surface_at(x as i32, y as i32) == Some(2);
        let u = self.units.get_mut(cur);
        // `if (inMelee != 0) orderLock = 0x40` [V]
        if u.in_melee {
            u.order_lock = crate::unit::ORDER_LOCK;
        }
        // `destIsWoodland == 0`: `targetCell = 0` [V]
        u.target_cell = 0;
        u.target_x = x;
        u.target_y = y;
        // `reTarg = 0x14; field_0x13 = 1` [V]
        u.reform = crate::unit::REFORM_ON_ORDER;
        u.reform_gate = !(surface2 && !u.human);
        // `BattleUnit_Order` (`0x00479E90`) opens with `movStraff = 0` for every
        // figure of the unit, whether or not the destination moved — and every
        // AI order routine reaches the figures through it. Without this a
        // closing archer that has shot once never follows again. `[V]`
        for f in self.figures.iter_mut() {
            if f.unit as usize == cur {
                f.straffed = false;
            }
        }
    }

    pub(crate) fn clear_withdrawing(&mut self, cur: usize) {
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
    pub fn nearest_enemy_unit(&self, cur: usize, max_dist: i32, min_figures: u8) -> usize {
        let mine = self.units.get(cur).owner;
        let (x, y) = self.unit_pos(cur);
        let mut best = 0usize;
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


    pub(crate) fn hold_position(&mut self, cur: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(cur);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::HoldPosition);
        l2_net::trace!("call", 0x0048B386, "Order_HoldPosition", unit = cur);
    }

    pub(crate) fn onto_unit(&mut self, cur: usize, other: usize) {
        self.clear_withdrawing(cur);
        let (x, y) = self.unit_pos(other);
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::OntoUnit(other));
        l2_net::trace!("call", 0x0048C4BD, "Order_OntoUnit", unit = other);
    }

    pub(crate) fn halfway_to_unit(&mut self, cur: usize, other: usize) {
        l2_net::trace!("call", 0x0048C56F, "Order_HalfwayToUnit", unit = other);
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

    pub(crate) fn step_away_from_unit(&mut self, cur: usize, other: usize) {
        l2_net::trace!("call", 0x0048B5FC, "Order_StepAwayFromUnit", unit = other);
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
        }
        // 0x0048B5FC ends `BattleUnit_Order(cur, targX, targY, 0, 0, 0)` [V]
        let (tx, ty) = (self.units.get(cur).target_x, self.units.get(cur).target_y);
        self.set_target(cur, tx + dx, ty + dy);
        self.ai.record(cur, Action::StepAwayFromUnit(other));
    }

    pub(crate) fn step_toward_rally_point(&mut self, cur: usize) {
        l2_net::trace!("call", 0x0048B7FC, "Order_StepTowardRallyPoint");
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
        // 0x0048B7FC ends `BattleUnit_Order(cur, targX, targY, 0, 0, 0)` [V]
        let (tx, ty) = (self.units.get(cur).target_x, self.units.get(cur).target_y);
        self.set_target(cur, tx + dx, ty + dy);
        self.ai.record(cur, Action::StepTowardRallyPoint);
    }

    pub(crate) fn to_rally_waypoint(&mut self, cur: usize, index: usize) {
        let side = self.side_index(cur);
        let (x, y) = self.field.rally[side][self.ai.rally_group][index];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToRallyWaypoint(index));
        l2_net::trace!("call", 0x0048BAC4, "Order_ToRallyWaypoint", index = index);
        l2_net::trace!("call", 0x00479E90, "BattleUnit_Order", unit = cur, x = x, y = y, a = 0, b = 0, c = 0);
    }

    pub(crate) fn to_enemy_end(&mut self, cur: usize, slot: usize) {
        self.clear_withdrawing(cur);
        let side = self.side_index(cur);
        let (x, y) = self.field.enemy_end[side][slot & 3];
        self.set_target(cur, x, y);
        self.ai.record(cur, Action::ToEnemyEnd(slot & 3));
        l2_net::trace!("call", 0x00479E90, "BattleUnit_Order", unit = cur, x = x, y = y, a = 0, b = 0, c = 0);
    }

    /// `Order_ChargeNearest` (`0x0048C8AF`) — the most consequential action.
    pub(crate) fn charge_nearest(&mut self, cur: usize) {
        l2_net::trace!("call", 0x0048C8AF, "Order_ChargeNearest", unit = cur);
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
    pub(crate) fn shoot_at_unit(&mut self, cur: usize, unit: usize) -> bool {
        l2_net::trace!("call", 0x0048C974, "Order_ShootAtUnit", unit = unit);
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

}
