//! `BattleMan_StateCloseToAttack` (`0x00484BF9`), state 17 — the missile arm of
//! [`BattleRunner::retarget`], lifted out of `orders.rs` for its size.

use super::*;

impl BattleRunner {
    pub(super) fn close_to_attack(&mut self, i: usize) {
        let sim = self.fighters[i].sim;
        let alive = self.sim.figures[sim]
            .target
            .is_some_and(|t| self.sim.figures[t].is_alive());
        // **`&& targetCell == 0` is the rest of the original's test**, and it is
        // what makes the player's fire arrow survive `Order_StopShortOfTarget`.
        // `BattleMan_StateCloseToAttack` (`0x00484BF9`) opens on
        // `other.owner == 0 && unit.targetCell == 0`, and that arm is the one
        // that *leaves* state 17 — by writing **state 5**, which answers
        // [`BattleRunner::fire_tick`]'s open question about what puts a figure
        // into the firing state.
        //
        // fire-arrow cell holds its bowmen in 17 with no live target, and they
        // acquire one there. `[V]`.
        let cell = {
            let u = self.sim.figures[sim].unit as usize;
            if (1..=MAX_UNITS).contains(&u) { self.units.get(u).target_cell } else { 0 }
        };
        if !alive {
            if cell == 0 {
                self.sim.figures[sim].state = State::Idle;
                self.sim.figures[sim].target = None;
                return;
            }
            let Some(shot) = WeaponClass::for_troop(self.fighters[i].troop) else {
                return;
            };
            match self.missile_target(i, shot.stats().range as i32) {
                Some(t) => self.sim.figures[sim].target = Some(t),
                None => return,
            }
        }
        // **`BattleMan_StateCloseToAttack` (`0x00484BF9`) looses.** Its cadence
        // is `BattleMan_FireMissile`'s own pair — swingTimer `+0xE0` against
        // reloadInterval `+0xCB`, `interval < timer` shoots and resets — with no
        // ten-tick acquisition, because the target was chosen when the state was
        // entered. `:2053-2054` set both firing bytes, which
        // [`BattleRunner::loose`] does for either site.
        let Some(class) = WeaponClass::for_troop(self.fighters[i].troop) else {
            return;
        };
        let counter = {
            let f = &mut self.sim.figures[sim];
            f.reload_counter = f.reload_counter.saturating_add(1);
            f.reload_counter
        };
        if counter <= class.stats().reload {
            return;
        }
        self.sim.figures[sim].reload_counter = 0;
        let Some(t) = self.sim.figures[sim].target else {
            return;
        };
        let Some(t) = self.fighters.iter().position(|f| f.sim == t) else {
            return;
        };
        // `g_battleUnits[unit].targX = g_battleMen[cur].mapX; .targY = .mapY` —
        // `00480000.c:2055-2056`, between the firing bytes and `Missile_Spawn`:
        // the shot pins the unit's destination to **the shooter's own cell**, so
        // the next `BattleUnit_Reform` (`0x0048970E`) forms the unit up around
        // whichever of its men loosed last.
        let here = (self.fighters[i].x, self.fighters[i].y);
        let u = self.sim.figures[sim].unit as usize;
        if (1..=MAX_UNITS).contains(&u) {
            let unit = self.units.get_mut(u);
            unit.target_x = here.0 as i16;
            unit.target_y = here.1 as i16;
        }
        self.loose(i, class, (self.fighters[t].x, self.fighters[t].y));
    }
}
