#![allow(unused_imports)]
use super::*;
use super::unit_impl::*;
use super::wages::*;
use super::starvation::*;
use super::combine_part::*;
use super::destroy_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

impl Units {
    pub fn new() -> Units {
        Units { slots: core::array::from_fn(|_| None) }
    }

    pub fn get(&self, id: usize) -> Option<&Unit> {
        self.slots.get(id).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut Unit> {
        self.slots.get_mut(id).and_then(Option::as_mut)
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Unit)> {
        self.slots.iter().enumerate().filter_map(|(i, u)| u.as_ref().map(|u| (i, u)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut Unit)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, u)| u.as_mut().map(|u| (i, u)))
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|u| u.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The lowest free slot, 1…150. `Unit_Spawn` (`0x0046E1B0`) takes the
    /// first free one, so which slot a unit lands in is deterministic and
    /// reproducible — and the AI's "first army in this county" is really "the
    /// lowest-numbered one".
    pub fn free_slot(&self) -> Option<usize> {
        (1..MAX_UNITS).find(|&i| self.slots[i].is_none())
    }

    pub fn spawn(&mut self, unit: Unit) -> Option<usize> {
        let slot = self.free_slot()?;
        self.slots[slot] = Some(unit);
        Some(slot)
    }

    pub fn remove(&mut self, id: usize) -> Option<Unit> {
        self.slots.get_mut(id).and_then(Option::take)
    }

    pub fn put(&mut self, id: usize, unit: Unit) {
        if let Some(slot) = self.slots.get_mut(id) {
            *slot = Some(unit);
        }
    }

    /// **`Unit_LinkToTile` (`0x0046EDDF`) opens `if (kind != 1 || garrisonCounty
    /// == 0)` and does nothing for anything else**, and `Army_GarrisonApply`
    /// calls `Unit_UnlinkFromTile` on its way in. So an army inside a castle is
    /// deliberately kept **out** of the tile's occupancy chain: two functions
    /// agree
    /// instead of the unit.
    ///
    /// `[V]`
    pub fn at(&self, x: u8, y: u8) -> Option<usize> {
        self.iter()
            .find(|(_, u)| u.x == x && u.y == y && !u.is_garrisoned())
            .map(|(i, _)| i)
    }

    /// `Units_ResetMoves` (`0x004651B9`) — turn phase 7, for **all 150 slots
    /// regardless of type or owner**: `moveState = 0` and `movesUsed = 0`.
    ///
    /// `[D]`
    /// besiegers or free slots, and it does not touch the allowance, which each
    /// type's tick handler rewrites anyway.
    pub fn reset_moves(&mut self) {
        for (_, u) in self.iter_mut() {
            u.moving = false;
            u.moves_used = 0;
        }
    }

    /// `Army_RecountCountyTroops` (`0x004AD6C0`) — rebuild every county's
    /// `friendly_troops` / `enemy_troops`, which is what
    /// [`crate::ration::people_to_feed`] adds to the food requirement when
    /// *Army foraging* is on.
    ///
    /// The original clears and fills all sixteen county slots regardless of how
    /// many the map has; so does this, because a unit standing on a tile whose
    /// county byte is above `county_count` would otherwise leave a stale count
    /// behind. `[D]`
    pub fn recount_county_troops(&self, counties: &mut [County; MAX_COUNTIES], realms: &[Realm; MAX_REALMS]) {
        for c in counties.iter_mut().skip(1) {
            c.friendly_troops = 0;
            c.enemy_troops = 0;
        }
        for (_, u) in self.iter() {
            if !u.kind.is_combatant() || u.is_garrisoned() {
                continue;
            }
            let Some(county) = counties.get_mut(u.county as usize) else { continue };
            let ally = realms.get(u.owner as usize).map_or(0, |r| r.ally);
            if county.owner == u.owner || (ally != 0 && ally == county.owner) {
                county.friendly_troops += u.men;
            } else {
                county.enemy_troops += u.men;
            }
        }
    }

    /// The men and the armies a realm has, which are two of `Score_RankRealms`'
    /// six inputs (realm `+0x54` and `+0x2C`).
    pub fn realm_totals(&self, realm: u8) -> (u8, i32) {
        let mut armies = 0u8;
        let mut men = 0i32;
        for (_, u) in self.iter() {
            if u.owner == realm && u.kind == UnitKind::Army {
                armies = armies.saturating_add(1);
                men += u.men;
            }
        }
        (armies, men)
    }
}

