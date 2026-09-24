#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;
use super::tests::*;
use crate::conquest::{self, Attack};
use crate::kingdom::Kingdom;
use crate::merchant;
use crate::movement::{self, Entry, Offence};
use crate::phase::Phase;
use crate::unit::{UnitKind, Units};

impl Kingdom {
    /// `Army_GarrisonApply` (`0x004A79A3`) — put an army inside its own
    /// county's castle.
    ///
    /// * **It is a teleport, not a step.** The army does not walk onto the
    ///   castle tile; it is placed on it and charged five moves. The tile it
    ///   was walking to is `County_FindCastleTile`'s `+0x74`/`+0x75`, which
    ///   this crate does not store — [`crate::ai_army::aim_tile`] recomputes it
    ///   off the map, which is what wrote that pair in the first place. `[I]`
    ///   on the substitution and `[V]` on there being one tile to find:
    pub fn garrison_army(&mut self, unit: usize, county: u8) -> Garrison {
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = true;
        }
        let Some(c) = self.counties.get(county as usize) else { return Garrison::TooMany };
        let (castle_type, sitting_slot) = (c.castle_type, c.garrison_unit);
        let sitting = self.campaign.units.get(sitting_slot).map_or(0, |u| u.men);
        let arriving = self.campaign.units.get(unit).map_or(0, |u| u.men);
        if arriving + sitting > crate::industry::garrison_cap(&self.tables, castle_type) {
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = crate::ai_army::Mission::SEEK_ENEMY;
                u.moving = false;
            }
            return Garrison::TooMany;
        }
        if sitting_slot == unit {
            return Garrison::Joined { into: sitting_slot };
        }
        if sitting_slot != 0 {
            let _ = crate::unit::combine(&mut self.campaign.units, sitting_slot, unit);
            let realms = self.realms.clone();
            self.campaign.units.recount_county_troops(&mut self.counties, &realms);
            return Garrison::Joined { into: sitting_slot };
        }
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let (x, y) = crate::ai_army::aim_tile(
            &self.campaign.map,
            &self.counties,
            from,
            county,
            crate::ai_army::Aim::Castle,
        );
        self.counties[county as usize].garrison_unit = unit;
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = crate::ai_army::Mission::GARRISON;
            u.garrison_county = county;
            u.x = x;
            u.y = y;
            u.county = county;
            u.moving = false;
            u.path.clear();
            u.player_driven = true;
            u.moves_used += GARRISON_MOVE_COST;
            u.dest_county = county;
            u.dest = Some((x, y));
        }
        let realms = self.realms.clone();
        self.campaign.units.recount_county_troops(&mut self.counties, &realms);
        Garrison::Took
    }

    /// `Units_Tick` (`0x004650B0`) — one step for every unit that is walking.
    ///
    /// The sweep **stops at the first battle**, which is the original's
    /// `DAT_0050A49E` latch: `Units_Tick` returns the moment
    /// `Unit_EnterOccupiedTile` has opened a battle, leaving the higher slots
    /// unmoved until the battle is over. That is observable — an army in slot 9
    /// does not move on the tick an army in slot 4 picks a fight — so it is
/// reproduced.
    pub fn tick_units(&mut self) -> UnitsTick {
        let mut out = UnitsTick::default();
        refresh_allowances(&mut self.campaign.units);
        for id in 1..crate::unit::MAX_UNITS {
            let Some(u) = self.campaign.units.get_mut(id) else { continue };
            if !(u.moving && !u.ordered) {
                // Army_Tick 00460000.c:4096-4098 `else if (playerDriven == 0) moving = 2;` [V].
                if u.kind == UnitKind::Army && !u.player_driven {
                    (u.moving, u.ordered) = (true, false);
                }
                continue;
            }
            if !cross_sub_tile(&mut self.campaign.units, id) {
                continue;
            }
            // **`Unit_Step` (`0x00465D28`)'s first statement inside its loop**,
            // and the fog of war's commonest writer:
            //
            // ```c
            // if (unit.field_0x14b & 1) {                       /* on a tile centre */
            //     if (unit.kind == 1 && unit.owner == g_localPlayer) {
            //         FUN_0046E067(unit.x, unit.y, 6);  Gfx_MarkAllDirty();
            //     }
            //     ...the stop tests, then Unit_StepOnce...
            // ```
            if let Some(u) = self.campaign.units.get(id) {
                if u.kind == crate::UnitKind::Army {
                    let (owner, x, y) = (u.owner, u.x as i32, u.y as i32);
                    self.campaign.explored.reveal_square(owner, x, y, crate::explore::ARMY_SIGHT);
                }
            }
            self.step_one(id, &mut out);
            if out.battle().is_some() {
                break;
            }
        }
        out
    }

    fn step_one(&mut self, id: usize, out: &mut UnitsTick) {
        let arm = self.enter_occupied(id, out);
        // Unit_Step 00460000.c:4386 `if (iVar2 == 10) break;`: no step, `moving` kept [V].
        if arm == Arm::Captured {
            return;
        }
        let Some(step) = movement::step_over(
            &mut self.campaign.map,
            &mut self.counties,
            &self.realms,
            &mut self.campaign.units,
            id,
            arm == Arm::Over,
        ) else {
            let (mut stacked, mut at_end) = (false, false);
            if let Some(u) = self.campaign.units.get_mut(id) {
                // :4337 tests the allowance before :4350's pathLen; that branch returns.
                at_end = u.moves_used < u.move_allowance;
                u.moving = false;
                // Unit_Step 00460000.c:4340 `movesUsed = moveAllowance;`, so FUN_00467196's refund is spendable [V].
                if u.moves_used >= u.move_allowance {
                    u.moves_used = u.move_allowance;
                }
                if u.path.is_empty() {
                    u.needs_destination = true;
                }
                (stacked, at_end) = (!u.player_driven, at_end && u.path.is_empty());
            }
            // Unit_Step 00460000.c:4356/:4368 `if (playerDriven == 0) FUN_00467196(unit);` [V].
            if stacked {
                let c = &mut self.campaign;
                let _ = movement::step_aside(&c.move_cost, &c.map, &mut c.units, id);
                // :4368 falls through to path[0]; movement::zeroed_head.
                if at_end && movement::zeroed_head(&mut c.units, id) {
                    return self.step_one(id, out);
                }
                if let Some(u) = c.units.get_mut(id).filter(|u| at_end && u.path.is_empty()) {
                    u.moving = false;
                }
            }
            return;
        };

        if step.moved {
            out.stepped += 1;
        }
        if let Some(o) = step.offence {
            // `Unit_CrossField` (`0x0046673C`) and `Unit_BurnDwelling`
            // (`0x00468AE2`) both call `Diplo_Offend` **inline**, in the middle
            // of the step, and this is the same place. `Step::offence` stays as
            // a report because the caller may want to say something about it;
            // what it no longer is, is the only thing that happens.
            crate::diplomacy::offend(
                &mut self.realms,
                o.against,
                o.by,
                o.amount.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
            );
            out.offences.push(o);
        }
        if step.entered_county.is_some()
            && self.campaign.units.get(id).is_some_and(|u| u.kind.is_combatant())
        {
            self.campaign.units.recount_county_troops(&mut self.counties, &self.realms);
        }
        if let (Some(county), Some(u)) = (step.entered_county, self.campaign.units.get(id)) {
            if u.kind == crate::UnitKind::Army {
                if self.counties.get(county as usize).is_some_and(|c| c.owner != u.owner) {
                    out.incursions.push(Incursion { unit: id, owner: u.owner, county });
                }
                if let Some(letter) =
                    crate::arrival::enter_county(&self.counties, &mut self.realms, u, county)
                {
                    out.posted.push(Posted::Letter(letter));
                }
            }
        }

        // `PeasantMob_Tick`'s own crossing call, `FUN_004ABD0F` (`0x004ABD0F`)
        // — the mob carries its revolution over the border. The original calls
        // it every tick with the tile's county byte and does the `!=` test
        // inside; `movement::step` has already made that test and written the
        // byte, so `entered_county` is that branch. [`crate::mob`].
        if let (Some(county), Some(u)) = (step.entered_county, self.campaign.units.get(id)) {
            if u.kind == UnitKind::PeasantMob {
                self.mob_crossed_border(county, out);
            }
        }

        // **Two branches implemented this rule
        // `ai-lords-play` added a garrison-only handler here; the castles
        // branch added `conquest::reach_castle_building` below, which covers
        // the siege half too and is traced statement by statement to
        // `Army_GarrisonApply` (`0x004A79A3`) — including the teleport onto
        // the cached castle tile, which the other placed by search. The one
        // thing only the AI branch had, the mission reset that stops a lord
        // marching at a full castle for ever, now lives in that function.
        //
        // `Transport_Deliver` (`0x004296B5`) — the mover's code-5 branch calls
        // it *before* `Army_AttackCounty`, and its tail is `FUN_0046F0A9`, the
        // slot free `Army_Destroy` also uses: a transport that reaches its
        // cargo county's town unloads and is gone. `[V]` from the
        // decompilation. Non-matching county: nothing at all
        // stands. `attack_county` would refuse either way (`NotAnArmy`).
        if let Some(county) = step.reached_castle {
            if let Some(u) = self.campaign.units.get(id) {
                if u.kind == UnitKind::Transport {
                    if u.cargo_county == county {
                        let u = u.clone();
                        crate::supply::deliver(&mut self.counties, &u);
                        crate::field::herd_update_crowding(
                            &self.tables,
                            &mut self.counties[county as usize],
                            &mut self.campaign.map,
                        );
                        self.campaign.units.remove(id);
                    }
                    return;
                }
            }
        }

        if let Some(county) = step.reached_castle {
            let restore = self.restore();
            let outcome = conquest::attack_county(
                &self.tables,
                &self.campaign.map,
                &mut self.counties,
                &mut self.realms,
                &mut self.campaign.units,
                &mut self.campaign.names,
                id,
                county,
                self.options.difficulty,
                self.year,
                &mut self.campaign.explored,
                restore,
            );
            // FUN_004A6C68 0x004A6C68 004a0000.c:2865: set once the guards pass, battle or none.
            if !matches!(outcome, Attack::Refused(_)) {
                self.campaign.battle_county = county;
            }
            if let Attack::Captured(capture) = outcome {
                out.posted.push(Posted::Capture(capture));
            }
            out.contacts.push(Contact::Castle { unit: id, county, outcome });
            return;
        }

        if let Some(county) = step.reached_castle_building {
            let arrival = conquest::reach_castle_building(
                &self.tables,
                &self.campaign.map,
                &mut self.counties,
                &self.realms,
                &mut self.campaign.units,
                id,
                county,
                self.season,
            );
            out.contacts.push(Contact::CastleBuilding { unit: id, county, arrival });
            return;
        }

        if let Entry::Occupied(occupant) = step.entry {
            let county = self.campaign.units.get(occupant).map_or(0, |o| o.county);
            out.contacts.push(match arm {
                Arm::Battle(_) => Contact::Battle(Encounter { mover: id, occupant, county }),
                _ => Contact::Blocked { mover: id, occupant },
            });
        }
    }

    pub fn begin_unit_phase(&mut self, phase: Phase) -> usize {
        match phase {
            // `Siege_StartPhase` (`0x004A82B9`). Sieges are out of scope, and
            // the important half for this module is the negative: the function
            // touches only `besiegedBy`, `besiegingCounty` and `garrisonUnit`,
            // and never `moving`, `path`, `dest` or `movesUsed`. **Phase 2
            // originates no movement**, so there is nothing here to leave out.
            Phase::ArmyMovement => 0,
            Phase::SupplyTransports => self.retarget_transports(),
            Phase::PeasantMobs => self.retarget_mobs(),
            Phase::Merchants => merchant::advance_all(
                &self.campaign.routes,
                &self.campaign.move_cost,
                &self.campaign.map,
                &self.counties,
                &mut self.campaign.units,
                self.county_count,
            ),
            _ => 0,
        }
    }

    /// Whether any unit of a kind is still walking — the predicate
    /// `FUN_004A4F5B(kind)` answers for phases 3 and 6, and
    /// `FUN_004A4E3D(2, 6)` for phase 5.
    pub fn units_moving(&mut self, kind: UnitKind) -> bool {
        let mut any = false;
        for (_, u) in self.campaign.units.iter_mut() {
            if u.moving
                && u.kind == kind
                && (kind != UnitKind::PeasantMob || (u.owner == OWNERLESS && !u.owner_is_human))
            {
                // Units_AnyMovingOfKind 004a0000.c:2004 / ForRealm :1976 `moving = 2` [V].
                u.ordered = false;
                any = true;
            }
        }
        any
    }

    /// `FUN_004A4E3D(kind, realm)` — is that realm's own unit of this kind
    /// still walking?
    pub fn realm_units_moving(&mut self, kind: UnitKind, realm: u8) -> bool {
        let mut any = false;
        for id in 1..crate::unit::MAX_UNITS {
            let Some(u) = self.campaign.units.get_mut(id) else { continue };
            if u.moving && u.kind == kind && u.owner == realm && !u.owner_is_human {
                // 004a0000.c:1976-1979 `moving = 2; if (kind == 1) Siege_Break(unit);` [V].
                u.ordered = false;
                if kind == UnitKind::Army {
                    crate::siege::break_siege(&self.counties, &mut self.campaign.units, id);
                }
                any = true;
            }
        }
        any
    }

    /// `FUN_00429418` — **the first step of phase 3.**
    fn retarget_transports(&mut self) -> usize {
        let mut started = 0;
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::Transport)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            let Some(cargo) = self.campaign.units.get(id).map(|u| u.cargo_county) else { continue };
            let Some(county) = self.counties.get(cargo as usize) else { continue };
            let anchor = (county.anchor_x, county.anchor_y);
            if let Some(u) = self.campaign.units.get_mut(id) {
                u.dest = Some(anchor);
                u.dest_county = cargo;
                u.needs_destination = false;
            }
            if movement::order_move_by_road(&self.campaign.move_cost, &self.campaign.map, &mut self.campaign.units, id, anchor)
                .is_some()
            {
                started += 1;
            }
        }
        started
    }

    /// `FUN_004AC499` and its picker `FUN_004AC5BA` — **the first step of
    /// phase 5.**
    fn retarget_mobs(&mut self) -> usize {
        let mut started = 0;
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::PeasantMob)
            .map(|(id, _)| id)
            .collect();
        if self.county_count == 0 {
            return 0;
        }
        for id in ids {
            let Some(u) = self.campaign.units.get(id) else { continue };
            let (here, mut dest_county) = (u.county, u.dest_county);
            if self.season == MOB_RETARGET_SEASON || dest_county == 0 {
                self.mob_cursor_next();
                if here as usize == self.campaign.mob_cursor {
                    self.mob_cursor_next();
                }
                dest_county = self.campaign.mob_cursor as u8;
            }
            if dest_county == 0 {
                continue;
            }
            let Some(county) = self.counties.get(self.campaign.mob_cursor) else { continue };
            let anchor = (county.anchor_x, county.anchor_y);
            if let Some(u) = self.campaign.units.get_mut(id) {
                u.dest_county = dest_county;
                u.dest = Some(anchor);
                u.needs_destination = false;
            }
            if movement::repath(
                &self.campaign.move_cost,
                &self.campaign.map,
                &mut self.campaign.units,
                id,
                anchor,
                movement::Routing::Direct,
            )
            .is_some()
            {
                // PeasantMob_AdvanceAll 004a0000.c:5094 `moving = 1` [V].
                if let Some(u) = self.campaign.units.get_mut(id) {
                    u.ordered = true;
                }
                started += 1;
            }
        }
        started
    }

    fn mob_cursor_next(&mut self) {
        self.campaign.mob_cursor += 1;
        if self.campaign.mob_cursor > self.county_count {
            self.campaign.mob_cursor = 1;
        }
    }
}



