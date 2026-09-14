#![allow(unused_imports)]
use super::*;
use super::operations::*;
use super::targeting::*;
use super::*;
use super::wants::*;
use super::muster::*;
use super::raid::*;
use super::aim::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};

impl Kingdom {
    /// AI step 11 — `FUN_004A5667`, which this crate names `Ai_AdvanceArmies`.
    ///
    /// ```c
    /// for (i = 1; i <= 150; i++)
    ///   if (units[i].kind == 1 && units[i].owner == realm && mission_handler(i) != 0) {
    ///       units[i].castlePassThrough = 0;          /* +0x14E */
    ///       Move_FloodFill(0, units[i].x, units[i].y, 0);
    ///       if (Move_ExtractPath(0, units[i].destX, units[i].destY))
    ///           { Path_CopyToUnit(0, i); units[i].moving = 1; }
    ///   }
    /// ```
    ///
    /// It is the same shape as `PeasantMob_AdvanceAll`, which is already
    /// implemented as [`Kingdom::begin_unit_phase`]'s phase-5 arm — the AI's
    /// armies are the mobs' mechanism with a six-way mission dispatch in front
    /// of it.
    ///
    /// **The routing is `Direct`, not `PreferRoads`.** `Unit_OrderMove` — the
    /// player's path — tries roads first and falls back; this call passes
    /// mode 0 outright. So the claim in [`crate::movement::Routing`] that *"AI
    /// armies prefer roads and the player's do not"* is true of the
    /// *order-move* entry point and **false of the AI's own driver**, which is
/// the one that moves AI armies every turn. Both readings are of
    /// the binary; they are about two different functions.
    ///
    /// Unit `+0x14E`, cleared before each re-path, is a *pass through the
    /// castle site* flag: `Unit_TryEnterTile` masks off the settlement bit
    /// while it is set, which is what stops a garrison just turned out of a
    /// castle immediately walking back into it. This crate has no such field
    /// and no path that sets one, so there is nothing to clear. Named rather
    /// than omitted. `[D]`
    pub fn run_ai_move_armies(&mut self, realm_id: u8) -> MoveReport {
        let mut report = MoveReport::default();
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::Army && u.owner == realm_id)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if !self.run_mission(id, &mut report) {
                continue;
            }
            let Some(dest) = self.campaign.units.get(id).and_then(|u| u.dest) else { continue };
            if crate::movement::order_move(
                &self.campaign.map,
                &mut self.campaign.units,
                id,
                dest,
                crate::movement::Routing::Direct,
            )
            .is_some()
            {
                report.marching.push(id);
            }
        }
        report
    }

    /// `FUN_004A57AC` — dispatch one army's mission.
    ///
    /// Returns whether the unit wants a fresh path. **An unrecognised mission
    /// byte is rewritten to [`Mission::SEEK_ENEMY`] and the unit is not moved
    /// this turn** — so a freshly spawned army, whose byte is 0, loses exactly
    /// one turn and then behaves as an attacker.
    fn run_mission(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(mission) = self.campaign.units.get(unit).map(|u| u.mission) else { return false };
        match mission {
            Mission::RAID => self.mission_raid(unit),
            Mission::ASSIST_ALLY => self.mission_assist_ally(unit),
            Mission::SEEK_ENEMY => self.mission_seek_enemy(unit),
            Mission::HOLD_HOME => self.mission_hold_home(unit),
            Mission::JOIN_GARRISON => self.mission_join_garrison(unit, report),
            Mission::GARRISON => self.mission_garrison(unit, report),
            _ => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.mission = Mission::SEEK_ENEMY;
                }
                false
            }
        }
    }

    /// [`Mission::RAID`] — `FUN_004A58F4`. Re-aim at the target county's
    /// nearest standing crop, every turn, unless the unit is camped outside a
    /// castle.
    ///
    /// Re-aiming *every* turn is what makes a raid destructive out of
    /// proportion to its size: the party is fifty men who cannot fight, but it
    /// walks a fresh line to the nearest surviving crop each season and
    /// tramples everything it crosses.
    fn mission_raid(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (from, target) = (u.tile(), u.dest_county);
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, Aim::StandingCrop);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest = Some(tile);
            u.needs_destination = false;
        }
        true
    }

    /// [`Mission::ASSIST_ALLY`] — `FUN_004A599D`.
    ///
    /// Re-validate the ally's request; a request that no longer stands demotes
    /// the unit to [`Mission::SEEK_ENEMY`] and **runs that handler in the same
    /// tick**, so no turn is lost. Otherwise close on the nearest enemy in the
    /// ally's county, but only within [`ASSIST_ALLY_RADIUS`] — an army too far
/// away does nothing at all this turn, which
    /// is a real difference from [`Mission::HOLD_HOME`].
    fn mission_assist_ally(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, mut county) = (u.owner, u.mission_county);
        if county != 0 {
            county = ally_request_still_stands(&self.counties, &self.realms, county, owner);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission_county = county;
            }
        }
        if county == 0 {
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return self.mission_seek_enemy(unit);
        }
        let (d, other) = nearest_attackable_army_in_county(
            &self.campaign.units,
            &mut self.realms,
            unit,
            county,
        );
        let Some(other) = other.filter(|_| d < ASSIST_ALLY_RADIUS) else { return false };
        let tile = self.campaign.units.get(other).map(|u| u.tile());
        if let (Some(tile), Some(u)) = (tile, self.campaign.units.get_mut(unit)) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// [`Mission::SEEK_ENEMY`] — `FUN_004A5B1F`, the default war mission and
    /// the largest of the six.
    ///
    /// Four things in order:
    ///
    /// 1. **Intercept.** An enemy army within [`SEEK_ENEMY_RADIUS`] that is
    ///    not this realm's ally, and that is either *not itself on this
    ///    mission* or is to the east. The `x` test is a tie-break that stops
    ///    two attacking armies chasing each other for ever: whichever is
    ///    further west gives chase. `[I]` on the intent, `[D]` on the code.
    /// 2. **Keep the standing order** if the county it names is still somebody
    ///    else's and still borders this realm.
    /// 3. Otherwise, if the army is standing somewhere it cannot attack —
    ///    its own county, or one that does not touch its realm — **pick a new
    ///    target** and go.
    /// 4. Otherwise it is standing in a hostile county that borders its realm:
    ///    **attack where it stands**, unless that county belongs to its ally,
    ///    in which case pick another.
    ///
    /// **Two defects are reproduced.** In arm 4 the unit's
    /// [`crate::unit::Unit::dest_county`] is *not* updated, so it walks into the county it
    /// is in while its stored order still names the county arm 2 rejected; and
    /// if the ally case finds nothing, the aim is taken at **county 0**, which
    /// falls through to county 0's anchor. Both are in the decompilation and
    /// neither is tidied here.
    ///
    /// A third is *not* reproduced because it cannot be: the original calls
    /// `FUN_00467F2E` between arms 3 and 4 and **discards the result** — the
    /// two branches it was meant to choose between are identical. A dead call
    /// with no side effect is nothing to reproduce.
    fn mission_seek_enemy(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, order, here) = (u.owner, u.x, u.dest_county, u.county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);

        // 1 — intercept.
        let (d, other) = nearest_enemy_army(&self.campaign.units, unit);
        if d < SEEK_ENEMY_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }

        // 2 — the standing order still stands.
        if order != 0
            && self.counties.get(order as usize).is_some_and(|c| c.owner != owner)
            && county_borders_realm(&self.counties, order, owner)
        {
            self.aim_at_county(unit, order);
            return true;
        }

        // 3 — nowhere to attack from here: choose again.
        let here_owner = self.counties.get(here as usize).map_or(0, |c| c.owner);
        if here_owner == owner || !county_borders_realm(&self.counties, here, owner) {
            let next = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if next == 0 {
                return false;
            }
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = next;
            }
            self.aim_at_county(unit, next);
            return true;
        }

        // 4 — attack where it stands. `dest_county` is deliberately left alone.
        let mut target = here;
        if ally == here_owner {
            target = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = target;
            }
        }
        self.aim_at_county(unit, target);
        true
    }

    /// [`Mission::HOLD_HOME`] — `FUN_004A5F0A`.
    ///
    /// The same intercept test as [`Mission::SEEK_ENEMY`] but restricted to
    /// [`crate::unit::Unit::home_county`] and with five times the radius
    /// ([`HOLD_HOME_RADIUS`]) — and, unlike the ally mission, **it always
    /// returns something to do**: nothing to intercept means walk back to the
    /// county it is posted to.
    ///
    /// "Home county" is not always where the army was raised: step 7's
    /// abandonment branch overwrites it with the realm's muster county, which
    /// is what turns a written-off county's levy into a retreat.
    fn mission_hold_home(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, home) = (u.owner, u.x, u.home_county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);
        let (d, other) =
            nearest_attackable_army_in_county(&self.campaign.units, &mut self.realms, unit, home);
        if d < HOLD_HOME_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }
        self.aim_at_county(unit, home);
        true
    }

    /// [`Mission::JOIN_GARRISON`] — `FUN_004A6270`.
    ///
    /// Re-check every turn that the castle this army was sent to is still
    /// worth walking to: it must still have room for **all** of the army, be
    /// standing, not be under construction, and still belong to the realm. Any
    /// of those failing sends it to
    /// [`first_castle_with_room`] instead.
    /// army is **disbanded** — its men go back into a county and its weapons
    /// back into the armoury.
    ///
    /// **The original returns 1 even after disbanding**, so its caller then
    /// flood-fills from a record `Army_Destroy` has already cleared. Here the
/// slot is empty and the re-path finds nothing to do.
    /// same observable outcome by a route that cannot read freed memory.
    fn mission_join_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, men, target) = (u.owner, u.men, u.dest_county);
        let ok = self.counties.get(target as usize).is_some_and(|c| {
            let sitting = self.campaign.units.get(c.garrison_unit).map_or(0, |g| g.men);
            c.castle_type != 0
                && !c.castle_ruined
                && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
                && c.owner == owner
                && men <= crate::industry::garrison_cap(&self.tables, c.castle_type) - sitting
        });
        let county = if ok {
            target
        } else {
            first_castle_with_room(
                &self.tables,
                &self.counties,
                &self.campaign.units,
                self.county_count,
                owner,
                men,
            )
        };
        if county == 0 {
            let Kingdom { tables, counties, realms, campaign, options, .. } = self;
            let _ = crate::divide::disband(
                tables,
                counties,
                realms,
                &mut campaign.units,
                &mut campaign.names,
                &mut campaign.mercenaries,
                unit,
                options.difficulty,
            );
            report.disbanded.push(unit);
            return true;
        }
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Castle);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest_county = county;
            u.needs_destination = false;
            u.dest = Some(tile);
        }
        true
    }

    /// [`Mission::GARRISON`] — `FUN_004A60B9`, *"should I still be in here?"*
    ///
    /// A garrison whose county still belongs to its realm does **nothing at
    /// all** — the common case.
    /// nearly always. Once the county is lost, the garrison is turned out
    /// ([`Kingdom::evict_garrison`]) and demoted to [`Mission::SEEK_ENEMY`],
    /// and in the one case where the castle is still standing and unruined it
    /// is also sent straight at the **county town** — i.e. it walks out of the
    /// castle to take the county back.
    fn mission_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, county) = (u.owner, u.dest_county);
        let Some(c) = self.counties.get(county as usize) else { return false };
        if c.owner == owner {
            return false;
        }
        let retake = c.castle_type != 0
            && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
            && !c.castle_ruined;
        if !retake {
            self.evict_garrison(unit, county, report);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return false;
        }
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = Mission::SEEK_ENEMY;
        }
        self.evict_garrison(unit, county, report);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Town);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// `FUN_00437535` — turn a garrison out of its castle onto the nearest
    /// free tile, or destroy it if there is nowhere to stand.
    ///
    /// The link is cleared on **both** sides — the unit's
    /// [`crate::unit::Unit::garrison_county`] and the county's
    /// [`crate::county::County::garrison_unit`] — and a besieger waiting outside is reported
    ///; see [`Eviction`].
    ///
    /// **`[I]` on the search.** The original calls `Map_FindFreeTileNear`,
    /// whose radius this crate has not read; the box walk here is the one
    /// [`crate::levy::muster_tile`] and [`crate::merchant::find_free_road_tile`]
    /// both use, growing 1, 2, 3 around the unit's own tile and taking any
    /// passable unoccupied ground. The choice of tile is not observable in any
    /// rule; whether one is *found* is, and at radius 3 around a castle it
    /// always is on a real map.
    pub fn evict_garrison(&mut self, unit: usize, county: u8, report: &mut MoveReport) {
        let besieger = self.campaign.units.get(unit).and_then(|u| {
            let b = u.besieged_by as usize;
            if b == 0 {
                None
            } else {
                Some(b)
            }
        });
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let spot = free_tile_near(&self.campaign.map, &self.campaign.units, from);
        if let Some(c) = self.counties.get_mut(county as usize) {
            if c.garrison_unit == unit {
                c.garrison_unit = 0;
            }
        }
        match spot {
            Some((x, y)) => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.x = x;
                    u.y = y;
                    u.garrison_county = 0;
                    u.besieged_by = 0;
                }
                report.evictions.push(Eviction { unit, county, besieger, destroyed: false });
            }
            None => {
                self.campaign.units.remove(unit);
                report.evictions.push(Eviction { unit, county, besieger, destroyed: true });
            }
        }
    }

    /// `FUN_004A64CA` — aim an army at a county, at its castle when that
    /// castle is held and at its town otherwise. See [`aim_for_county`].
    fn aim_at_county(&mut self, unit: usize, county: u8) {
        let aim = aim_for_county(&self.counties, county);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, aim);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
        }
    }
}

/// Any free, passable tile within three of `from`. See
/// [`Kingdom::evict_garrison`].
fn free_tile_near(map: &CampaignMap, units: &Units, (ax, ay): (u8, u8)) -> Option<(u8, u8)> {
    for r in 1..=3i32 {
        let (x0, y0) = ((ax as i32 - r).max(0), (ay as i32 - r).max(0));
        let (x1, y1) = ((ax as i32 + r).min(MAP_DIM as i32 - 1), (ay as i32 + r).min(MAP_DIM as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (x, y) = (x as u8, y as u8);
                if units.at(x, y).is_none() && map.flags_at(x, y) & flags::IMPASSABLE == 0 {
                    return Some((x, y));
                }
            }
        }
    }
    None
}


