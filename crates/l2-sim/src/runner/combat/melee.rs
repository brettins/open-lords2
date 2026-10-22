#![allow(unused_imports)]
use super::*;
use super::siege_works::*;
use super::castle::*;
use super::super::*;

impl BattleRunner {
    /// `Melee_ChooseChaseTarget` (`0x004954DD`): lowest score wins, where the
    /// score is the Chebyshev distance, **halved** if the enemy carries a
    /// missile weapon, plus that enemy's `targeted` count. No range limit at
    /// all, and siege engines are never chosen.
    pub(crate) fn chase_target(&self, i: usize) -> Option<usize> {
        let me = self.fighters[i].sim;
        let mine = self.sim.figures[me].owner;
        let (mx, my) = (self.fighters[i].x as i16, self.fighters[i].y as i16);
        let mut best: Option<(i32, usize)> = None;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() || f.owner == mine || f.owner == 0 || f.troop.is_siege() {
                continue;
            }
            let mut score = chebyshev(mx, my, self.fighters[j].x as i16, self.fighters[j].y as i16);
            if WEAPON_CLASS[f.troop.index()] != 0 {
                score /= 2;
            }
            // 00490000.c:2031 `local_c + (char)targeted`: read signed [V]
            score += f.targeted as i8 as i32;
            if best.is_none_or(|(bs, _)| score < bs) {
                best = Some((score, sim));
            }
        }
        best.map(|(_, j)| j)
    }

    /// `BattleUnit_JoinMelee`: contact drops the rest of an AI unit into free
    /// pursuit. **[D]** `docs/battle-ai.md` §4.2 — skipped for a human unit, a
    /// missile unit, or a unit holding an order lock, which is the single
    /// biggest visible difference between how the two sides fight.
    pub(crate) fn join_melee(&mut self, unit: usize) {
        if unit == 0 || unit > MAX_UNITS {
            return;
        }
        let u = *self.units.get(unit);
        if !u.is_live() || u.human || u.category == 1 || u.order_lock != 0 {
            return;
        }
        // `g_battleUnits[unit].withdrawing = 0` — 0x0048F079, 00480000.c:6340. [V]
        self.units.get_mut(unit).withdrawing = false;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &mut self.sim.figures[sim];
            if f.unit as usize != unit || !f.is_alive() || f.state == State::Melee {
                continue;
            }
            f.state = State::Chasing;
            f.target = None;
            self.fighters[j].path.clear();
            self.fighters[j].barred = 0;
        }
    }

    /// * **A besieger stepping onto a bridge sets it alight.** `Cell_TryEnter`'s
    ///   first statement is `if (surface == 7 && side == 4) FUN_0048551D(cell)`,
    ///   before it has decided anything, and it decides on the flags and height
    ///   it read *before* the fire cleared them. So the fire is lit after the
    ///   decision here, which is the same answer. Engines use
    ///   `Cell_TryEnterEngine`, which has no such test.
    /// BattleMan_Step 0x0048F1DD 00480000.c:6476-6498: JoinMelee for the stepped-at unit only
    /// `if (other.state != 4)` (:6489) [V].
    pub(crate) fn step_at_enemy(&mut self, i: usize, other: usize) {
        let (a, b) = (self.fighters[i].sim, self.fighters[other].sim);
        let fighting = self.sim.figures[b].state == State::Melee;
        let ua = self.unit_of(i);
        self.join_melee(ua);
        if !fighting {
            let ub = self.unit_of(other);
            self.join_melee(ub);
        }
        self.sim.engage_stepped(a, b);
    }

    pub(crate) fn enter(&mut self, i: usize, next: Pos) {
        let troop = self.fighters[i].troop;
        if troop == Troop::SiegeTowers && self.tower_step(i, next) {
            return;
        }
        let dst = next.y as usize * DIM + next.x as usize;
        let lights = self.fighters[i].side == SIDE_B
            && !fire::is_engine(troop)
            && self.field.cells[dst].surface == fire::SURFACE_BRIDGE;
        self.enter_cell(i, next);
        if lights {
            self.bridge_fire_at(next.x as i32, next.y as i32);
        }
    }

    fn enter_cell(&mut self, i: usize, next: Pos) {
        let dst = next.y as usize * DIM + next.x as usize;
        if self.siege.is_siege && self.strike_castle(i, dst) {
            return;
        }
        // `docs/battle.md` §7: *"a step is only allowed when the two cells'
        // elevations differ by at most 1, unless the destination's elevation is
        // exactly 5"*, marked `[V]`. [`crate::movement::can_step_elevation`]
        // has said so since it was written and **nothing called it** — the
        // pathfinder enforced the rule,
        // walking straight at its target (which is what a figure does when the
        // line is clear, and no search ever runs) climbed cliffs. It is inert
        // on a `.skr` field, where every cell is at elevation 0, and it is the
        // difference between a castle wall and a ramp everywhere else.
        {
            let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
            let (a, b) =
                (self.field.cells[src].elevation as i32, self.field.cells[dst].elevation as i32);
            if !crate::movement::can_step_elevation(a, b) {
                self.request_path(i);
                return;
            }
        }
        if self.blocked[dst] {
            // ```c
            // if (state == 9 && DAT_004EEA94 == 2) {
            //     man.field_0x190 = DAT_004EEAD4;   /* the cell it could not enter */
            //     man.field_0x18F = 1;
            //     return 0;
            // }
            // ```
            if self.sim.figures[self.fighters[i].sim].state == State::FillingMoat {
                self.fighters[i].moat_cell = Some(dst as u32);
                self.fighters[i].moat_load = 0;
                return;
            }
            self.request_path(i);
            return;
        }
        match self.occupant[dst] {
            None => {
                let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
                if self.occupant[src] == Some(i as u16) {
                    self.occupant[src] = None;
                }
                self.occupant[dst] = Some(i as u16);
                let f = &mut self.fighters[i];
                f.x = next.x;
                f.y = next.y;
                if f.path.last() == Some(&next) {
                    f.path.pop();
                }
                f.barred = 0;
                f.hold = 0;
                // `barred = 0; holdIt = 0; stepFlags &= ~1; dirc = dir;
                // walking = 1; FUN_00491b1f(man)`. The move above is
                // `FUN_00491B1F`; this is the counter, and from here the
                // figure's cell is the one it is crossing *to* — which is what
                // `BattleFigure_Draw` trails him behind.
                f.progress.begin_crossing();
                self.march(i, true);
            }
            Some(other) => {
                let other = other as usize;
                if self.fighters[other].side == self.fighters[i].side {
                    // `BattleMan_Step` (`0x0048F1DD`, `00480000.c:6441`): an
                    // engine skips the swap arm, so `local_10` stays 0 and
                    // `00480000.c:6495` — `if ((local_10 == 1) ||
                    // (isSiegeEngine == 0))` — hands it to the engine arm
                    // instead of the side-step and the search. **[V]**.
                    //
                    // That arm, `00480000.c:6560-6567`: `FUN_00491492` (return
                    // if non-zero), `local_10 = FUN_004912EC`, then `if
                    // (field_0x169 < 3) { tgX = mapX; tgY = mapY; return 0; }`
                    // — the destination only, `onRoute` untouched, so the
                    // engine keeps its stored route.
                    //
                    // **Two of the three are not built.** `FUN_004912EC`, the
                    // engine's own side-step through three rotations each way,
                    // so `local_10` is never 1 here and the tail parks him
                    // (`00480000.c:6583-6586`); and `FUN_00491492`
                    // (`00480000.c:6561-6563`), which runs *before* the
                    // distance test and can destroy the man
                    // (`BattleMan_Destroy`, `g_siegeApproachScore`,
                    // `g_siegeBreachScore`) — so the give-up and the park below
                    // run in frames where the original had already returned.
                    // `[D]`, both; [`Self::tower_step`] is the same arm reached
                    // from [`Self::enter`] for a tower.
                    if self.fighters[i].troop.is_siege() {
                        let f = &mut self.fighters[i];
                        if chebyshev(f.x as i16, f.y as i16, f.target.0 as i16, f.target.1 as i16)
                            < 3
                        {
                            f.target = (f.x, f.y);
                            return;
                        }
                        f.delay = 100;
                        return;
                    }
                    // The swap, the wait and the side-step are one arm and are
                    // written together: [`Self::friendly_step`],
                    // `00480000.c:6442-6463`.
                    self.friendly_step(i, other);
                } else if self.is_alive(other) {
                    // The 999 arm puts a pot in state 4 like anyone; it pours on its own Melee_Tick, step_one [V]
                    self.step_at_enemy(i, other);
                    let facing = self.fighters[i].facing;
                    self.strike(i, facing);
                } else {
                    self.occupant[dst] = None;
                }
            }
        }
    }

}
