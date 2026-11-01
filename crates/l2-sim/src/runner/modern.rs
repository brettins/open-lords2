//! Modern battle controls, the simulation half. C272, `docs/battle-controls.md` §3, §4 Tier 2.
//! `[I]` ours: the original has one seat and one scheme. Each verb composes the original's own
//! order primitives and names them; what has no equivalent cites C272.

use super::*;
use crate::ai::World;
use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

/// Orders a unit holds behind the one it is carrying out. `[I]` ours, C272.
pub const QUEUE_MAX: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Move,
    AttackMove,
    Stop,
    Hold,
    Line,
    Column,
    Merge,
}

impl Verb {
    const ALL: [Verb; 7] =
        [Verb::Move, Verb::AttackMove, Verb::Stop, Verb::Hold, Verb::Line, Verb::Column, Verb::Merge];

    pub fn tag(self) -> u8 {
        Verb::ALL.iter().position(|&v| v == self).unwrap_or(0) as u8
    }

    pub fn from_tag(tag: u8) -> Option<Verb> {
        Verb::ALL.get(tag as usize).copied()
    }
}

/// An order waiting behind the current one, its destination already worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Queued {
    pub verb: Verb,
    pub at: (u8, u8),
    pub target: Option<usize>,
}

/// One unit's modern state. The default is the stance of a unit never commanded.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitMode {
    /// Move and Hold: the original's player behaviour, no engagement (Q271 `sc2`).
    pub passive: bool,
    /// Attack-move's destination, re-issued when nothing is left to fight.
    pub attack_move: Option<(u8, u8)>,
    /// The sweep has this unit in a fight and will release it when it is clear.
    pub engaged: bool,
    pub queue: Vec<Queued>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modern {
    /// Realms whose seat plays Modern: set by that seat's first command.
    realms: u16,
    modes: Vec<UnitMode>,
}

impl Default for Modern {
    fn default() -> Modern {
        Modern { realms: 0, modes: vec![UnitMode::default(); MAX_UNITS + 1] }
    }
}

fn bit(realm: u8) -> u16 {
    1 << (realm & 15)
}

impl Modern {
    fn is_idle(&self) -> bool {
        self.realms == 0
    }

    pub fn mode(&self, unit: usize) -> &UnitMode {
        &self.modes[unit]
    }

    /// This realm's seat has commanded in the Modern scheme.
    pub fn plays(&self, realm: u8) -> bool {
        self.realms & bit(realm) != 0
    }
}

impl Encode for Modern {
    fn encode(&self, out: &mut Canonical) {
        out.u16(self.realms);
        let used: Vec<usize> =
            (0..self.modes.len()).filter(|&i| self.modes[i] != UnitMode::default()).collect();
        out.seq(&used, |c, &i| {
            let m = &self.modes[i];
            c.u8(i as u8);
            c.bool(m.passive);
            c.option(m.attack_move.as_ref(), |c, p| {
                c.u8(p.0);
                c.u8(p.1);
            });
            c.bool(m.engaged);
            c.seq(&m.queue, |c, q| {
                c.u8(q.verb.tag());
                c.u8(q.at.0);
                c.u8(q.at.1);
                c.option(q.target.as_ref(), |c, t| c.len32(*t));
            });
        });
    }
}

impl Decode for Modern {
    fn decode(input: &mut Reader<'_>) -> Result<Modern, CodecError> {
        let mut m = Modern { realms: input.u16()?, ..Modern::default() };
        let used = input.seq(|r| {
            let at = r.position();
            let i = r.u8()? as usize;
            let passive = r.bool()?;
            let attack_move = r.option(|r| Ok((r.u8()?, r.u8()?)))?;
            let engaged = r.bool()?;
            let queue = r.seq(|r| {
                let at = r.position();
                let tag = r.u8()?;
                let verb = Verb::from_tag(tag).ok_or(CodecError::BadTag { tag, expected: "verb", at })?;
                Ok(Queued { verb, at: (r.u8()?, r.u8()?), target: r.option(|r| r.len32())? })
            })?;
            Ok((at, i, UnitMode { passive, attack_move, engaged, queue }))
        })?;
        for (at, i, mode) in used {
            let slot = m.modes.get_mut(i).ok_or(CodecError::BadTag { tag: i as u8, expected: "unit", at })?;
            *slot = mode;
        }
        Ok(m)
    }
}

impl BattleRunner {
    /// The unit's men answer a touch, a hit or a near enemy: `Enemy_NearestUnit`'s side of Q271.
    pub(crate) fn engages(&self, unit: usize) -> bool {
        let u = self.units.get(unit);
        u.human && self.modern.realms & bit(u.owner) != 0 && !self.modern.modes[unit].passive
    }

    /// One modern order, per-unit ownership refused as a whole (`turn::concerns`, per unit).
    /// `Some(unit)` is the unit a Merge made, else `Some(0)`. `docs/battle-controls.md` §3.
    pub fn command(
        &mut self,
        realm: u8,
        units: &[u8],
        verb: Verb,
        at: (u8, u8),
        target: Option<usize>,
        queue: bool,
    ) -> Option<usize> {
        let mut ids: Vec<usize> = units.iter().map(|&u| u as usize).collect();
        ids.sort_unstable();
        ids.dedup();
        let own = |u: &usize| (1..=MAX_UNITS).contains(u) && self.units.get(*u).owner == realm;
        if realm == 0 || !ids.iter().all(own) {
            return None;
        }
        let enemy = |t: &usize| {
            self.fighters.get(*t).is_some_and(|f| {
                let o = self.sim.figures[f.sim].owner;
                o != realm && o != 0
            })
        };
        if target.as_ref().is_some_and(|t| !enemy(t)) {
            return None;
        }
        self.modern.realms |= bit(realm);
        let at = (at.0.min(DIM as u8 - 1), at.1.min(DIM as u8 - 1));
        if verb == Verb::Merge {
            return Some(self.merge_units(realm, &ids));
        }
        let dests = self.group_dests(&ids, at);
        for (&unit, &dest) in ids.iter().zip(&dests) {
            let mode = &self.modern.modes[unit];
            let busy = !mode.queue.is_empty() || !self.arrived(unit);
            let moves = matches!(verb, Verb::Move | Verb::AttackMove);
            if queue && moves && busy {
                let q = &mut self.modern.modes[unit].queue;
                if q.len() < QUEUE_MAX {
                    q.push(Queued { verb, at: dest, target });
                }
                continue;
            }
            self.modern.modes[unit].queue.clear();
            self.run_verb(unit, verb, dest, target);
        }
        Some(0)
    }

    /// SC2's group move: `at` outside the units' bounding box sends each to `at` plus its offset from
    /// the box's centre; inside, they converge on `at`. `[I]` ours, C272.
    fn group_dests(&self, ids: &[usize], at: (u8, u8)) -> Vec<(u8, u8)> {
        let pos = |u: &usize| (self.units.get(*u).x as i32, self.units.get(*u).y as i32);
        let xs = ids.iter().map(|u| pos(u).0);
        let ys = ids.iter().map(|u| pos(u).1);
        let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
        let (y0, y1) = (ys.clone().min().unwrap_or(0), ys.max().unwrap_or(0));
        let inside = (x0..=x1).contains(&(at.0 as i32)) && (y0..=y1).contains(&(at.1 as i32));
        let centre = ((x0 + x1) / 2, (y0 + y1) / 2);
        let clamp = |v: i32| v.clamp(0, DIM as i32 - 1) as u8;
        ids.iter()
            .map(|u| {
                if inside {
                    return at;
                }
                let (x, y) = pos(u);
                (clamp(at.0 as i32 + x - centre.0), clamp(at.1 as i32 + y - centre.1))
            })
            .collect()
    }

    fn run_verb(&mut self, unit: usize, verb: Verb, dest: (u8, u8), target: Option<usize>) {
        self.units.get_mut(unit).halted = false;
        let mode = &mut self.modern.modes[unit];
        mode.engaged = false;
        mode.attack_move = None;
        match verb {
            Verb::Move | Verb::AttackMove => {
                mode.passive = verb == Verb::Move;
                mode.attack_move = (verb == Verb::AttackMove).then_some(dest);
                self.order_full(unit, dest.0, dest.1, target, false, Formation::Keep);
            }
            Verb::Stop | Verb::Hold => {
                mode.passive = verb == Verb::Hold;
                self.hold(unit);
            }
            Verb::Line => self.order_formation(unit, Formation::Line),
            Verb::Column => self.order_formation(unit, Formation::Column),
            Verb::Merge => {}
        }
        if matches!(verb, Verb::Line | Verb::Column) {
            return;
        }
        self.reform_unit(unit);
    }

    /// `Order_HoldPosition` (`0x0048B386`): the AI's own stop, through its `World`.
    fn hold(&mut self, unit: usize) {
        let mut w = World {
            units: &mut self.units,
            figures: &mut self.sim.figures,
            positions: &self.positions,
            field: &self.ai_field,
            ai: &mut self.ai,
        };
        w.hold_position(unit);
        for (u, x, y) in std::mem::take(&mut self.ai.orders) {
            self.pour_on_order(u, x, y);
        }
    }

    /// `FUN_00478987` over the chosen units' men. Its quirks come with it (`docs/battle-controls.md`
    /// §3); `figure.selected` is put back as found, selection staying presentation.
    fn merge_units(&mut self, realm: u8, ids: &[usize]) -> usize {
        let before: Vec<u8> = self.sim.figures.iter().map(|f| f.selected).collect();
        for f in self.sim.figures.iter_mut() {
            f.selected = if f.is_alive() && f.owner == realm && ids.contains(&(f.unit as usize)) { realm } else { 0 };
        }
        let merged = self.regroup_selection(realm);
        for (f, s) in self.sim.figures.iter_mut().zip(before) {
            f.selected = s;
        }
        merged
    }

    /// Every man stands on the cell he was sent to, none walking, and the unit is out of the fight.
    fn arrived(&self, unit: usize) -> bool {
        self.units.get(unit).figures > 0
            && !self.units.get(unit).in_melee
            && !self.modern.modes[unit].engaged
            && self.members(unit).into_iter().all(|m| {
                let f = &self.fighters[m];
                self.sim.figures[f.sim].state == State::Idle && (f.x, f.y) == f.target
            })
    }

    /// Once per frame, after the AI's sweep (`Battle_UpdateAllUnits` `0x00489401`): the engaging
    /// stances, then the queues. A no-op until a seat plays Modern, so Classic's frame is untouched.
    pub(crate) fn modern_sweep(&mut self) {
        if self.modern.is_idle() {
            return;
        }
        for unit in 1..=MAX_UNITS {
            if !self.units.get(unit).is_live() {
                self.modern.modes[unit] = UnitMode::default();
                continue;
            }
            if self.engages(unit) {
                self.engage(unit);
            }
            if !self.modern.modes[unit].queue.is_empty() && self.arrived(unit) {
                let q = self.modern.modes[unit].queue.remove(0);
                self.run_verb(unit, q.verb, q.at, q.target);
            }
        }
    }

    /// Q271 `sc2`: touched (`BattleUnit_JoinMelee` without the `human` skip, in `join_melee`),
    /// hit (unit `+0x0A`/`+0x0C`) or an enemy unit within `Enemy_NearestUnit`'s 8 cells (foot) / 9
    /// (melee), the men go: melee `Order_ChargeNearest`, bows `Order_ShootAtUnit` without walking.
    /// Clear again, the unit stops and reforms: `[I]` ours, C272 (a charged chase has no range limit,
    /// `Melee_ChooseChaseTarget` `0x004954DD`).
    fn engage(&mut self, unit: usize) {
        let u = *self.units.get(unit);
        let missile = u.category == 1;
        let radius = match (missile, u.category) {
            (true, _) => self.members(unit).first().map_or(9, |&m| {
                WeaponClass::for_troop(self.fighters[m].troop).map_or(9, |c| c.stats().range as i32 - 2)
            }),
            (_, 2) => 8,
            _ => 9,
        };
        let mut w = World {
            units: &mut self.units,
            figures: &mut self.sim.figures,
            positions: &self.positions,
            field: &self.ai_field,
            ai: &mut self.ai,
        };
        let near = w.nearest_enemy_unit(unit, radius, 1);
        let hit = u.hit_memory > 0 && w.units.get(u.last_attacker as usize).is_live();
        let engaged = self.modern.modes[unit].engaged;
        if missile {
            let firing = w.figures.iter().any(|f| f.unit as usize == unit && f.state == State::Shooting);
            if near != 0 && !firing {
                w.shoot_at_unit(unit, near);
                for f in w.figures.iter_mut().filter(|f| f.unit as usize == unit) {
                    f.straffed = true;
                }
                self.modern.modes[unit].engaged = true;
            }
        } else if (near != 0 || hit || u.in_melee) && !u.halted {
            w.charge_nearest(unit);
            self.modern.modes[unit].engaged = true;
        }
        let clear = near == 0 && !hit && !u.in_melee;
        if engaged && clear && !(missile && self.units.get(unit).firing != 0) {
            let dest = self.modern.modes[unit].attack_move.unwrap_or((u.x as u8, u.y as u8));
            self.modern.modes[unit].engaged = false;
            self.units.get_mut(unit).halted = false;
            self.order_unit(unit, dest.0, dest.1);
        }
    }
}
