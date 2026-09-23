#![allow(unused_imports)]
use super::*;
use super::movement::*;
use super::targeting::*;
use super::missiles::*;
use super::super::*;

impl BattleRunner {
    pub(super) fn unit_dest(&self, unit: usize) -> (i16, i16) {
        let u = self.units.get(unit);
        (u.target_x.clamp(1, 78), u.target_y.clamp(1, 78))
    }

    /// `BattleUnit_Reform` (`0x0048970E`): re-issue every figure a destination,
    /// from the formation rectangle if it is clear and by search if it is not.
    pub(crate) fn reform_unit(&mut self, unit: usize) {
        let members = self.members(unit);
        if members.is_empty() {
            return;
        }
        let (footprint, cols) = self.unit_geometry(unit, &members);
        let target = self.unit_dest(unit);
        // **`DAT_00553FE4` — "this unit was ordered onto water".**
        let dest_is_water = self.field.at(target.0 as usize, target.1 as usize).surface
            == crate::siege::SURFACE_WATER;
        if self.rect_is_clear(unit, target, members.len(), footprint, cols) {
            let rect = formation::compute_rect(target, members.len(), footprint, cols);
            let mut assigned = vec![false; members.len()];
            for i in 0..members.len() {
                let (x, y) = rect.slot(i);
                let Some(k) = self.nearest_free_figure(&members, &assigned, x, y) else {
                    continue;
                };
                assigned[k] = true;
                self.send_figure(unit, members[k], x, y, dest_is_water);
            }
        } else {
            self.assign_searched_slots(unit, &members, target, dest_is_water);
        }
        self.units.get_mut(unit).reform_gate = false;
    }

    /// `Formation_RectIsClear` (`0x00489F9D`): every slot on the map, at the
    /// destination's elevation, holding no other unit's figure, not impassable.
    fn rect_is_clear(
        &self,
        unit: usize,
        target: (i16, i16),
        figures: usize,
        footprint: i32,
        cols: i32,
    ) -> bool {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        if dest.surface == 2 {
            return false;
        }
        let rect = formation::compute_rect(target, figures, footprint, cols);
        for i in 0..figures {
            let (x, y) = rect.slot(i);
            if !(1..=0x4E).contains(&x) || !(1..=0x4E).contains(&y) {
                return false;
            }
            let cell = self.field.at(x as usize, y as usize);
            if cell.elevation != dest.elevation {
                return false;
            }
            if let Some(o) = self.occupant[y as usize * DIM + x as usize] {
                if self.unit_of(o as usize) != unit {
                    return false;
                }
            }
            if cell.impassable() {
                return false;
            }
        }
        true
    }

    /// `Formation_NearestFreeFigure` (`0x00489A62`): the member nearest `(x, y)`
    /// by Manhattan distance that has not been given a slot this pass. Greedy
    /// nearest-first,
    fn nearest_free_figure(
        &self,
        members: &[usize],
        assigned: &[bool],
        x: i32,
        y: i32,
    ) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (k, &m) in members.iter().enumerate() {
            if assigned[k] {
                continue;
            }
            let f = &self.fighters[m];
            let d = (f.x as i32 - x).abs() + (f.y as i32 - y).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k));
            }
        }
        best.map(|(_, k)| k)
    }

    /// `Formation_AssignSearchedSlots` (`0x00489798`): the destination's
    /// rectangle is blocked, so search outward from it once per figure and
    /// claim cells as they are taken.
    ///
    /// The claim list is the original's — it exists so two figures never get
    /// the same cell. The acceptance test is `Formation_SlotIsUsable`
    /// (`0x0048A672`), whose field-battle half is reproduced: a cell holding an
    /// enemy figure is usable, a cell holding another friendly unit's figure is
    /// not, and an impassable empty cell is not.
    fn assign_searched_slots(
        &mut self,
        unit: usize,
        members: &[usize],
        target: (i16, i16),
        dest_is_water: bool,
    ) {
        let mut assigned = vec![false; members.len()];
        let mut claimed: Vec<(i32, i32)> = Vec::with_capacity(members.len());
        for _ in 0..members.len() {
            let Some((x, y)) = self.find_nearby_slot(unit, target, &claimed) else {
                break;
            };
            claimed.push((x, y));
            let Some(k) = self.nearest_free_figure(members, &assigned, x, y) else {
                break;
            };
            assigned[k] = true;
            self.send_figure(unit, members[k], x, y, dest_is_water);
        }
    }

    /// `Formation_FindNearbySlot` (`0x0048A38E`): two short-circuits, then the
    /// expanding-ring search.
    pub fn find_nearby_slot(
        &self,
        unit: usize,
        target: (i16, i16),
        claimed: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        // `if (surface == 2) { origin = target; return 1; }` — `00480000.c:4077`,
        // the **first** short-circuit, ahead of the catapult arm at `:4082`.
        // Every figure of a unit ordered onto the moat is sent to the moat cell
        // itself, and `Formation_AssignSearchedSlots` (`0x00489798`, `:3758`)
        // never reaches the claim list on this path, so they share the one cell.
        // Corrects C81's "a dry slot beside it" — `docs/decisions.md`
        // correction C236.
        if dest.surface == crate::siege::SURFACE_WATER {
            return Some((target.0 as i32, target.1 as i32));
        }
        // `if (surface == 4 && g_battleUnits[unit].category == 5) { origin =
        // target; return 1; }` — `:4082`, the second, so the rampart-walk
        // branch below never refuses a catapult.
        if dest.surface == crate::siege::SURFACE_RAMPART_WALK && self.units.get(unit).category == 5 {
            return Some((target.0 as i32, target.1 as i32));
        }
        for radius in 0..20i32 {
            let x0 = (target.0 as i32 - radius).max(0);
            let y0 = (target.1 as i32 - radius).max(0);
            let x1 = (target.0 as i32 + radius + 1).min(DIM as i32);
            let y1 = (target.1 as i32 + radius + 1).min(DIM as i32);
            for y in y0..y1 {
                for x in x0..x1 {
                    // `0 < local_40 && local_40 < 0x4f && 0 < local_10 &&
                    // local_10 < 0x4f` — `:4112-4113`, ahead of the
                    // `Formation_SlotIsUsable` call in the same `if`: the search
                    // never answers with a cell on the map's border ring.
                    if !(1..0x4f).contains(&x) || !(1..0x4f).contains(&y) {
                        continue;
                    }
                    if claimed.contains(&(x, y)) {
                        continue;
                    }
                    if self.slot_is_usable(unit, x, y, dest.surface, dest.elevation) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    /// `Formation_SlotIsUsable` (`0x0048A672`), all four branches. `surface`
    /// and `elevation` are the caller's reference cell — the unit's
    /// destination in [`Self::find_nearby_slot`], the unit's own cell in
    /// [`Self::dest_find_reachable_near`]. `[V]` — 359 bytes, four parameters.
    pub fn slot_is_usable(
        &self,
        unit: usize,
        x: i32,
        y: i32,
        surface: u8,
        elevation: u8,
    ) -> bool {
        let cell = self.field.at(x as usize, y as usize);
        let occupant = self.occupant[y as usize * DIM + x as usize];
        // First branch: `uVar2 == 0 && side == 0 && g_battleIsSiege != 0 &&
        // surface[cell] < 4`.
        if occupant.is_none()
            && self.units.get(unit).side == crate::SIDE_A
            && self.siege.is_siege
            && cell.surface < crate::siege::SURFACE_RAMPART_WALK
        {
            return false;
        }
        if cell.impassable() && occupant.is_some() {
            return true;
        }
        if let Some(o) = occupant {
            let other = self.unit_of(o as usize);
            if self.units.owner_of(other) != self.units.owner_of(unit) {
                return true;
            }
            if other != unit {
                return false;
            }
        }
        // `if (surface[cell] == surface || surface != 4) { … } else 0`.
        if cell.surface != surface && surface == crate::siege::SURFACE_RAMPART_WALK {
            return false;
        }
        if (cell.elevation as i32) < elevation as i32 - 1 {
            return false;
        }
        !cell.impassable()
    }

}
