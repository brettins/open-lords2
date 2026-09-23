#![allow(unused_imports)]
use super::*;
use super::moves::*;
use super::targets::*;
use super::super::*;
use super::super::*;
use super::super::handlers::*;
use super::super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

impl World<'_> {
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
                if best.is_none_or(|(b, _, _)| d <= b) {
                    best = Some((d, cx, cy));
                }
            }
        }
        best.map(|(_, cx, cy)| (cx, cy))
    }

    pub(super) fn nearest_surface_from_box_corner(
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

    pub(crate) fn oil_is_on_high_wall(&self, cur: usize) -> bool {
        let (x, y) = self.unit_pos(cur);
        self.field.elevation_at(x as i32, y as i32) >= 2
            && self.field.surface_at(x as i32, y as i32).unwrap_or(0) >= 6
    }

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


    pub(crate) fn may_think(&mut self, cur: usize, interval: i16, melee_gates: bool) -> bool {
        let u = self.units.get_mut(cur);
        u.think = u.think.saturating_add(1);
        if u.think < interval || (melee_gates && u.in_melee) {
            return false;
        }
        u.think = 0;
        true
    }

    pub(crate) fn advance_script(&mut self, cur: usize) {
        let u = self.units.get_mut(cur);
        u.orders = u.orders.wrapping_add(1);
    }

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


