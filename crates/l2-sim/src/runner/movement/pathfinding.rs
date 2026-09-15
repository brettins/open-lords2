#![allow(unused_imports)]
use super::movement::*;
use super::*;
use super::*;

impl BattleRunner {
    /// `FUN_00479A71` (`0x00479A71`) — **clear one player's whole selection**,
    /// and the tail of every other selection verb.
    pub fn clear_selection(&mut self, owner: u8) {
        for f in self.sim.figures.iter_mut() {
            if f.selected == owner {
                f.selected = 0;
            }
        }
    }

    /// `FUN_00479B58` (`0x00479B58`) — **the rubber-band box**.
    ///
    /// **It does not clear first.** `FUN_0043C247` clears and then boxes, in
    /// that order, so the box is always a fresh selection
    /// addition — see [`Self::pick_box`].
    pub fn select_box(&mut self, owner: u8, a: (u8, u8), b: (u8, u8)) {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        for y in y0..=y1.min(DIM as u8 - 1) {
            for x in x0..=x1.min(DIM as u8 - 1) {
                let Some(o) = self.occupant[y as usize * DIM + x as usize] else {
                    continue;
                };
                let sim = self.fighters[o as usize].sim;
                if self.sim.figures[sim].owner == owner {
                    self.sim.figures[sim].selected = owner;
                }
            }
        }
    }

    /// The original has no such entry point on the click path — a box is the
    /// only way a figure is picked — but `FUN_0043C910`, the control-group
    /// recall, restores a *stored selection* by copying ten bytes back over the
    /// live one, which is this, applied to a list.
    pub fn select_figure(&mut self, fighter: usize, owner: u8) {
        let Some(f) = self.fighters.get(fighter) else {
            return;
        };
        let sim = f.sim;
        if self.sim.figures[sim].is_alive() && self.sim.figures[sim].owner == owner {
            self.sim.figures[sim].selected = owner;
        }
    }

    /// `FUN_00478F0B` (`0x00478F0B`) — **narrow the selection to one unit**.
    pub fn narrow_selection(&mut self, owner: u8) {
        let first = (0..self.fighters.len()).find(|&i| {
            let sim = self.fighters[i].sim;
            self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
        });
        let Some(first) = first else { return };
        let keep = self.sim.figures[self.fighters[first].sim].unit;
        if keep == 0 {
            return;
        }
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            let f = &mut self.sim.figures[sim];
            if f.selected == owner && f.unit != keep {
                f.selected = 0;
            }
        }
    }

    /// `FUN_0043C247` (`0x0043C247`) — **clear, box, and (on the committing
    /// call) regroup**, which is the whole of what a completed drag does.
    ///
    /// The original passes `param_2 = 1` on the release and `0` on every frame
    /// of the drag, and only the `1` runs `FUN_00478987`. So the selection is
    /// recomputed live while the box is being drawn and the *units are only
    /// rearranged once*, when the button comes up.
    pub fn pick_box(&mut self, owner: u8, a: (u8, u8), b: (u8, u8), commit: bool) {
        self.clear_selection(owner);
        self.select_box(owner, a, b);
        if commit {
            self.regroup_selection(owner);
            self.narrow_selection(owner);
        }
    }

    /// `FUN_0043C4C6` (`0x0043C4C6`) — **one click on one banner in the right
    /// column takes that figure out of the selection**, then regroups.
    pub fn deselect_figure(&mut self, fighter: usize) {
        let Some(f) = self.fighters.get(fighter) else {
            return;
        };
        let sim = f.sim;
        let owner = self.sim.figures[sim].owner;
        self.sim.figures[sim].selected = 0;
        self.regroup_selection(owner);
    }

    /// `FUN_00478987` (`0x00478987`) — **the regroup
    /// is simulation state**.
    ///
    /// Returns the unit the player's selection now is — the original's
    /// `DAT_0053E984` — or 0 when nothing is selected.
    pub fn regroup_selection(&mut self, owner: u8) -> usize {
        if owner == 0 {
            return 0;
        }
        let first = (0..self.fighters.len()).find(|&i| {
            let sim = self.fighters[i].sim;
            self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
        });
        let Some(first) = first else { return 0 };
        let base = self.sim.figures[self.fighters[first].sim].unit as usize;

        let mut split = false;
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() {
                continue;
            }
            if f.selected == 0 && f.unit as usize == base {
                split = true;
            }
            if f.selected == owner && f.unit as usize != base {
                split = true;
            }
        }
        if base != 0 {
            self.units.get_mut(base).reform = crate::unit::REFORM_ON_ORDER;
        }
        if !split {
            return base;
        }

        let human = self.units.get(base).human;
        let side = self.units.get(base).side;
        let Some(new) = self.units.create(owner, human, side, 3) else {
            return base;
        };
        self.units.get_mut(new).reform = crate::unit::REFORM_ON_ORDER;
        let mut first_fig = 0u16;
        let mut last_fig = 0u16;
        let mut category = 3u8;
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            if !self.sim.figures[sim].is_alive() || self.sim.figures[sim].selected != owner {
                continue;
            }
            if first_fig == 0 {
                first_fig = i as u16;
            }
            last_fig = i as u16;
            category = if WEAPON_CLASS[self.fighters[i].troop.index()] == 0 {
                3
            } else {
                1
            };
            self.sim.figures[sim].unit = new as u16;
        }
        {
            let u = self.units.get_mut(new);
            u.first = first_fig;
            u.last = last_fig;
            u.category = category;
        }
        self.rebuild_units();
        new
    }

    /// How many of `owner`'s figures are picked — the original's
    /// `DAT_00553078`, which gates every order.
    pub fn selected_count(&self, owner: u8) -> usize {
        (0..self.fighters.len())
            .filter(|&i| {
                let sim = self.fighters[i].sim;
                self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
            })
            .count()
    }

    /// The picked figures, low index first — the order the banner panel draws
    /// them in and the order `FUN_0043C2A9` hit-tests them in.
    pub fn selected_fighters(&self, owner: u8) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&i| {
                let sim = self.fighters[i].sim;
                self.sim.figures[sim].is_alive() && self.sim.figures[sim].selected == owner
            })
            .collect()
    }

    pub fn is_selected(&self, fighter: usize) -> bool {
        self.fighters
            .get(fighter)
            .is_some_and(|f| self.sim.figures[f.sim].selected != 0)
    }

    pub fn selected_by(&self, fighter: usize) -> u8 {
        self.fighters
            .get(fighter)
            .map_or(0, |f| self.sim.figures[f.sim].selected)
    }

    /// `FUN_0043C634` (`0x0043C634`) → `BattleUnit_Order` — **the click that
    /// gives an order**.
    ///
    /// The original orders exactly one unit, `DAT_0053E984`, because
    /// [`Self::regroup_selection`] has already made the selection be one unit.
    ///
    /// `woodland` is the original's fifth argument, `DAT_0053E874`. See
    /// [`Self::order_full`] — it is not "from a player" whatever
    /// `docs/symbols.json` calls it.
    pub fn order_selected(
        &mut self,
        owner: u8,
        x: u8,
        y: u8,
        target: Option<usize>,
        woodland: bool,
    ) -> bool {
        let unit = self.regroup_selection(owner);
        if unit == 0 {
            return false;
        }
        self.order_full(unit, x, y, target, woodland, Formation::Keep);
        true
    }

    /// `FUN_0043C77A` (`0x0043C77A`) — **the `H` and `V` keys**.
    ///
    /// The original reads `DAT_0053E984` directly and does **not** regroup
    /// first; it also plays the acknowledgement cry unconditionally, even when
    pub fn order_formation(&mut self, unit: usize, formation: Formation) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        let (x, y) = {
            let u = self.units.get(unit);
            (
                u.x.clamp(0, DIM as i16 - 1) as u8,
                u.y.clamp(0, DIM as i16 - 1) as u8,
            )
        };
        self.order_full(unit, x, y, None, false, formation);
    }

    /// `BattleUnit_Order` (`0x00479E90`) with **all six of its arguments**, as
    /// against [`Self::order_unit`], which is the AI's three.
    ///
    /// * **`attackTarget`** — the enemy figure under the cursor. It is stored in
    ///   unit `+0x2C` and read by the order handlers; when the unit mixes
    ///   missile and melee figures the original *splits it*
    ///   it, which [`Self::regroup_selection`] already models on the selection
    ///   side.
    ///
    /// * **`woodland`** — `DAT_0053E874`, and the name in `docs/symbols.json`
    ///   is wrong. Its only writer is `Battle_UpdateHover`, which sets it when
    ///   the hovered cell's **surface byte is 15** and clears it otherwise, and
    ///   every AI call site passes a literal 0. Its effect is that a missile
    ///   unit of **side 0** ordered onto woodland has `Order_StopShortOfTarget`
    ///   applied. Reported as a correction.
    pub fn order_full(
        &mut self,
        unit: usize,
        x: u8,
        y: u8,
        target: Option<usize>,
        woodland: bool,
        formation: Formation,
    ) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        {
            let u = self.units.get_mut(unit);
            u.withdrawing = false;
            if u.in_melee {
                u.order_lock = crate::unit::ORDER_LOCK;
            }
            u.reform = crate::unit::REFORM_ON_ORDER;
            if !woodland {
                u.target_cell = 0;
            }
        }
        // `unit.orderedTarget`, `+0x2C`: the enemy figure the player pointed at.
        if let Some(t) = target {
            let members = self.members(unit);
            for m in members {
                let sim = self.fighters[m].sim;
                self.sim.figures[sim].target = Some(self.fighters[t].sim);
            }
        }
        // **The two arms that pull a missile unit's destination back**, and
        // `targetCell`, `+0x30` — the player's fire arrow. `BattleUnit_Order`
        // (`0x00479E90`) writes `targetCell` only when the hovered cell is
        // woodland, the unit has a live missile figure, it is of side 0, no
        // enemy is under the cursor, and `BattleUnit_Classify` leaves
        // `+0x08 < 5`. `[V]`. Either arm then runs
        // [`Self::pull_back_to_range`] — `Order_StopShortOfTarget` and
        // `Dest_FindReachableNear`. The mixed missile-and-melee unit the
        // original splits here is already split by
        // [`Self::regroup_selection`], so the enemy-under-cursor arm needs no
        // melee test.
        let missiles_here = self
            .members(unit)
            .into_iter()
            .any(|m| WEAPON_CLASS[self.fighters[m].troop.index()] != 0);
        let (side, category) = {
            let u = self.units.get(unit);
            (u.side, u.category)
        };
        let mut dest = (x as i32, y as i32);
        if target.is_some() && missiles_here {
            self.units.get_mut(unit).target_cell = 0;
            if category < 5 {
                dest = self.pull_back_to_range(unit, dest);
            }
        } else if woodland && missiles_here && side == SIDE_A {
            self.units.get_mut(unit).target_cell = 0;
            if category < 5 {
                self.units.get_mut(unit).target_cell = fire::cell_byte_offset(x as i32, y as i32);
                dest = self.pull_back_to_range(unit, dest);
            }
        }
        self.pour_on_order(unit, dest.0 as i16, dest.1 as i16);
        {
            let u = self.units.get_mut(unit);
            u.target_x = dest.0 as i16;
            u.target_y = dest.1 as i16;
        }
        if let Some(o) = formation.orientation() {
            self.units.get_mut(unit).orientation = o;
            let (hx, hy) = {
                let u = self.units.get(unit);
                (u.x, u.y)
            };
            let u = self.units.get_mut(unit);
            u.target_x = hx;
            u.target_y = hy;
        }
        self.reform_unit(unit);
    }

    /// `FUN_0047A76D` (`0x0047A76D`) — **the charge button**, battle button 3.
    ///
    /// Every one of this player's figures whose troop type is under 7 — so no
    /// siege engine and no oil — goes into free pursuit and its unit is halted,
    /// which switches the every-500-frame reform off. It is **not**
    /// `Order_ChargeNearest` (`0x0048C8AF`), which is the AI's and also clears
    /// the withdraw flag and the figures' targets; this one does neither.
    ///
    /// The button that calls it is guarded by a once-per-battle latch
    /// (`DAT_0055322C`),
    pub fn charge_all(&mut self, owner: u8) {
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            if self.sim.figures[sim].owner != owner || self.fighters[i].troop.is_siege() {
                continue;
            }
            let unit = self.sim.figures[sim].unit as usize;
            if unit != 0 && unit <= MAX_UNITS {
                self.units.get_mut(unit).halted = true;
            }
            self.sim.figures[sim].state = State::Chasing;
        }
    }

    pub fn occupant_of(&self, x: u8, y: u8) -> Option<usize> {
        if x as usize >= DIM || y as usize >= DIM {
            return None;
        }
        self.occupant[y as usize * DIM + x as usize].map(|o| o as usize)
    }

    /// `BattleUnits_RebuildFromFigures` (`0x00488DFE`) — exposed because
    /// [`Self::regroup_selection`] has to run it after moving figures between
    /// units, and the tick loop's own call is a tick away.
    fn rebuild_units(&mut self) {
        self.units.rebuild_from_figures(&mut self.sim.figures);
    }
}
