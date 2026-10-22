#![allow(unused_imports)]
use super::*;
use super::slots::*;
use super::movement::*;
use super::missiles::*;
use super::super::*;

impl BattleRunner {
    /// `BattleMan_StateChase` (`0x00483E55`, `00480000.c:1653-1681`). True when the
    /// man stood (state 5): the original returns there. [V]
    pub(crate) fn retarget(&mut self, i: usize) -> bool {
        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].state != State::Chasing {
            return false;
        }
        // :1658 `if (6 < troopType) state = 0xb;` [I] no state-11 variant; Idle stands in
        // for BattleMan_StateEngineHalt 0x0048437E, as send_figure's Idle does for 10.
        if self.fighters[i].troop.is_siege() {
            self.sim.figures[sim].state = State::Idle;
        }
        // :1661 `if (target == 0 || target.owner == 0) target = Melee_ChooseChaseTarget();`
        let held = self.sim.figures[sim].target.filter(|&t| self.sim.figures[t].owner != 0);
        if held.is_none() {
            let chosen = self.chase_target(i);
            self.sim.figures[sim].target = chosen;
            if let Some(t) = chosen {
                // 00490000.c:2043 `targeted + 2`: a byte, wraps [V]
                self.sim.figures[t].targeted = self.sim.figures[t].targeted.wrapping_add(2);
            }
        }
        // :1665-1669 `if (target.state == 2) target = 0; if (target == 0) {Anim_Stand(); state = 5;}`
        let target = self.sim.figures[sim].target;
        match target.filter(|&t| self.sim.figures[t].state != State::Dead) {
            Some(t) => {
                if let Some(f) = self.fighters.iter().position(|f| f.sim == t) {
                    self.fighters[i].target = (self.fighters[f].x, self.fighters[f].y);
                }
                false
            }
            None => {
                self.sim.figures[sim].target = None;
                self.sim.figures[sim].state = State::Idle;
                self.stand(i);
                true
            }
        }
    }


    /// **`BattleMan_FireMissile` (`0x00483337`)** — one tick of a figure that
    /// carries a bow, a crossbow or a catapult.
    ///
    /// Marked `[I]`; the mechanism it drives is `[V]` throughout.
    pub(super) fn fire_tick(&mut self, i: usize) {
        self.reload_tick(i);
        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].target.is_some() {
            self.shoot(i);
        }
    }

    fn reload_tick(&mut self, i: usize) {
        let Some(class) = WeaponClass::for_troop(self.fighters[i].troop) else {
            return;
        };
        let stats = class.stats();
        let sim = self.fighters[i].sim;
        let counter = {
            let f = &mut self.sim.figures[sim];
            f.reload_counter = f.reload_counter.saturating_add(1);
            f.reload_counter
        };

        if counter + missile::ACQUIRE_LEAD == stats.reload {
            match self.missile_target(i, stats.range as i32) {
                Some(t) => self.sim.figures[sim].target = Some(t),
                None => {
                    let f = &mut self.sim.figures[sim];
                    f.target = None;
                    f.reload_counter = missile::NO_TARGET_RESET;
                }
            }
            return;
        }
        if counter <= stats.reload {
            return;
        }
        self.sim.figures[sim].reload_counter = 0;
        let Some(target) = self.sim.figures[sim].target else {
            return;
        };
        // [D] the original spawns while `owner != 0` (00480000.c:1478); a body
        // that stood back up (BNEW-dead-archer-shoots) is shot at, a corpse is not
        if self.sim.figures[target].state == State::Dead {
            return;
        }
        let Some(t) = self.fighters.iter().position(|f| f.sim == target) else {
            return;
        };
        self.loose(i, class, (self.fighters[t].x, self.fighters[t].y));
    }

    fn loose(&mut self, shooter: usize, class: WeaponClass, at: (u8, u8)) {
        self.loose_from(shooter, class, at, false);
    }

    /// `closing` picks `BattleMan_StateCloseToAttack`'s copy of the launch
    /// (`00480000.c:2057-2104`) over `BattleMan_FireMissile`'s. The two differ
    /// in the blocked seed — `((Gtime + figure) & 0x10) + 4` against `(figure &
    /// 0x10) + 4` — and in which occasion the sound is asked on. `[V]`
    pub(crate) fn loose_from(
        &mut self,
        shooter: usize,
        class: WeaponClass,
        at: (u8, u8),
        closing: bool,
    ) {
        let sim = self.fighters[shooter].sim;
        // `BattleMan_FireMissile` (`0x00483337`, `00480000.c:1478-1479`)
        // `g_battleMen[cur].firing = 0x78; g_battleUnits[sVar2].firing = 0x78;`
        // — **before** `Missile_Spawn`, so a shot that finds no free slot still
        // marks the unit. Read by `UnitOrder_FieldMissile` (`0x0048A9D2`,
        // `:4336`) and `UnitOrder_SiegeDefMissile` (`0x0048E097`, `:5813`).
        //
        // `[D]` the `<= MAX_UNITS` bound: `:1479` indexes `sVar2` unbounded,
        // and slot 0 is the original's unused record, which ours is too — the
        // bound is only what keeps an out-of-range `+0x178` from panicking.
        //
        // `g_battleMen[cur].firing = 0x78` stands beside each of the three
        // unit-level writes — `:1478`, `:1837`, `:2053`. Stepped down by
        // `Battle_UpdateAllMen` (`0x004822ED`, `:896-897`), its only reader.
        self.sim.figures[sim].firing = crate::unit::FIRING;
        {
            let u = self.sim.figures[sim].unit as usize;
            if u <= MAX_UNITS {
                self.units.get_mut(u).firing = crate::unit::FIRING;
            }
        }
        let (owner, band) = {
            let f = &self.sim.figures[sim];
            (f.owner, f.strength_band())
        };
        let here = (self.fighters[shooter].x, self.fighters[shooter].y);
        let elevation = self.field.cells[here.1 as usize * DIM + here.0 as usize].elevation;
        let power = missile::band_scaled(class.stats().damage, band);
        let Some(slot) = missile::spawn(
            &mut self.missiles,
            owner,
            class,
            sim,
            here,
            at,
            power,
            elevation,
        ) else {
            return;
        };
        if closing {
            // `:2088-2089`, `((char)DAT_0057c998 + (char)g_curBattleMan & 0x10) + 4`.
            // `DAT_0057C998` is the debug panel's `Gtime`, zeroed at turn phase
            // 7 and not at the start of a battle; the battle's tick stands in
            // for it. `[D]` on the stand-in — the only effect is whether a
            // blocked volley gives up after 4 frames or 20.
            self.missiles.get_mut(slot).blocked_ticks =
                ((self.tick as u8).wrapping_add(sim as u8) & 0x10) + 4;
        }
        // **A fire arrow.** `BattleMan_FireMissile` writes `+0x44 = 1` when the
        // shooter's unit is an AI's, of side 0, and more than three of a
        // human's figures are standing in woodland — the garrison sets the
        // wood alight under a player who has hidden an army in it. Before the
        // launch steps, so an arrow loosed from inside the wood lights its
        // own cell. The catapult fires from `BattleMan_StateEngineFire`, which
        // has no such write.
        let unit = self.sim.figures[sim].unit as usize;
        if self.sim.figures[sim].state == State::Shooting {
            // **The player's fire arrow.** `BattleMan_StateCloseToAttack`
            // (`0x00484BF9`) copies the unit's `targetCell` (`+0x30`) onto
            // every arrow it looses, unconditionally — no woodland count and
            // no side test, because `BattleUnit_Order` has already refused to
            // write `targetCell` for anyone else.
            if (1..=MAX_UNITS).contains(&unit) {
                self.missiles.get_mut(slot).fire_arrow = self.units.get(unit).target_cell;
            }
        } else if class != WeaponClass::Catapult && self.humans_in_woods > fire::FIRE_ARROWS_ABOVE {
            if (1..=MAX_UNITS).contains(&unit)
                && !self.units.get(unit).human
                && self.units.get(unit).side == SIDE_A
            {
                self.missiles.get_mut(slot).fire_arrow = 1;
            }
        }
        // `:1516-1518` has no exit: eight `Missile_Step`s whatever a step did,
        // then `Missile_LinkToCell(g_curMissile)` at `:1519` on the record as
        // it stands. A step on a freed record is inert — `FUN_0046EB78` zeroed
        // `+0x38`, so `:1260-1265`'s range test frees it again.
        for _ in 0..missile::LAUNCH_STEPS {
            self.step_missile(slot);
        }
        // **The bow sounds after the flight, not before.**
        // `BattleMan_FireMissile` (`00480000.c:1523-1527`) tests the record's
        // class `+0x09` *after* the eight steps (`:1516-1518`); a step that
        // removed the missile ran `FUN_0046EB78`, which zeroes all `0x4C`
        // bytes, so neither `2` nor `1` matches and the shot is silent. Same
        // tail at `BattleMan_StateCloseToAttack` (`0x00484BF9`, `:2099-2104`).
        // `BattleMan_StateEngineFire` (`0x004843BC`, `:1861`) plays slot `0xE`
        // with no such test. `[V]`
        if class == WeaponClass::Catapult || self.missiles.get(slot).class == class.index() {
            if closing {
                self.sim.cues.closing_loose(class);
            } else {
                self.sim.cues.loose(class);
            }
        }
    }

    /// `Missile_FindTarget` (`0x004956CC`) as the firing path uses it.
    pub(super) fn missile_target(&self, i: usize, range: i32) -> Option<usize> {
        let me = self.fighters[i].sim;
        let mine = self.sim.figures[me].owner;
        let (sx, sy) = (self.fighters[i].x as i32, self.fighters[i].y as i32);
        let mut best: Option<(i32, usize)> = None;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &self.sim.figures[sim];
            // Missile_FindTarget 0x004956CC: `state != 2`, not men > 0 [V]
            if f.state == State::Dead || f.owner == mine || f.owner == 0 {
                continue;
            }
            let (dx, dy) = (
                (self.fighters[j].x as i32 - sx).abs(),
                (self.fighters[j].y as i32 - sy).abs(),
            );
            if dx > range || dy > range {
                continue;
            }
            let mut score = (dx + dy).min(160);
            if f.troop.is_siege() {
                score += 35;
            }
            if score >= 160 {
                continue;
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, sim));
            }
        }
        best.map(|(_, j)| j)
    }

}
